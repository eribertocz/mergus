//! The last commit of a load by chunks and its row of `ops.loads` (MG-REQ-0413,
//! MG-REQ-0415.3, MG-REQ-0418.1), and the row of a load that failed before any commit.

use std::collections::BTreeSet;

use arrow::array::{RecordBatch, RecordBatchIterator};
use arrow::datatypes::SchemaRef;
use mergus_adapter::{StorageAdapter, VersionRef, WriteMode};
use mergus_ports::Timestamp;
use mergus_source::{IncrementalReader, LateRows};

use crate::RawError;
use crate::chunk_load::{LoadOutcome, Stores, TableLoad, load_rows, rows_error};
use crate::chunk_rows::{Current, deleted_flags, stamped};
use crate::load_error::LoadError;
use crate::raw_rows::{Metadata, conform};
use crate::registry::{LoadMode, LoadRecord, LoadResult, micros, record_load};

/// The load in progress.
pub(crate) struct Step<'a> {
    /// The table to load.
    pub(crate) load: &'a TableLoad<'a>,
    /// Its load id.
    pub(crate) load_id: String,
    /// Its `_extracted_at`.
    pub(crate) extracted_at: Timestamp,
    /// The `watermark_after` of the table's last recorded load.
    pub(crate) before: Option<String>,
    /// When the last `compare` of the table ran: the latest non-NULL `deletes_checked_at`.
    pub(crate) checked: Option<Timestamp>,
    /// Whether it resumes an unfinished load.
    pub(crate) resumed: bool,
}

impl Step<'_> {
    /// The metadata columns of the load's rows (MG-REQ-0512).
    pub(crate) fn metadata(&self) -> Metadata {
        Metadata {
            load_id: self.load_id.clone(),
            extracted_at: micros(self.extracted_at),
        }
    }
}

/// The source schema, the raw schema and the current rows of a load that read a schema.
pub(crate) type Shape = (SchemaRef, SchemaRef, Current);

/// What the chunks of a load did.
pub(crate) struct Progress {
    /// The shape of its rows; `None` when no row and no current table gave a schema.
    pub(crate) shape: Option<Shape>,
    /// The latest version of raw.
    pub(crate) raw_version: Option<VersionRef>,
    /// The rows read, those committed before a resume included (MG-REQ-0514).
    pub(crate) read: i64,
    /// The keys read, to find one read twice (MG-REQ-0518.3).
    pub(crate) seen: BTreeSet<arrow::row::OwnedRow>,
}

/// Ends the load of `step`: appends the deletes of a due `compare` to `changes`, rewrites
/// `current` from the load's rows in the same commit, then records the load in `ops.loads`
/// with its watermark (MG-REQ-0418.1, MG-REQ-0413).
pub(crate) fn finish(
    stores: &mut Stores<'_>,
    reader: &mut IncrementalReader<'_>,
    step: &Step<'_>,
    progress: Progress,
    changes: &str,
    current: &str,
) -> Result<LoadOutcome, LoadError> {
    let load = step.load;
    let Some((source, schema, rows)) = progress.shape else {
        let empty = RecordBatch::new_empty(std::sync::Arc::new(arrow::datatypes::Schema::empty()));
        let ended = Ended {
            read: 0,
            checked: None,
            late: None,
        };
        return recorded(stores, reader, step, progress.raw_version, &empty, ended);
    };
    let raw_version = progress.raw_version;
    let loaded = load_rows(stores.raw, changes, raw_version.as_ref(), &step.load_id)?;
    let loaded = match loaded {
        Some(loaded) => conform(&loaded, &schema),
        None => Ok(RecordBatch::new_empty(schema.clone())),
    }
    .map_err(|error| rows_error(load, error))?;
    let compare = reader.compare_due(step.checked, step.extracted_at);
    let (deletes, new_current, late) =
        match final_rows(reader, step, compare, (&source, &schema, &rows), &loaded) {
            Ok(rows) => rows,
            Err(error) => return Err(record_failed(stores, step, true, error)),
        };
    if deletes.num_rows() > 0 {
        write(stores.raw, changes, WriteMode::Append, deletes)?;
    }
    write(stores.raw, current, WriteMode::Replace, new_current)?;
    let raw_version = Some(stores.raw.commit().map_err(RawError::Storage)?);
    let checked = compare.then_some(step.extracted_at);
    let ended = Ended {
        read: progress.read,
        checked,
        late,
    };
    recorded(stores, reader, step, raw_version, &loaded, ended)
}

