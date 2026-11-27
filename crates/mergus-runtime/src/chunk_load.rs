//! An incremental load of one table by chunks: each chunk is a commit to `__changes`, the
//! last commit rewrites the current table, and only then is the load recorded
//! (MG-REQ-0412, MG-REQ-0413, MG-REQ-0415, MG-REQ-0418, MG-REQ-0420).

use std::num::NonZeroU32;
use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchIterator};
use arrow::datatypes::{Schema, SchemaRef};
use arrow::error::ArrowError;
use mergus_adapter::{StorageAdapter, VersionRef, WriteMode};
use mergus_ports::{Clock, Random};
use mergus_project::Source;
use mergus_source::{AttachedSource, DeclaredTable, IncrementalReader, LateRows, Start};

use crate::chunk_rows::{Current, finished, read_or_none, rows_of, stamped, unfinished};
use crate::chunk_step::{Shape, Step, finish, record_failed};
use crate::load_error::LoadError;
use crate::raw_rows::{METADATA, Metadata, raw_schema};
use crate::registry::micros;
use crate::registry_read::{TableLoads, table_loads};
use crate::{RAW_ALIAS, RawError};

/// The stores and ports a load writes through.
pub struct Stores<'a> {
    /// The storage adapter of raw.
    pub raw: &'a mut dyn StorageAdapter,
    /// The storage adapter of the ops zone, which holds `ops.loads`.
    pub ops: &'a mut dyn StorageAdapter,
    /// The wall clock: `_extracted_at` and the duration of the load.
    pub clock: &'a dyn Clock,
    /// The bits of a new load id.
    pub random: &'a dyn Random,
}

/// One table to load.
#[derive(Clone, Debug)]
pub struct TableLoad<'a> {
    /// The source, as `mergus-project` validated it.
    pub source: &'a Source,
    /// The declared table.
    pub table: &'a DeclaredTable,
    /// The rows of one keyset chunk (MG-REQ-0417); `None` reads the table in one statement.
    pub chunk_rows: Option<NonZeroU32>,
    /// The latest version of raw; `None` before its first commit.
    pub raw_version: Option<VersionRef>,
    /// The latest version of the ops store; `None` before its first commit.
    pub ops_version: Option<VersionRef>,
}

/// What a load did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadOutcome {
    /// The load id, that of the unfinished load it resumed if any (MG-REQ-0418.2).
    pub load_id: String,
    /// Whether it resumed an unfinished load.
    pub resumed: bool,
    /// The latest version of raw after the load.
    pub raw_version: Option<VersionRef>,
    /// The version of the commit of its `ops.loads` row.
    pub ops_version: VersionRef,
    /// The rows it appended to `__changes`, deletes included.
    pub rows: i64,
    /// Its `watermark_after`.
    pub watermark: Option<String>,
    /// The very late rows a `compare` found, a warning (MG-REQ-0420).
    pub late: Option<LateRows>,
}

/// Loads `load.table` from `attached` into raw by chunks, and records it in `ops.loads`.
///
/// - The read starts at `watermark − overlap` of the last recorded load (MG-REQ-0412). Each
///   chunk appends to `__changes` the rows that differ from the current row of their key
///   (MG-REQ-0412.3) and the rows the delete column marks deleted (MG-REQ-0415.2), with the
///   load's `_load_id` and `_extracted_at`, in a commit of its own; the read moves past a
///   chunk only once it is committed (MG-REQ-0418.1).
/// - The last commit appends the deletes a due `compare` found (MG-REQ-0415.3) and rewrites
///   the current table from the load's rows. Only then is the `ops.loads` row written, with
///   the largest incremental value of the load's committed rows as `watermark_after`
///   (MG-REQ-0412.1, MG-REQ-0413).
/// - A load id of `__changes` with no `ops.loads` row is resumed with the same id, after its
///   largest key, when the table has a `key`; without one it is ignored and the table read
///   again whole (MG-REQ-0418.2, MG-REQ-0418.3).
///
/// A load that fails before any commit is recorded as `failed` with its code; one that fails
/// later is left unfinished for the next load to resume, not recorded as failed
/// (MG-REQ-0418.2).
///
/// # Errors
///
/// - [`LoadError::Source`] when the source cannot be read, `MG-0406` among them;
/// - [`LoadError::Raw`] when raw or `ops.loads` cannot be read or written.
pub fn load_table(
    stores: &mut Stores<'_>,
    attached: &mut AttachedSource,
    load: &TableLoad<'_>,
) -> Result<LoadOutcome, LoadError> {
    let names = load.table.raw(&load.source.name);
    let changes = in_catalog(&names.changes);
    let current = in_catalog(&names.current);
    let (step, resume_after) = prepare(stores, load, &changes)?;
    let start = Start {
        chunk_rows: load.chunk_rows,
        watermark: step.before.clone(),
        resume_after,
    };
    let mut reader = match IncrementalReader::open(attached, load.source, load.table, start) {
        Ok(reader) => reader,
        Err(error) => return Err(record_failed(stores, &step, step.resumed, error.into())),
    };
    let current_rows = read_or_none(stores.raw, &current, load.raw_version.as_ref())?;
    let (shape, raw_version) =
        read_chunks(stores, &mut reader, &step, current_rows.as_ref(), &changes)?;
    finish(
        stores,
        &mut reader,
        &step,
        shape,
        raw_version,
        &changes,
        &current,
    )
}

