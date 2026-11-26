//! The index check of `mergus doctor --source` (MG-REQ-0431): the key and the incremental
//! column of each declared table must have an index in the source. In phase 0.3 only
//! PostgreSQL sources are checked, by reading `pg_index` through `postgres_query`
//! (MG-REQ-0431.3).

use std::fmt;

use arrow::array::{Array, AsArray};
use arrow::datatypes::{DataType, Int64Type};
use arrow::record_batch::RecordBatch;
use mergus_ports::{PortError, SqlValue};
use mergus_project::Source;

use crate::engine::{AttachedSource, DatabaseEngine};
use crate::table::{DeclaredTable, declared_tables};

/// The schema a declared table is read from.
pub(crate) const SCHEMA: &str = "public";

/// The PostgreSQL query of the index check: the columns of every index of the server, one
/// row per indexed column, in index order. The text is fixed: the table is chosen by bound
/// parameters outside it (MG-REQ-0007).
pub const PG_INDEXES: &str = "SELECT n.nspname::text AS schema_name, t.relname::text AS table_name, \
     i.relname::text AS index_name, k.ord::bigint AS position, a.attname::text AS column_name \
     FROM pg_index x \
     JOIN pg_class t ON t.oid = x.indrelid \
     JOIN pg_class i ON i.oid = x.indexrelid \
     JOIN pg_namespace n ON n.oid = t.relnamespace \
     CROSS JOIN LATERAL unnest(x.indkey::int2[]) WITH ORDINALITY AS k(attnum, ord) \
     LEFT JOIN pg_attribute a ON a.attrelid = x.indrelid AND a.attnum = k.attnum";

/// The DuckDB query of the indexes of one table, read from the answer of [`PG_INDEXES`],
/// with the parameters alias, [`PG_INDEXES`], schema and table.
pub const TABLE_INDEXES: &str = "SELECT index_name, column_name FROM postgres_query(?, ?) \
     WHERE schema_name = ? AND table_name = ? ORDER BY index_name, position";

/// One index of a table: its name and its columns in index order; an expression is `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Index {
    pub(crate) name: String,
    pub(crate) columns: Vec<Option<String>>,
}

impl Index {
    /// Whether the index starts with `columns`, in any order.
    pub(crate) fn starts_with(&self, columns: &[String]) -> bool {
        columns.len() <= self.columns.len()
            && columns.iter().all(|column| {
                self.columns[..columns.len()]
                    .iter()
                    .any(|indexed| indexed.as_ref() == Some(column))
            })
    }
}

/// Which columns of a table have no index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Unindexed {
    /// The key.
    Key,
    /// The incremental column.
    Incremental,
}

/// A warning of `mergus doctor --source`: a key or incremental column without an index,
/// with the index to create (MG-REQ-0431.1, MG-REQ-0431.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexWarning {
    /// The source name.
    pub source_name: String,
    /// The declared table.
    pub table: String,
    /// Whether the key or the incremental column has no index.
    pub unindexed: Unindexed,
    /// The columns, the key's in key order.
    pub columns: Vec<String>,
}

impl IndexWarning {
    /// The `MG-####` code of the warning (MG-REQ-0431.1).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "MG-0406"
    }

    /// The suggested index: `CREATE INDEX ON "<schema>"."<table>" ("<column>", …);`
    /// (MG-REQ-0431.2).
    #[must_use]
    pub fn suggestion(&self) -> String {
        let columns = self
            .columns
            .iter()
            .map(|column| quote(column))
            .collect::<Vec<_>>()
            .join(", ");
        // sql: identifiers quoted by quote
        format!(
            "CREATE INDEX ON {}.{} ({columns});",
            quote(SCHEMA),
            quote(&self.table)
        )
    }
}

impl fmt::Display for IndexWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.unindexed {
            Unindexed::Key => "key",
            Unindexed::Incremental => "incremental column",
        };
        write!(
            formatter,
            "the {what} ({}) of table `{}` of source `{}` has no index in the source; \
             suggested index: {}",
            self.columns.join(", "),
            self.table,
            self.source_name,
            self.suggestion()
        )
    }
}

/// The index warnings of every table `source` declares, in the order of their names, the
/// key's before the incremental column's. A source that is not PostgreSQL is not checked
/// (MG-REQ-0431.3).
///
/// # Errors
///
/// Fails with the [`PortError`] of a query that fails.
pub fn check_indexes(
    attached: &mut AttachedSource,
    source: &Source,
) -> Result<Vec<IndexWarning>, PortError> {
    if attached.identity.engine != DatabaseEngine::PostgreSql {
        return Ok(Vec::new());
    }
    let mut warnings = Vec::new();
    for table in declared_tables(source) {
        let indexes = table_indexes(attached, &table)?;
        let mut missing = |unindexed, columns: Vec<String>| {
            if !columns.is_empty() && !indexes.iter().any(|index| index.starts_with(&columns)) {
                warnings.push(IndexWarning {
                    source_name: source.name.clone(),
                    table: table.name.clone(),
                    unindexed,
                    columns,
                });
            }
        };
        missing(Unindexed::Key, table.key.clone());
        missing(
            Unindexed::Incremental,
            table.incremental.iter().cloned().collect(),
        );
    }
    Ok(warnings)
}

/// The indexes of `table` in a PostgreSQL source, sorted by name.
pub(crate) fn table_indexes(
    attached: &mut AttachedSource,
    table: &DeclaredTable,
) -> Result<Vec<Index>, PortError> {
    let params = [
        SqlValue::Text(attached.alias.clone()),
        SqlValue::Text(PG_INDEXES.to_owned()),
        SqlValue::Text(SCHEMA.to_owned()),
        SqlValue::Text(table.name.clone()),
    ];
    let rows = attached.connection.query_arrow(TABLE_INDEXES, &params)?;
    let mut indexes: Vec<Index> = Vec::new();
    for batch in &rows {
        for row in 0..batch.num_rows() {
            let Some(name) = text_at(batch, 0, row) else {
                continue;
            };
            let column = text_at(batch, 1, row);
            match indexes.last_mut() {
                Some(index) if index.name == name => index.columns.push(column),
                _ => indexes.push(Index {
                    name,
                    columns: vec![column],
                }),
            }
        }
    }
    Ok(indexes)
}

/// The text at `row` of the column `column` of `batch`, when it is text and not null.
pub(crate) fn text_at(batch: &RecordBatch, column: usize, row: usize) -> Option<String> {
    let column = batch.columns().get(column)?;
    if column.is_null(row) {
        return None;
    }
    match column.data_type() {
        DataType::Utf8 => Some(column.as_string::<i32>().value(row).to_owned()),
        DataType::LargeUtf8 => Some(column.as_string::<i64>().value(row).to_owned()),
        DataType::Utf8View => Some(column.as_string_view().value(row).to_owned()),
        _ => None,
    }
}

/// The integer at `row` of the column `column` of `batch`, when it is a `BIGINT` and not
/// null.
pub(crate) fn integer_at(batch: &RecordBatch, column: usize, row: usize) -> Option<i64> {
    let column = batch.columns().get(column)?;
    if column.is_null(row) || column.data_type() != &DataType::Int64 {
        return None;
    }
    Some(column.as_primitive::<Int64Type>().value(row))
}

/// Quotes `identifier` for PostgreSQL: in double quotes, with each double quote doubled.
fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