/// Records the unfinished load of `step` whose last commit was made, its rows `prior`: only
/// its `ops.loads` row was missing (MG-REQ-0418.2). Whether it compared is known again from
/// its `_extracted_at` and the last compare.
pub(crate) fn record_only(
    stores: &mut Stores<'_>,
    reader: &mut IncrementalReader<'_>,
    step: &Step<'_>,
    prior: &RecordBatch,
) -> Result<LoadOutcome, LoadError> {
    let compare = reader.compare_due(step.checked, step.extracted_at);
    let read = reader.rows_through(None)?;
    let ended = Ended {
        read,
        checked: compare.then_some(step.extracted_at),
        late: None,
    };
    let raw_version = step.load.raw_version.clone();
    let read = read_rows(reader, prior).map_err(|error| rows_error(step.load, error))?;
    recorded(stores, reader, step, raw_version, &read, ended)
}

/// The rows of `rows`, a load's rows in `__changes`, that were read from the source: all but
/// the deletes of a `compare`, which carry the last known values. With `column:<name>` a
/// delete was read; with another policy every delete comes from a `compare`. These are the
/// rows `finish` takes the watermark from, before its compare (MG-REQ-0412.1).
fn read_rows(
    reader: &IncrementalReader<'_>,
    rows: &RecordBatch,
) -> Result<RecordBatch, arrow::error::ArrowError> {
    if let mergus_source::DeletePolicy::Column(_) = reader.options().deletes {
        return Ok(rows.clone());
    }
    live_rows(rows)
}

/// How a load ended, for its `ops.loads` row.
struct Ended {
    /// The rows read (MG-REQ-0514).
    read: i64,
    /// Its `_extracted_at` when it compared, else `None` (MG-REQ-0415.3).
    checked: Option<Timestamp>,
    /// The very late rows its compare found.
    late: Option<LateRows>,
}

/// Records the finished load of `step`, whose committed rows are `loaded`, in `ops.loads`,
/// with the largest incremental value of `loaded` as `watermark_after`, or the previous
/// watermark when none has a value (MG-REQ-0412.1, MG-REQ-0413).
fn recorded(
    stores: &mut Stores<'_>,
    reader: &mut IncrementalReader<'_>,
    step: &Step<'_>,
    raw_version: Option<VersionRef>,
    loaded: &RecordBatch,
    ended: Ended,
) -> Result<LoadOutcome, LoadError> {
    let watermark = if loaded.num_rows() > 0 {
        reader.watermark(std::slice::from_ref(loaded))?
    } else {
        None
    };
    let watermark = watermark.or_else(|| step.before.clone());
    let record = record(
        stores,
        step,
        ended.read,
        watermark.clone(),
        ended.checked,
        LoadResult::Ok,
    );
    let ops_version = record_load(stores.ops, &record).map_err(RawError::Registry)?;
    let mut outcome = outcome(step, raw_version, ops_version, ended.read, watermark);
    outcome.late = ended.late;
    Ok(outcome)
}

/// The deletes of the last commit, the current rows it writes and the very late rows: for a
/// table without `key`, the load's rows that are not deletes; otherwise the current rows with
/// the load applied, less what a due `compare` found deleted (MG-REQ-0415.3).
fn final_rows(
    reader: &mut IncrementalReader<'_>,
    step: &Step<'_>,
    compare: bool,
    shape: (&SchemaRef, &SchemaRef, &Current),
    loaded: &RecordBatch,
) -> Result<(RecordBatch, RecordBatch, Option<LateRows>), LoadError> {
    let load = step.load;
    let schema = shape.1;
    let arrow = |error| LoadError::Raw(rows_error(load, error));
    let empty = RecordBatch::new_empty(schema.clone());
    if load.table.key.is_empty() {
        return Ok((empty, live_rows(loaded).map_err(arrow)?, None));
    }
    let merged = shape.2.merged(loaded).map_err(arrow)?;
    if !compare {
        return Ok((empty, merged, None));
    }
    let found = reader.compare(std::slice::from_ref(&merged), step.extracted_at)?;
    let metadata = Metadata {
        load_id: step.load_id.clone(),
        extracted_at: micros(step.extracted_at),
    };
    let (deletes, kept) =
        apply_deletes(shape, loaded, &merged, &found.deleted, &metadata).map_err(arrow)?;
    Ok((deletes, kept, found.late))
}

