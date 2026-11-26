//! Breaking changes of a source table since its last load (MG-REQ-0407).

use std::fmt;

use arrow::array::{Array, AsArray};
use arrow::datatypes::DataType;
use arrow::record_batch::RecordBatch;
use mergus_ports::PortError;

use crate::engine::AttachedSource;
use crate::table::DeclaredTable;

/// A column of a source table, with its type as DuckDB names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceColumn {
    /// The column name.
    pub name: String,
    /// Its type, as `DESCRIBE` gives it, for example `DECIMAL(10,2)`.
    pub data_type: String,
}

/// A breaking change of one column (MG-REQ-0407).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnChange {
    /// The column of the last load.
    pub column: String,
    /// Its type at the last load.
    pub old_type: String,
    /// Its type in the source; `None` when it was dropped or renamed (MG-REQ-0407.2).
    pub new_type: Option<String>,
}

/// A source table changed in a breaking way since its last load; its extraction stops
/// (MG-REQ-0407).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "table `{table}` of source `{source_name}` changed since its last load: {}",
    describe_changes(changes)
)]
pub struct SchemaChange {
    /// The source name.
    pub source_name: String,
    /// The declared table name.
    pub table: String,
    /// Its breaking changes, in the order of the columns of the last load.
    pub changes: Vec<ColumnChange>,
}

impl SchemaChange {
    /// Returns the `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        "MG-0405"
    }
}

impl fmt::Display for ColumnChange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.new_type {
            Some(new_type) => write!(
                formatter,
                "column `{}` changed type from `{}` to `{new_type}`",
                self.column, self.old_type
            ),
            None => write!(
                formatter,
                "column `{}` (`{}`) was dropped or renamed",
                self.column, self.old_type
            ),
        }
    }
}

/// The changes of a [`SchemaChange`], joined.
fn describe_changes(changes: &[ColumnChange]) -> String {
    changes
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Returns the breaking changes from the columns `last` of the last load to the columns
/// `current` of the source, in the order of `last`: a column whose type changed, every change
/// being incompatible in phase 0.3 (MG-REQ-0407.1), and a column that is missing, dropped or
/// renamed (MG-REQ-0407.2). An added column is not a change (MG-REQ-0407.3); an empty `last`,
/// a first load, has none.
#[must_use]
pub fn breaking_changes(last: &[SourceColumn], current: &[SourceColumn]) -> Vec<ColumnChange> {
    last.iter()
        .filter_map(|old| {
            let new_type = current
                .iter()
                .find(|column| column.name == old.name)
                .map(|column| column.data_type.clone());
            (new_type.as_deref() != Some(old.data_type.as_str())).then(|| ColumnChange {
                column: old.name.clone(),
                old_type: old.data_type.clone(),
                new_type,
            })
        })
        .collect()
}

/// Returns the columns of `table` in `attached`, in their order, read with `DESCRIBE`, which
/// resolves the table as the read does.
///
/// # Errors
///
/// Fails with the [`PortError`] of the query, or when its answer has no text columns
/// `column_name` and `column_type`.
pub fn source_columns(
    attached: &mut AttachedSource,
    table: &DeclaredTable,
) -> Result<Vec<SourceColumn>, PortError> {
    // sql: identifiers quoted by quote_identifier
    let sql = format!(
        "SELECT column_name, column_type FROM (DESCRIBE {}.{})",
        quote_identifier(&attached.alias),
        quote_identifier(&table.name)
    );
    let batches = attached.connection.query_arrow(&sql, &[])?;
    let mut columns = Vec::new();
    for batch in &batches {
        let names = text_column(batch, "column_name")?;
        let types = text_column(batch, "column_type")?;
        for row in 0..batch.num_rows() {
            columns.push(SourceColumn {
                name: names[row].clone(),
                data_type: types[row].clone(),
            });
        }
    }
    Ok(columns)
}

/// The values of the text column `name` of `batch`.
fn text_column(batch: &RecordBatch, name: &str) -> Result<Vec<String>, PortError> {
    let missing = || PortError::Failed(format!("`DESCRIBE` returned no text column `{name}`"));
    let column = batch.column_by_name(name).ok_or_else(missing)?;
    let values = match column.data_type() {
        DataType::Utf8 => column.as_string::<i32>().iter().collect::<Vec<_>>(),
        DataType::LargeUtf8 => column.as_string::<i64>().iter().collect(),
        DataType::Utf8View => column.as_string_view().iter().collect(),
        _ => return Err(missing()),
    };
    values
        .into_iter()
        .map(|value| value.map(str::to_owned).ok_or_else(missing))
        .collect()
}

/// Quotes `identifier` for DuckDB: in double quotes, with each double quote doubled.
fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
