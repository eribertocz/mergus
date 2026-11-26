//! DuckLake snapshots: reading their ids and times, and choosing which to expire.

use std::collections::BTreeSet;

use arrow::array::{AsArray, RecordBatch};
use arrow::datatypes::Int64Type;
use mergus_adapter::{Retention, VersionRef};
use mergus_ports::Timestamp;

use crate::DuckLakeError;

/// One snapshot of the catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    /// Its id, the adapter's [`VersionRef`].
    pub(crate) id: i64,
    /// When it was taken, in microseconds since the Unix epoch.
    pub(crate) micros: i64,
}

/// Returns the snapshot id that `version` names, if it names one.
pub(crate) fn snapshot_id(version: &VersionRef) -> Option<i64> {
    version.as_str().parse().ok()
}

/// Returns the non-null values of column `index` of `batches`, which must be `BIGINT`.
///
/// # Errors
///
/// Fails with [`DuckLakeError::UnexpectedRows`] naming `sql` when the column is missing or
/// not `BIGINT`.
pub(crate) fn int_column(
    batches: &[RecordBatch],
    index: usize,
    sql: &str,
) -> Result<Vec<i64>, DuckLakeError> {
    let mut values = Vec::new();
    for batch in batches {
        let column = (index < batch.num_columns())
            .then(|| batch.column(index).as_primitive_opt::<Int64Type>())
            .flatten()
            .ok_or_else(|| DuckLakeError::UnexpectedRows(sql.to_owned()))?;
        values.extend(column.iter().flatten());
    }
    Ok(values)
}

/// Returns the one non-null `BIGINT` of `batches`, or `None` when there is none.
///
/// # Errors
///
/// Fails with [`DuckLakeError::UnexpectedRows`] naming `sql` when the first column is not
/// `BIGINT` or holds more than one value.
pub(crate) fn single_int(batches: &[RecordBatch], sql: &str) -> Result<Option<i64>, DuckLakeError> {
    match int_column(batches, 0, sql)?.as_slice() {
        [] => Ok(None),
        [value] => Ok(Some(*value)),
        _ => Err(DuckLakeError::UnexpectedRows(sql.to_owned())),
    }
}

/// Reads the snapshots from the rows of [`crate::sql::SNAPSHOTS`].
///
/// # Errors
///
/// Fails with [`DuckLakeError::UnexpectedRows`] when the rows do not hold two `BIGINT`
/// columns of the same length.
pub(crate) fn from_rows(
    batches: &[RecordBatch],
    sql: &str,
) -> Result<Vec<Snapshot>, DuckLakeError> {
    let ids = int_column(batches, 0, sql)?;
    let micros = int_column(batches, 1, sql)?;
    if ids.len() != micros.len() {
        return Err(DuckLakeError::UnexpectedRows(sql.to_owned()));
    }
    Ok(ids
        .into_iter()
        .zip(micros)
        .map(|(id, micros)| Snapshot { id, micros })
        .collect())
}

/// Returns the ids of the snapshots `expire` may delete, in ascending order: never the newest
/// (the published version), one in [`Retention::live`], or one younger than
/// [`Retention::rollback_window`] at `now` (MG-REQ-0288).
pub(crate) fn expirable(snapshots: &[Snapshot], retention: &Retention, now: Timestamp) -> Vec<i64> {
    let live: BTreeSet<i64> = retention.live.iter().filter_map(snapshot_id).collect();
    let newest = snapshots.iter().map(|snapshot| snapshot.id).max();
    let window = i128::try_from(retention.rollback_window.as_nanos()).unwrap_or(i128::MAX);
    let cutoff = now.unix_nanos().saturating_sub(window);
    let mut ids: Vec<i64> = snapshots
        .iter()
        .filter(|snapshot| Some(snapshot.id) != newest && !live.contains(&snapshot.id))
        .filter(|snapshot| i128::from(snapshot.micros).saturating_mul(1_000) <= cutoff)
        .map(|snapshot| snapshot.id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}
