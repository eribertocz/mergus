//! The source impact `mergus plan` shows per source table (MG-REQ-0432): the estimated rows
//! to read, the index used when known, the window and the load budget.

use std::fmt;

use mergus_ports::{PortError, SqlValue};
use mergus_project::Source;

use crate::care::{Budget, Load, SourceCare};
use crate::engine::{AttachedSource, DatabaseEngine};
use crate::index::{SCHEMA, integer_at, table_indexes};
use crate::table::{DeclaredTable, declared_tables};
use crate::window::Window;

/// The PostgreSQL query of the row estimate of every table of the server; `-1` when never
/// analyzed.
pub const PG_ESTIMATES: &str = "SELECT n.nspname::text AS schema_name, c.relname::text AS table_name, \
     c.reltuples::bigint AS estimate \
     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
     WHERE c.relkind IN ('r', 'p')";

/// The DuckDB query of the row estimate of one PostgreSQL table, read from the answer of
/// [`PG_ESTIMATES`], with the parameters alias, [`PG_ESTIMATES`], schema and table.
pub const PG_TABLE_ESTIMATE: &str = "SELECT estimate FROM postgres_query(?, ?) \
     WHERE schema_name = ? AND table_name = ?";

/// The row estimate of one table of an attached DuckDB file.
const DUCKDB_TABLE_ESTIMATE: &str = "SELECT CAST(estimated_size AS BIGINT) FROM duckdb_tables() \
     WHERE database_name = ? AND schema_name = ? AND table_name = ?";

/// The schema of the tables of a DuckDB file.
const DUCKDB_SCHEMA: &str = "main";

/// What reading one source table costs the source (MG-REQ-0432).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableImpact {
    /// The source name.
    pub source_name: String,
    /// The declared table.
    pub table: String,
    /// The rows the source estimates the table holds, the most a read reads; `None` when
    /// unknown.
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
            .map_or_else(|| "unknown".to_owned(), |rows| rows.to_string());
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
/// estimates and indexes are read from `attached` when it is given; without it they are
/// unknown.
///
/// # Errors
///
/// Fails with the [`PortError`] of a query that fails.
pub fn source_impact(
    mut attached: Option<&mut AttachedSource>,
    source: &Source,
    care: &SourceCare,
) -> Result<Vec<TableImpact>, PortError> {
    let mut impacts = Vec::new();
    for table in declared_tables(source) {
        let (estimated_rows, index) = match attached.as_deref_mut() {
            Some(attached) => (estimate(attached, &table)?, used_index(attached, &table)?),
            None => (None, None),
        };
        impacts.push(TableImpact {
            source_name: source.name.clone(),
            table: table.name,
            estimated_rows,
            index,
            window: care.window,
            load: care.load,
            budget: care.budget(),
        });
    }
    Ok(impacts)
}

/// The rows the source estimates `table` holds.
fn estimate(
    attached: &mut AttachedSource,
    table: &DeclaredTable,
) -> Result<Option<u64>, PortError> {
    let alias = SqlValue::Text(attached.alias.clone());
    let name = SqlValue::Text(table.name.clone());
    let (sql, params) = match attached.identity.engine {
        DatabaseEngine::PostgreSql => (
            PG_TABLE_ESTIMATE,
            vec![
                alias,
                SqlValue::Text(PG_ESTIMATES.to_owned()),
                SqlValue::Text(SCHEMA.to_owned()),
                name,
            ],
        ),
        DatabaseEngine::DuckDb => (
            DUCKDB_TABLE_ESTIMATE,
            vec![alias, SqlValue::Text(DUCKDB_SCHEMA.to_owned()), name],
        ),
    };
    let rows = attached.connection.query_arrow(sql, &params)?;
    Ok(rows
        .iter()
        .find(|batch| batch.num_rows() > 0)
        .and_then(|batch| integer_at(batch, 0, 0))
        .and_then(|estimate| u64::try_from(estimate).ok()))
}

/// The index a read of `table` uses, in a PostgreSQL source.
fn used_index(
    attached: &mut AttachedSource,
    table: &DeclaredTable,
) -> Result<Option<String>, PortError> {
    if attached.identity.engine != DatabaseEngine::PostgreSql {
        return Ok(None);
    }
    let columns = match &table.incremental {
        Some(column) => vec![column.clone()],
        None => table.key.clone(),
    };
    if columns.is_empty() {
        return Ok(None);
    }
    let indexes = table_indexes(attached, table)?;
    Ok(indexes
        .into_iter()
        .find(|index| index.starts_with(&columns))
        .map(|index| index.name))
}
