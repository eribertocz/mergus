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
/// column of `table` in the DuckDB type of its declared type, or, with `widen`, in a type
/// every value of which the declared type holds exactly ([`widens`]); the rows are then
/// converted, value by value, when they are read. `source` names what is described in errors,
/// such as "the Parquet file".
pub(crate) fn described(
    session: &mut dyn DatabaseConnection,
    describe: &str,
    params: &[SqlValue],
    table: &Table,
    (source, widen): (&str, bool),
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
            Some((_, actual)) if widen && widens(actual, &declared) => continue,
            Some((_, actual)) => format!("{source} holds {actual}, the schema declares {declared}"),
            None => format!("{source} has no such column"),
        };
        return Err(unrepresentable(&table.name, field.name(), reason));
    }
    Ok(())
}

/// Returns whether every value of the DuckDB type `actual` is held exactly by the DuckDB type
/// `declared`: an integer by a wider integer (an unsigned one by a strictly wider signed one),
/// `FLOAT` by `DOUBLE`, and `DECIMAL(p,s)` by `DECIMAL(p',s)` with `p' >= p`. Any other pair,
/// a narrowing or a change of scale among them, does not widen.
pub(crate) fn widens(actual: &str, declared: &str) -> bool {
    if let (Some(from), Some(to)) = (integer(actual), integer(declared)) {
        return match (from, to) {
            ((false, from), (false, to)) | ((true, from), (true, to)) => to >= from,
            ((true, from), (false, to)) => to > from,
            ((false, _), (true, _)) => false,
        };
    }
    if let (Some((from_precision, from_scale)), Some((to_precision, to_scale))) =
        (decimal(actual), decimal(declared))
    {
        return from_scale == to_scale && to_precision >= from_precision;
    }
    actual == "FLOAT" && declared == "DOUBLE"
}

/// Returns whether the DuckDB integer type `name` is unsigned, and its width in bits.
fn integer(name: &str) -> Option<(bool, u8)> {
    let (unsigned, bits) = match name {
        "TINYINT" => (false, 8),
        "SMALLINT" => (false, 16),
        "INTEGER" => (false, 32),
        "BIGINT" => (false, 64),
        "UTINYINT" => (true, 8),
        "USMALLINT" => (true, 16),
        "UINTEGER" => (true, 32),
        "UBIGINT" => (true, 64),
        _ => return None,
    };
    Some((unsigned, bits))
}

/// Returns the precision and scale of the DuckDB type `DECIMAL(p,s)`.
fn decimal(name: &str) -> Option<(u8, u8)> {
    let (precision, scale) = name
        .strip_prefix("DECIMAL(")?
        .strip_suffix(')')?
        .split_once(',')?;
    Some((precision.trim().parse().ok()?, scale.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::widens;

    #[test]
    fn mg_req_0180_exact_widenings_are_accepted() {
        assert!(widens("INTEGER", "BIGINT"));
        assert!(widens("TINYINT", "SMALLINT"));
        assert!(widens("UINTEGER", "BIGINT"));
        assert!(widens("UINTEGER", "UBIGINT"));
        assert!(widens("DECIMAL(10,2)", "DECIMAL(14,2)"));
        assert!(widens("FLOAT", "DOUBLE"));
    }

    #[test]
    fn mg_req_0180_narrowings_are_rejected() {
        assert!(!widens("BIGINT", "INTEGER"));
        assert!(!widens("UINTEGER", "INTEGER"));
        assert!(!widens("INTEGER", "UBIGINT"));
        assert!(!widens("DECIMAL(14,2)", "DECIMAL(10,2)"));
        assert!(!widens("DECIMAL(10,2)", "DECIMAL(14,3)"));
        assert!(!widens("DOUBLE", "FLOAT"));
        assert!(!widens("INTEGER", "DECIMAL(18,0)"));
        assert!(!widens("VARCHAR", "BIGINT"));
    }
}
