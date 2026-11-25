//! The SQL text of the adapter. Values are always bound; identifiers are quoted by
//! [`quote`] (MG-REQ-0007).

use arrow::datatypes::{DataType, Schema};

use crate::DuckLakeError;

/// The Arrow types the adapter stores exactly, with their DuckDB type.
pub(crate) const TYPES: [(DataType, &str); 14] = [
    (DataType::Boolean, "BOOLEAN"),
    (DataType::Int8, "TINYINT"),
    (DataType::Int16, "SMALLINT"),
    (DataType::Int32, "INTEGER"),
    (DataType::Int64, "BIGINT"),
    (DataType::UInt8, "UTINYINT"),
    (DataType::UInt16, "USMALLINT"),
    (DataType::UInt32, "UINTEGER"),
    (DataType::UInt64, "UBIGINT"),
    (DataType::Float32, "FLOAT"),
    (DataType::Float64, "DOUBLE"),
    (DataType::Utf8, "VARCHAR"),
    (DataType::Binary, "BLOB"),
    (DataType::Date32, "DATE"),
];

/// Whether `snapshot_id` names a snapshot of the catalog bound first.
pub(crate) const SNAPSHOT_EXISTS: &str =
    "SELECT count(*) AS found FROM ducklake_snapshots(?) WHERE snapshot_id = ?";
/// The newest snapshot of the catalog bound first.
pub(crate) const LATEST: &str = "SELECT max(snapshot_id) AS snapshot_id FROM ducklake_snapshots(?)";
/// Every snapshot of the catalog bound first, with its time in microseconds since the epoch.
pub(crate) const SNAPSHOTS: &str = "SELECT snapshot_id, epoch_us(snapshot_time) AS micros \
                                    FROM ducklake_snapshots(?) ORDER BY snapshot_id";
/// Expires one snapshot of a catalog.
pub(crate) const EXPIRE: &str = "CALL ducklake_expire_snapshots(?, versions => [?])";
/// Deletes the files of expired snapshots of a catalog.
pub(crate) const CLEANUP: &str = "CALL ducklake_cleanup_old_files(?, cleanup_all => true)";
pub(crate) const BEGIN: &str = "BEGIN TRANSACTION";
pub(crate) const COMMIT: &str = "COMMIT";
pub(crate) const ROLLBACK: &str = "ROLLBACK";

/// Quotes `identifier` for DuckDB: in double quotes, with each double quote doubled.
pub(crate) fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

/// The statement that creates the staging table `stage` with the columns of `schema`.
///
/// # Errors
///
/// Fails with [`DuckLakeError::UnsupportedType`] for a column of a type not in [`TYPES`].
pub(crate) fn create_stage(stage: &str, schema: &Schema) -> Result<String, DuckLakeError> {
    let mut columns = Vec::new();
    for field in schema.fields() {
        let duckdb_type = TYPES
            .iter()
            .find(|(data_type, _)| data_type == field.data_type())
            .map(|(_, duckdb_type)| *duckdb_type)
            .ok_or_else(|| DuckLakeError::UnsupportedType {
                column: field.name().clone(),
                data_type: field.data_type().clone(),
            })?;
        columns.push(format!("{} {duckdb_type}", quote(field.name())));
    }
    let stage = quote(stage);
    let columns = columns.join(", ");
    // sql: identifiers quoted by quote
    Ok(format!("CREATE TABLE {stage} ({columns})"))
}

/// `table` of the catalog `alias`, quoted.
pub(crate) fn qualified(alias: &str, table: &str) -> String {
    format!("{}.{}", quote(alias), quote(table))
}

/// The statement that creates `target` with the columns of `stage` when it does not exist.
pub(crate) fn create_target(target: &str, stage: &str) -> String {
    // sql: identifiers quoted by quote
    format!("CREATE TABLE IF NOT EXISTS {target} AS SELECT * FROM {stage} LIMIT 0")
}

/// The statement that deletes every row of `target`.
pub(crate) fn delete_all(target: &str) -> String {
    // sql: identifiers quoted by quote
    format!("DELETE FROM {target}")
}

/// The statement that inserts the rows of `stage` into `target`, matching columns by name.
pub(crate) fn insert(target: &str, stage: &str) -> String {
    // sql: identifiers quoted by quote
    format!("INSERT INTO {target} BY NAME SELECT * FROM {stage}")
}

/// The statement that drops the staging table `stage`.
pub(crate) fn drop_stage(stage: &str) -> String {
    // sql: identifiers quoted by quote
    format!("DROP TABLE {stage}")
}

/// The query that reads `target` at the snapshot bound first.
pub(crate) fn read_at(target: &str) -> String {
    // sql: identifiers quoted by quote
    format!("SELECT * FROM {target} AT (VERSION => ?)")
}
