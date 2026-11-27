//! The source impact `mergus plan` shows per source table (MG-REQ-0432): the estimated rows
//! to read, the index used when known, the window and the load budget.

use std::collections::BTreeMap;
use std::fmt;

use mergus_ports::{PortError, SqlValue};
use mergus_project::Source;

use crate::care::{Budget, Load, SourceCare};
use crate::engine::{AttachedSource, DatabaseEngine};
use crate::index::{Index, SCHEMA, integer_at, schema_indexes, text_at};
use crate::table::{DeclaredTable, declared_tables};
use crate::window::Window;

/// The PostgreSQL query of the row estimate of every table of the server; `-1` when never
/// analyzed.
pub const PG_ESTIMATES: &str = "SELECT n.nspname::text AS schema_name, c.relname::text AS table_name, \
     c.reltuples::bigint AS estimate \
     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
     WHERE c.relkind IN ('r', 'p')";

/// The DuckDB query of the row estimates of every table of one PostgreSQL schema, read once
/// per source from the answer of [`PG_ESTIMATES`], with the parameters alias,
/// [`PG_ESTIMATES`] and schema.
pub const SCHEMA_ESTIMATES: &str = "SELECT table_name, estimate FROM postgres_query(?, ?) \
     WHERE schema_name = ?";

/// The row estimates of every table of one schema of an attached DuckDB file.
const DUCKDB_ESTIMATES: &str = "SELECT table_name, CAST(estimated_size AS BIGINT) \
     FROM duckdb_tables() WHERE database_name = ? AND schema_name = ?";

/// The schema of the tables of a DuckDB file.
const DUCKDB_SCHEMA: &str = "main";

/// What reading one source table costs the source (MG-REQ-0432).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableImpact {
    /// The source name.
    pub source_name: String,
    /// The declared table.
    pub table: String,
    /// The rows the source estimates the table holds, the most a read reads, shown as
    /// "up to N" (MG-REQ-0432.1); `None` when unknown.
    pub estimated_rows: Option<u64>,
    /// The index the read uses: one that starts with the incremental column of an
    /// incremental table, or with the key of a table read in chunks; `None` when unknown.
    pub index: Option<String>,
    /// The source's window.
    pub window: Option<Window>,
    /// The source's load.
    pub load: Load,
    /// The source's budget.
    pub budget: Budget,
}

impl fmt::Display for TableImpact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rows = self
            .estimated_rows
            .map_or_else(|| "unknown".to_owned(), |rows| format!("up to {rows}"));
        let index = self.index.as_deref().unwrap_or("unknown");
        let window = self
            .window
            .map_or_else(|| "none".to_owned(), |window| window.to_string());
        write!(
            formatter,
            "source {}.{}: estimated rows {rows}, index {index}, window {window}, load {} \
             ({} connections, {} rows per chunk, pause {} ms)",
            self.source_name,
            self.table,
            self.load,
            self.budget.connections,
            self.budget.chunk_rows,
            self.budget.pause.as_millis()
        )
    }
}

/// The impact of reading every table `source` declares, in the order of their names. The
/// estimates and indexes are read from `attached` when it is given, once for the whole
/// source; without it they are unknown.
///
/// # Errors
///
/// Fails with the [`PortError`] of a query that fails.
pub fn source_impact(
    attached: Option<&mut AttachedSource>,
    source: &Source,
    care: &SourceCare,
) -> Result<Vec<TableImpact>, PortError> {
    let (estimates, indexes) = match attached {
        Some(attached) => catalog(attached)?,
        None => (BTreeMap::new(), BTreeMap::new()),
    };
    let impacts = declared_tables(source)
        .into_iter()
        .map(|table| TableImpact {
            source_name: source.name.clone(),
            estimated_rows: estimates.get(&table.name).copied(),
            index: used_index(indexes.get(&table.name), &table),
            table: table.name,
            window: care.window,
            load: care.load,
            budget: care.budget(),
        })
        .collect();
    Ok(impacts)
}

/// The estimates of the source, by table, and its indexes, by table: two queries for
/// PostgreSQL, one for a DuckDB file, whose indexes are not read.
type Catalog = (BTreeMap<String, u64>, BTreeMap<String, Vec<Index>>);

/// Reads the [`Catalog`] of `attached`.
fn catalog(attached: &mut AttachedSource) -> Result<Catalog, PortError> {
    let alias = SqlValue::Text(attached.alias.clone());
    let (sql, params) = match attached.identity.engine {
        DatabaseEngine::PostgreSql => (
            SCHEMA_ESTIMATES,
            vec![
                alias,
                SqlValue::Text(PG_ESTIMATES.to_owned()),
                SqlValue::Text(SCHEMA.to_owned()),
            ],
        ),
        DatabaseEngine::DuckDb => (
            DUCKDB_ESTIMATES,
            vec![alias, SqlValue::Text(DUCKDB_SCHEMA.to_owned())],
        ),
    };
    let mut estimates = BTreeMap::new();
    for batch in &attached.connection.query_arrow(sql, &params)? {
        for row in 0..batch.num_rows() {
            let table = text_at(batch, 0, row);
            let rows = integer_at(batch, 1, row).and_then(|rows| u64::try_from(rows).ok());
            if let (Some(table), Some(rows)) = (table, rows) {
                estimates.insert(table, rows);
            }
        }
    }
    let indexes = if attached.identity.engine == DatabaseEngine::PostgreSql {
        schema_indexes(attached)?
    } else {
        BTreeMap::new()
    };
    Ok((estimates, indexes))
}

/// The index a read of `table` uses: one led by its incremental column, or by its key.
fn used_index(indexes: Option<&Vec<Index>>, table: &DeclaredTable) -> Option<String> {
    let columns = match &table.incremental {
        Some(column) => vec![column.clone()],
        None => table.key.clone(),
    };
    if columns.is_empty() {
        return None;
    }
    indexes?
        .iter()
        .find(|index| index.starts_with(&columns))
        .map(|index| index.name.clone())
}
