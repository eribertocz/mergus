//! Reading the load registry, `ops.loads` (MG-REQ-0514): the last load of a table and the
//! loads recorded, which tell finished loads from unfinished ones (MG-REQ-0418.2).

use std::collections::BTreeSet;

use arrow::array::{Array, AsArray};
use arrow::compute::cast;
use arrow::datatypes::TimestampMicrosecondType;
use arrow::error::ArrowError;
use mergus_adapter::{AdapterError, StorageAdapter, VersionRef};
use mergus_ports::Timestamp;

use crate::RawError;
use crate::registry::{LOADS, micros, utc};

/// The last recorded load of a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastLoad {
    /// Its load id.
    pub load_id: String,
    /// Its `watermark_after`, where the next load starts.
    pub watermark_after: Option<String>,
    /// Its `deletes_checked_at`: when the last `compare` of the table ran (MG-REQ-0415.3).
    pub deletes_checked_at: Option<Timestamp>,
}

/// The loads of one table recorded in `ops.loads`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableLoads {
    /// The row with the greatest `started_at`, then the greatest `load_id`; `None` when no
    /// load of the table is recorded.
    pub last: Option<LastLoad>,
    /// The id of every recorded load of the table, finished or failed; a load id of
    /// `__changes` that is not here is unfinished (MG-REQ-0418.2).
    pub recorded: BTreeSet<String>,
}

/// The loads of `table` of `source` recorded in `ops.loads` at `version`, whatever order the
/// rows are read in. A registry written before `deletes_checked_at` existed reads it as
/// `None`.
///
/// # Errors
///
/// Returns [`RawError::Registry`] when `ops.loads` cannot be read at `version`.
pub fn table_loads(
    ops: &dyn StorageAdapter,
    version: &VersionRef,
    source: &str,
    table: &str,
) -> Result<TableLoads, RawError> {
    let mut loads = TableLoads::default();
    let mut last: Option<(i64, LastLoad)> = None;
    for batch in ops.read_at(LOADS, version).map_err(RawError::Registry)? {
        let batch = batch.map_err(unreadable)?;
        let text = |name: &str| {
            batch
                .column_by_name(name)
                .and_then(|column| column.as_string_opt::<i32>().cloned())
                .ok_or_else(|| missing(name))
        };
        let (sources, tables, ids, marks) = (
            text("source")?,
            text("table")?,
            text("load_id")?,
            text("watermark_after")?,
        );
        let started = times(&batch, "started_at")?.ok_or_else(|| missing("started_at"))?;
        let checked = times(&batch, "deletes_checked_at")?;
        for index in 0..batch.num_rows() {
            if sources.value(index) != source || tables.value(index) != table {
                continue;
            }
            let id = ids.value(index).to_owned();
            loads.recorded.insert(id.clone());
            let row = LastLoad {
                load_id: id,
                watermark_after: (!marks.is_null(index)).then(|| marks.value(index).to_owned()),
                deletes_checked_at: checked.as_ref().and_then(|checked| checked[index]),
            };
            let at = started[index].map_or(i64::MIN, micros);
            if last
                .as_ref()
                .is_none_or(|(known, load)| (at, &row.load_id) > (*known, &load.load_id))
            {
                last = Some((at, row));
            }
        }
    }
    loads.last = last.map(|(_, load)| load);
    Ok(loads)
}

/// The timestamps of the column `name` of `batch`, `None` when it has no such column.
fn times(
    batch: &arrow::array::RecordBatch,
    name: &str,
) -> Result<Option<Vec<Option<Timestamp>>>, RawError> {
    let Some(column) = batch.column_by_name(name) else {
        return Ok(None);
    };
    let column = cast(column, &utc()).map_err(unreadable)?;
    let column = column.as_primitive::<TimestampMicrosecondType>();
    Ok(Some(
        column
            .iter()
            .map(|at| at.map(|at| Timestamp::from_unix_nanos(i128::from(at) * 1_000)))
            .collect(),
    ))
}

/// A failure to read `ops.loads`.
fn unreadable(error: ArrowError) -> RawError {
    RawError::Registry(AdapterError::Storage(Box::new(error)))
}

/// `ops.loads` read without the column `name`.
fn missing(name: &str) -> RawError {
    unreadable(ArrowError::SchemaError(format!("no column `{name}`")))
}
