//! The checks of a segment before it reads or writes a row: declared schemas DuckDB can hold
//! exactly, described columns of the declared types, and a session without `icu`
//! (MG-REQ-0180).

use std::collections::BTreeSet;

use arrow::array::AsArray;
use mergus_ports::{DatabaseConnection, SqlValue};

use crate::exact::{duckdb_type, unrepresentable};
use crate::{EngineError, Table};

/// The statement that lists a loaded `icu` extension: with it, DuckDB would give a
/// `TIMESTAMPTZ` back in the session's zone, which can be the host's, instead of UTC.
pub(crate) const LOADED_ICU: &str =
    "SELECT extension_name FROM duckdb_extensions() WHERE loaded AND extension_name = ?";

/// Checks that every column of `table` has a DuckDB type that holds it exactly, and that no
/// two columns share a name.
pub(crate) fn declared(table: &Table) -> Result<(), EngineError> {
    let mut names = BTreeSet::new();
    for field in table.schema.fields() {
        duckdb_type(field.data_type())
            .map_err(|reason| unrepresentable(&table.name, field.name(), reason))?;
        if !names.insert(field.name()) {
            let reason = "the schema declares the column twice".to_owned();
            return Err(unrepresentable(&table.name, field.name(), reason));
        }
    }
    Ok(())
}

/// Fails with [`EngineError::ZoneExtensionLoaded`] when the session has loaded `icu`.
pub(crate) fn without_icu(session: &mut dyn DatabaseConnection) -> Result<(), EngineError> {
    let icu = [SqlValue::Text("icu".to_owned())];
    let loaded = session.query_arrow(LOADED_ICU, &icu)?;
    if loaded.iter().any(|batch| batch.num_rows() > 0) {
        return Err(EngineError::ZoneExtensionLoaded);
    }
    Ok(())
}

/// Checks that the statement `describe`, a `DESCRIBE` run with `params`, gives every declared
/// column of `table` in the DuckDB type of its declared type. `source` names what is
/// described in errors, such as "the Parquet file".
pub(crate) fn described(
    session: &mut dyn DatabaseConnection,
    describe: &str,
    params: &[SqlValue],
    table: &Table,
    source: &str,
) -> Result<(), EngineError> {
    let mut found = Vec::new();
    for batch in &session.query_arrow(describe, params)? {
        let text = |name: &str| batch.column_by_name(name)?.as_string_opt::<i32>().cloned();
        let (Some(names), Some(types)) = (text("column_name"), text("column_type")) else {
            let reason = format!("DuckDB did not describe {source}");
            return Err(unrepresentable(&table.name, "", reason));
        };
        for (name, data_type) in names.iter().zip(types.iter()) {
            let (name, data_type) = (name.unwrap_or_default(), data_type.unwrap_or_default());
            found.push((name.to_owned(), data_type.to_owned()));
        }
    }
    for field in table.schema.fields() {
        let declared = duckdb_type(field.data_type())
            .map_err(|reason| unrepresentable(&table.name, field.name(), reason))?;
        let reason = match found.iter().find(|(name, _)| name == field.name()) {
            Some((_, actual)) if *actual == declared => continue,
            Some((_, actual)) => format!("{source} holds {actual}, the schema declares {declared}"),
            None => format!("{source} has no such column"),
        };
        return Err(unrepresentable(&table.name, field.name(), reason));
    }
    Ok(())
}