/// The load to run: a new one, or the unfinished one of `changes` to resume, with the key
/// its read resumes after (MG-REQ-0418.2).
fn prepare<'a>(
    stores: &Stores<'_>,
    load: &'a TableLoad<'a>,
    changes: &str,
) -> Result<(Step<'a>, Option<RecordBatch>), LoadError> {
    let loads = match &load.ops_version {
        Some(version) => registry(stores.ops, version, load)?,
        None => TableLoads::default(),
    };
    let history = read_or_none(stores.raw, changes, load.raw_version.as_ref())?;
    let resume = match (&history, load.table.key.is_empty()) {
        (Some(history), false) => unfinished(history, &loads.recorded, &load.table.key)
            .map_err(|error| rows_error(load, error))?,
        _ => None,
    };
    let last = loads.last.as_ref();
    let (load_id, extracted_at) = match &resume {
        Some(resume) => (resume.load_id.clone(), resume.extracted_at),
        None => (
            crate::new_load_id(stores.clock, stores.random),
            stores.clock.now(),
        ),
    };
    let step = Step {
        load,
        load_id,
        extracted_at,
        before: last.and_then(|last| last.watermark_after.clone()),
        checked: last.and_then(|last| last.deletes_checked_at),
        resumed: resume.is_some(),
    };
    Ok((step, resume.map(|resume| resume.last)))
}

/// Reads every chunk of `reader`, appending and committing what each changes to `changes`
/// before the read moves on (MG-REQ-0418.1). Returns the shape of the rows, `None` when no
/// schema is known, and the latest version of raw.
fn read_chunks(
    stores: &mut Stores<'_>,
    reader: &mut IncrementalReader<'_>,
    step: &Step<'_>,
    current_rows: Option<&RecordBatch>,
    changes: &str,
) -> Result<(Option<Shape>, Option<VersionRef>), LoadError> {
    let load = step.load;
    let mut raw_version = load.raw_version.clone();
    let mut committed = step.resumed;
    let mut shape: Option<Shape> = None;
    let metadata = Metadata {
        load_id: step.load_id.clone(),
        extracted_at: micros(step.extracted_at),
    };
    loop {
        let chunk = match reader.next_chunk() {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return Ok((shape, raw_version)),
            Err(error) => return Err(record_failed(stores, step, committed, error.into())),
        };
        if shape.is_none()
            && let Some(source) = source_schema(&chunk.rows, &chunk.deleted, current_rows)
        {
            let schema = raw_schema(&source);
            let count = source.fields().len();
            let rows = Current::new(current_rows, &schema, count, &load.table.key)
                .map_err(|error| rows_error(load, error))?;
            shape = Some((source, schema, rows));
        }
        if let Some((source, schema, rows)) = &shape {
            let appended = chunk_changes(load, rows, source, schema, &metadata, &chunk)
                .map_err(|error| rows_error(load, error))?;
            if appended.num_rows() > 0 {
                match append(stores.raw, changes, appended) {
                    Ok(version) => raw_version = Some(version),
                    Err(error) => {
                        return Err(record_failed(stores, step, committed, error.into()));
                    }
                }
                committed = true;
            }
        }
        reader.committed();
    }
}

/// The loads of the table recorded at `version` of the ops store; none when `ops.loads` does
/// not exist yet.
fn registry(
    ops: &dyn StorageAdapter,
    version: &VersionRef,
    load: &TableLoad<'_>,
) -> Result<TableLoads, RawError> {
    match table_loads(ops, version, &load.source.name, &load.table.name) {
        Err(RawError::Registry(mergus_adapter::AdapterError::VersionUnavailable { .. })) => {
            Ok(TableLoads::default())
        }
        other => other,
    }
}