/// The deletes a `compare` found, as rows of `__changes` with the last known values, minus
/// the keys this load already holds as deletes, which a resumed load committed before; and
/// `merged`, the current rows, without their keys.
fn apply_deletes(
    (source, schema, current): (&SchemaRef, &SchemaRef, &Current),
    loaded: &RecordBatch,
    merged: &RecordBatch,
    found: &[RecordBatch],
    metadata: &Metadata,
) -> Result<(RecordBatch, RecordBatch), arrow::error::ArrowError> {
    let mask = deleted_flags(loaded)?.into_iter().map(Some).collect();
    let done = arrow::compute::filter_record_batch(loaded, &mask)?;
    let done: BTreeSet<_> = current.keys_of(std::slice::from_ref(&done))?;
    let deletes = stamped(found, source, schema, metadata, true)?;
    let deletes = current.without(&deletes, &done)?;
    let gone = current.keys_of(std::slice::from_ref(&deletes))?;
    let kept = current.without(merged, &gone)?;
    Ok((deletes, kept))
}

/// The rows of `loaded` that are not deletes.
fn live_rows(loaded: &RecordBatch) -> Result<RecordBatch, arrow::error::ArrowError> {
    let mask = deleted_flags(loaded)?
        .iter()
        .map(|deleted| Some(!deleted))
        .collect();
    arrow::compute::filter_record_batch(loaded, &mask)
}

/// Stages `rows` for `table` through `raw`.
fn write(
    raw: &mut dyn StorageAdapter,
    table: &str,
    mode: WriteMode,
    rows: RecordBatch,
) -> Result<(), RawError> {
    let schema = rows.schema();
    raw.write(
        table,
        mode,
        Box::new(RecordBatchIterator::new([Ok(rows)], schema)),
    )?;
    Ok(())
}

/// Records `step` as failed with the code of `error` when it committed nothing, and returns
/// `error`. A load that committed a chunk stays unfinished, for the next load to resume
/// (MG-REQ-0418.2).
pub(crate) fn record_failed(
    stores: &mut Stores<'_>,
    step: &Step<'_>,
    committed: bool,
    error: LoadError,
) -> LoadError {
    if committed {
        return error;
    }
    let mut record = record(
        stores,
        step,
        0,
        step.before.clone(),
        None,
        LoadResult::Failed,
    );
    record.error_code = error.code().map(str::to_owned);
    // The failure of the load is the error reported; its row is a best effort.
    record_load(stores.ops, &record).ok();
    error
}

/// The row of `ops.loads` of `step`, ending now.
fn record(
    stores: &Stores<'_>,
    step: &Step<'_>,
    rows: i64,
    watermark: Option<String>,
    checked: Option<Timestamp>,
    result: LoadResult,
) -> LoadRecord {
    let load = step.load;
    let mode = if load.table.incremental.is_some() && step.before.is_some() {
        LoadMode::Incremental
    } else {
        LoadMode::Full
    };
    let elapsed = micros(stores.clock.now()).saturating_sub(micros(step.extracted_at));
    LoadRecord {
        load_id: step.load_id.clone(),
        source: load.source.name.clone(),
        table: load.table.name.clone(),
        mode,
        rows,
        watermark_before: step.before.clone(),
        watermark_after: watermark,
        started_at: step.extracted_at,
        duration_ms: elapsed.max(0) / 1_000,
        result,
        error_code: None,
        deletes_checked_at: checked,
    }
}

/// The outcome of `step`.
fn outcome(
    step: &Step<'_>,
    raw_version: Option<VersionRef>,
    ops_version: VersionRef,
    rows: i64,
    watermark: Option<String>,
) -> LoadOutcome {
    LoadOutcome {
        load_id: step.load_id.clone(),
        resumed: step.resumed,
        raw_version,
        ops_version,
        rows,
        watermark,
        late: None,
    }
}
