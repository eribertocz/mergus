//! The sandboxed session a macro file runs in (MG-REQ-0641.1): a fresh in-memory DuckDB with
//! external access off and the configuration locked.

use std::collections::BTreeSet;

use duckdb::{Config, Connection};

use super::failed;
use crate::{CatalogMacro, MacroRun, PortError};

/// The built-in functions: the `internal = true` rows of `duckdb_functions()`
/// (MG-REQ-0642.2).
const BUILTINS: &str =
    "SELECT DISTINCT function_name FROM duckdb_functions() WHERE internal ORDER BY 1";

/// Each catalog function MG-REQ-0641.2 compares, with the query that gives its rows as text.
const CATALOG: [(&str, &str); 6] = [
    (
        "duckdb_tables",
        "SELECT CAST(t AS VARCHAR) FROM duckdb_tables() AS t",
    ),
    (
        "duckdb_views",
        "SELECT CAST(t AS VARCHAR) FROM duckdb_views() AS t",
    ),
    (
        "duckdb_sequences",
        "SELECT CAST(t AS VARCHAR) FROM duckdb_sequences() AS t",
    ),
    (
        "duckdb_types",
        "SELECT CAST(t AS VARCHAR) FROM duckdb_types() AS t",
    ),
    (
        "duckdb_indexes",
        "SELECT CAST(t AS VARCHAR) FROM duckdb_indexes() AS t",
    ),
    (
        "duckdb_schemas",
        "SELECT CAST(t AS VARCHAR) FROM duckdb_schemas() AS t",
    ),
];

/// The macros of the session (MG-REQ-0641.3).
const MACROS: &str = "SELECT function_name, function_type = 'table_macro', \
     coalesce(macro_definition, '') FROM duckdb_functions() \
     WHERE NOT internal AND function_type IN ('macro', 'table_macro') ORDER BY 1, 3";

/// Runs `script` in a fresh session opened with `config`, external access off and the
/// configuration locked, and returns what the session shows.
pub(super) fn run(config: Config, script: &str) -> Result<MacroRun, PortError> {
    let config = config
        .enable_external_access(false)
        .and_then(|config| config.with("lock_configuration", "true"))
        .map_err(failed)?;
    let connection = Connection::open_in_memory_with_flags(config).map_err(failed)?;
    let builtins = texts(&connection, BUILTINS)?;
    let before = catalog(&connection)?;
    let refused = connection
        .execute_batch(script)
        .err()
        .map(|error| error.to_string());
    let after = catalog(&connection)?;
    let changed = before.symmetric_difference(&after).cloned().collect();
    let mut statement = connection.prepare(MACROS).map_err(failed)?;
    let macros = statement
        .query_map([], |row| {
            Ok(CatalogMacro {
                name: row.get(0)?,
                table: row.get(1)?,
                definition: row.get(2)?,
            })
        })
        .and_then(Iterator::collect)
        .map_err(failed)?;
    Ok(MacroRun {
        builtins,
        refused,
        changed,
        macros,
    })
}

/// The rows of every catalog function of [`CATALOG`], each as `<function>: <row>`.
fn catalog(connection: &Connection) -> Result<BTreeSet<String>, PortError> {
    let mut rows = BTreeSet::new();
    for (function, sql) in CATALOG {
        for row in texts(connection, sql)? {
            rows.insert(format!("{function}: {row}"));
        }
    }
    Ok(rows)
}

/// The first column of every row of `sql`, as text.
fn texts(connection: &Connection, sql: &str) -> Result<Vec<String>, PortError> {
    let mut statement = connection.prepare(sql).map_err(failed)?;
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .and_then(Iterator::collect)
        .map_err(failed)
}