/// The rows a chunk appends to `__changes`: for a keyed table, its rows that are not the
/// current row of their key (MG-REQ-0412.3) and its deleted rows whose key is current, as
/// deletes (MG-REQ-0415.2); for a table without `key`, every row read.
fn chunk_changes(
    load: &TableLoad<'_>,
    current: &Current,
    source: &SchemaRef,
    schema: &SchemaRef,
    metadata: &Metadata,
    chunk: &mergus_source::Chunk,
) -> Result<RecordBatch, ArrowError> {
    let rows = stamped(&chunk.rows, source, schema, metadata, false)?;
    let deleted = stamped(&chunk.deleted, source, schema, metadata, true)?;
    if load.table.key.is_empty() {
        return arrow::compute::concat_batches(schema, &[rows, deleted]);
    }
    let rows = current.changed(&rows)?;
    let deleted = current.present(&deleted)?;
    arrow::compute::concat_batches(schema, &[rows, deleted])
}

/// The schema of the source's columns: that of the rows read, else that of the current
/// table without its metadata columns.
fn source_schema(
    rows: &[RecordBatch],
    deleted: &[RecordBatch],
    current: Option<&RecordBatch>,
) -> Option<SchemaRef> {
    if let Some(batch) = rows.iter().chain(deleted).next() {
        return Some(batch.schema());
    }
    let current = current?;
    let fields = current
        .schema()
        .fields()
        .iter()
        .filter(|field| !METADATA.contains(&field.name().as_str()))
        .cloned()
        .collect::<Vec<_>>();
    Some(Arc::new(Schema::new(fields)))
}

/// Appends `rows` to `table` through `raw` and commits.
pub(crate) fn append(
    raw: &mut dyn StorageAdapter,
    table: &str,
    rows: RecordBatch,
) -> Result<VersionRef, RawError> {
    let schema = rows.schema();
    raw.write(
        table,
        WriteMode::Append,
        Box::new(RecordBatchIterator::new([Ok(rows)], schema)),
    )?;
    Ok(raw.commit()?)
}

/// The rows of the load `load_id` in `__changes` at `version`.
pub(crate) fn load_rows(
    raw: &dyn StorageAdapter,
    changes: &str,
    version: Option<&VersionRef>,
    load_id: &str,
) -> Result<Option<RecordBatch>, RawError> {
    let Some(history) = read_or_none(raw, changes, version)? else {
        return Ok(None);
    };
    let rows = rows_of(&history, load_id).map_err(|error| RawError::Rows {
        table: changes.to_owned(),
        error,
    })?;
    Ok(Some(rows))
}

/// An Arrow failure on the rows of `load`.
pub(crate) fn rows_error(load: &TableLoad<'_>, error: ArrowError) -> RawError {
    RawError::Rows {
        table: load.table.name.clone(),
        error,
    }
}

/// The name of the raw table `name`, `raw.<table>`, inside the raw catalog.
fn in_catalog(name: &str) -> String {
    name.strip_prefix(RAW_ALIAS)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(name)
        .to_owned()
}

/// The rows of `raw.<source>__<table>__changes` at `raw_version` that readers see: those of
/// the loads recorded in `ops.loads` at `ops_version`. The rows of an unfinished load are
/// ignored (MG-REQ-0518.1, MG-REQ-0418.3). `None` when the table has no row yet.
///
/// # Errors
///
/// Returns [`RawError`] when raw or `ops.loads` cannot be read.
pub fn finished_changes(
    raw: &dyn StorageAdapter,
    ops: &dyn StorageAdapter,
    source_name: &str,
    table: &DeclaredTable,
    raw_version: Option<&VersionRef>,
    ops_version: Option<&VersionRef>,
) -> Result<Option<RecordBatch>, RawError> {
    let changes = in_catalog(&table.raw(source_name).changes);
    let Some(history) = read_or_none(raw, &changes, raw_version)? else {
        return Ok(None);
    };
    let recorded = match ops_version {
        Some(version) => match table_loads(ops, version, source_name, &table.name) {
            Err(RawError::Registry(mergus_adapter::AdapterError::VersionUnavailable {
                ..
            })) => TableLoads::default(),
            other => other?,
        },
        None => TableLoads::default(),
    }
    .recorded;
    let rows = finished(&history, &recorded).map_err(|error| RawError::Rows {
        table: changes,
        error,
    })?;
    Ok(Some(rows))
}
