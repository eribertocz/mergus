//! The last commit of a load by chunks and its row of `ops.loads` (MG-REQ-0413,
//! MG-REQ-0415.3, MG-REQ-0418.1), and the row of a load that failed before any commit.

use std::collections::BTreeSet;

use arrow::array::{RecordBatch, RecordBatchIterator};
use arrow::compute::concat_batches;
use arrow::datatypes::SchemaRef;
use mergus_adapter::{StorageAdapter, VersionRef, WriteMode};
use mergus_ports::Timestamp;
use mergus_source::{IncrementalReader, LateRows, watermark_of};

use crate::RawError;
use crate::chunk_load::{LoadOutcome, Stores, TableLoad, load_rows, rows_error};
use crate::chunk_rows::{Current, count, deleted_flags, stamped};
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
    /// The `deletes_checked_at` of the table's last recorded load.
    pub(crate) checked: Option<Timestamp>,
    /// Whether it resumes an unfinished load.
    pub(crate) resumed: bool,
}

/// The source schema, the raw schema and the current rows of a load that read a schema.
pub(crate) type Shape = (SchemaRef, SchemaRef, Current);

/// Ends the load of `step`: appends the deletes of a due `compare` to `changes`, rewrites
/// `current` from the load's rows in the same commit, then records the load in `ops.loads`
/// with its watermark (MG-REQ-0418.1, MG-REQ-0413).
pub(crate) fn finish(
    stores: &mut Stores<'_>,
    reader: &mut IncrementalReader<'_>,
    step: &Step<'_>,
    shape: Option<Shape>,
    raw_version: Option<VersionRef>,
    changes: &str,
    current: &str,
) -> Result<LoadOutcome, LoadError> {
    let load = step.load;
    let Some((source, schema, rows)) = shape else {
        let record = record(
            stores,
            step,
            0,
            step.before.clone(),
            step.checked,
            LoadResult::Ok,
        );
        let ops_version = record_load(stores.ops, &record).map_err(RawError::Registry)?;
        return Ok(outcome(step, raw_version, ops_version, 0, None));
    };
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
        write(stores.raw, changes, WriteMode::Append, deletes.clone())?;
    }
    write(stores.raw, current, WriteMode::Replace, new_current)?;
    let raw_version = Some(stores.raw.commit().map_err(RawError::Storage)?);
    let all =
        concat_batches(&schema, &[loaded, deletes]).map_err(|error| rows_error(load, error))?;
    let checked = if compare {
        Some(step.extracted_at)
    } else {
        step.checked
    };
    recorded(stores, step, raw_version, &all, checked, late)
}

/// Records the finished load of `step`, whose rows in `__changes` are `all`, in
/// `ops.loads`, with the largest incremental value of `all` as `watermark_after`, or the
/// previous watermark when none has a value (MG-REQ-0412.1, MG-REQ-0413).
fn recorded(
    stores: &mut Stores<'_>,
    step: &Step<'_>,
    raw_version: Option<VersionRef>,
    all: &RecordBatch,
    checked: Option<Timestamp>,
    late: Option<LateRows>,
) -> Result<LoadOutcome, LoadError> {
    let load = step.load;
    let watermark = match &load.table.incremental {
        Some(column) => watermark_of(std::slice::from_ref(all), column)
            .map_err(mergus_source::ExtractError::from)?
            .or_else(|| step.before.clone()),
        None => None,
    };
    let rows = count(all);
    let record = record(
        stores,
        step,
        rows,
        watermark.clone(),
        checked,
        LoadResult::Ok,
    );
    let ops_version = record_load(stores.ops, &record).map_err(RawError::Registry)?;
    let mut outcome = outcome(step, raw_version, ops_version, rows, watermark);
    outcome.late = late;
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
        step.checked,
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
