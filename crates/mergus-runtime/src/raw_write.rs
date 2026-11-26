//! Writing one load into its raw tables (MG-REQ-0511, MG-REQ-0512, MG-REQ-0518).

use std::collections::BTreeSet;

use arrow::array::{BooleanArray, RecordBatch, RecordBatchIterator};
use arrow::compute::{concat_batches, filter_record_batch};
use arrow::datatypes::SchemaRef;
use arrow::error::ArrowError;
use arrow::row::OwnedRow;
use mergus_adapter::{BatchStream, StorageAdapter, VersionRef, WriteMode};
use mergus_ports::Timestamp;
use mergus_source::DeclaredTable;

use crate::RawError;
use crate::raw_open::RAW_ALIAS;
use crate::raw_rows::{Keys, Metadata, conform, raw_schema, with_metadata};

/// The rows one load extracted from one declared table, and what it detected deleted.
#[derive(Clone, Debug)]
pub struct RawLoad {
    /// The source's name.
    pub source: String,
    /// The declared table, with its key.
    pub table: DeclaredTable,
    /// The load's id in the load registry, `_load_id` (MG-REQ-0512.1).
    pub load_id: String,
    /// When the load's extraction started, `_extracted_at` (MG-REQ-0512.2).
    pub extracted_at: Timestamp,
    /// The schema of the rows, with the source's columns and their native types.
    pub schema: SchemaRef,
    /// The rows extracted, in the order read.
    pub rows: Vec<RecordBatch>,
    /// The keys of the rows detected deleted in the source, as batches holding at least the
    /// key columns. Ignored for a table without `key`.
    pub deleted: Vec<RecordBatch>,
}

/// The two raw tables of a load, ready to write.
struct Tables {
    changes: RecordBatch,
    current: RecordBatch,
}

/// Writes `load` into its raw tables through `adapter`, in one commit (MG-REQ-0518.1): its rows
/// are appended to `raw.<source>__<table>__changes` and `raw.<source>__<table>` is replaced by
/// the current rows. Both keep the source's columns with their native types (MG-REQ-0511),
/// followed by `_load_id`, `_extracted_at`, `_deleted` and `_deleted_at` (MG-REQ-0512).
///
/// For a table with `key`, the current rows are those of the current table at `previous`,
/// the version of the last commit of this table, `None` for its first load, without the keys
/// the load returned or detected deleted, and with the load's rows. A detected delete of a
/// current row is appended to `__changes` with that row's values (MG-REQ-0518.2). For a table
/// without `key`, the current rows are the load's.
///
/// Returns the version of the commit.
///
/// # Errors
///
/// - [`RawError::RepeatedKey`] when the load returns a key twice; nothing is written
///   (MG-REQ-0518.3);
/// - [`RawError::Rows`] when the rows do not match `load.schema` or lack a key column;
/// - [`RawError::Storage`] when the adapter fails.
pub fn write_raw(
    adapter: &mut dyn StorageAdapter,
    load: &RawLoad,
    previous: Option<&VersionRef>,
) -> Result<VersionRef, RawError> {
    let names = load.table.raw(&load.source);
    let current_table = in_catalog(&names.current);
    let tables = tables(adapter, load, previous, &current_table)?;
    adapter.write(
        &in_catalog(&names.changes),
        WriteMode::Append,
        stream(tables.changes),
    )?;
    adapter.write(&current_table, WriteMode::Replace, stream(tables.current))?;
    Ok(adapter.commit()?)
}

/// The name of the raw table `name`, `raw.<table>`, inside the raw catalog.
fn in_catalog(name: &str) -> String {
    name.strip_prefix(RAW_ALIAS)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(name)
        .to_owned()
}

/// `batch` as a stream.
fn stream(batch: RecordBatch) -> BatchStream {
    let schema = batch.schema();
    Box::new(RecordBatchIterator::new([Ok(batch)], schema))
}

/// The rows of `__changes` and of the current table of `load`.
fn tables(
    adapter: &dyn StorageAdapter,
    load: &RawLoad,
    previous: Option<&VersionRef>,
    current_table: &str,
) -> Result<Tables, RawError> {
    let rows = |error: ArrowError| RawError::Rows {
        table: load.table.name.clone(),
        error,
    };
    let schema = raw_schema(&load.schema);
    let metadata = Metadata {
        load_id: load.load_id.clone(),
        extracted_at: micros(load.extracted_at),
    };
    let count = load.schema.fields().len();
    let mut loaded = Vec::new();
    for batch in &load.rows {
        let batch = conform(batch, &load.schema).map_err(rows)?;
        loaded.push(with_metadata(&batch, count, &schema, &metadata, false).map_err(rows)?);
    }
    let loaded = concat_batches(&schema, &loaded).map_err(rows)?;
    if load.table.key.is_empty() {
        return Ok(Tables {
            changes: loaded.clone(),
            current: loaded,
        });
    }
    let keys = Keys::new(&schema, &load.table.key).map_err(rows)?;
    let returned = unique_keys(&keys, &loaded, load)?;
    let before = match previous {
        Some(version) => read(adapter, current_table, version, &schema)?,
        None => RecordBatch::new_empty(schema.clone()),
    };
    let deleted = keys.set(&load.deleted).map_err(rows)?;
    let before_keys = keys.of(&before).map_err(rows)?;
    let mask = |keep: &dyn Fn(&OwnedRow) -> bool| -> BooleanArray {
        before_keys.iter().map(|key| Some(keep(key))).collect()
    };
    let kept = mask(&|key| !returned.contains(key) && !deleted.contains(key));
    let gone = mask(&|key| !returned.contains(key) && deleted.contains(key));
    let kept = filter_record_batch(&before, &kept).map_err(rows)?;
    let gone = filter_record_batch(&before, &gone).map_err(rows)?;
    let gone = with_metadata(&gone, count, &schema, &metadata, true).map_err(rows)?;
    Ok(Tables {
        changes: concat_batches(&schema, &[loaded.clone(), gone]).map_err(rows)?,
        current: concat_batches(&schema, &[kept, loaded]).map_err(rows)?,
    })
}

/// The keys of `loaded`, which must all differ (MG-REQ-0518.3).
fn unique_keys(
    keys: &Keys,
    loaded: &RecordBatch,
    load: &RawLoad,
) -> Result<BTreeSet<OwnedRow>, RawError> {
    let rows = |error: ArrowError| RawError::Rows {
        table: load.table.name.clone(),
        error,
    };
    let mut seen = BTreeSet::new();
    for (row, key) in keys.of(loaded).map_err(rows)?.into_iter().enumerate() {
        if !seen.insert(key) {
            return Err(RawError::RepeatedKey {
                source_name: load.source.clone(),
                table: load.table.name.clone(),
                key: keys.show(loaded, row).map_err(rows)?,
            });
        }
    }
    Ok(seen)
}

/// The rows of `table` at `version`, with the columns of `schema`.
fn read(
    adapter: &dyn StorageAdapter,
    table: &str,
    version: &VersionRef,
    schema: &SchemaRef,
) -> Result<RecordBatch, RawError> {
    let rows = |error: ArrowError| RawError::Rows {
        table: table.to_owned(),
        error,
    };
    let mut batches = Vec::new();
    for batch in adapter.read_at(table, version)? {
        batches.push(conform(&batch.map_err(rows)?, schema).map_err(rows)?);
    }
    concat_batches(schema, &batches).map_err(rows)
}

/// `at` in microseconds since the epoch, saturated to the range of `i64`.
fn micros(at: Timestamp) -> i64 {
    let micros = at.unix_nanos().div_euclid(1_000);
    i64::try_from(micros).unwrap_or(if micros < 0 { i64::MIN } else { i64::MAX })
}
