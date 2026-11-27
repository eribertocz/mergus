//! The reads of an extraction through the `Database` port: the projection and the
//! incremental filter pushed down (MG-REQ-0404), full loads (MG-REQ-0411) and keyset chunks
//! (MG-REQ-0417), every value bound as a parameter (MG-REQ-0007).

use std::num::NonZeroU32;

use arrow::array::{Array, AsArray};
use arrow::compute::cast;
use arrow::datatypes::{
    DataType, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type, UInt8Type,
    UInt16Type, UInt32Type, UInt64Type,
};
use arrow::record_batch::RecordBatch;
use mergus_ports::{PortError, SqlValue};
use mergus_project::Source;

use crate::copied::copied_tables;
use crate::engine::AttachedSource;
use crate::table::{DeclaredTable, RawTables};

/// The columns a read transfers (MG-REQ-0404).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projection {
    /// Every column of the table.
    All,
    /// The columns the project uses, given by the caller. The table's key and incremental
    /// columns are added when missing, since chunks and marks are read from them.
    Columns(Vec<String>),
}

/// How the caller wants a declared table read.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadOptions {
    /// The columns to transfer.
    pub projection: Projection,
    /// The lowest value of the incremental column to read, overlap already applied; `None`
    /// for a first load. Ignored for a table without `incremental` (MG-REQ-0411).
    pub since: Option<SqlValue>,
    /// The rows of one keyset chunk (MG-REQ-0417); `None`, or a table without `key`, reads
    /// the table in one statement.
    pub chunk_rows: Option<NonZeroU32>,
}

/// The batches read from one declared table, and the raw tables they land in.
#[derive(Clone, Debug, PartialEq)]
pub struct TableExtraction {
    /// The declared table name.
    pub table: String,
    /// Where its rows land (MG-REQ-0403).
    pub raw: RawTables,
    /// Its rows, chunk after chunk.
    pub batches: Vec<RecordBatch>,
}

/// Reads every table `source` declares, and no other (MG-REQ-0402), from `attached`, in
/// the order of their names, each with the options `options` gives for it. A `connect` or
/// `live` source returns no table and reads nothing (MG-REQ-0405).
///
/// It checks no breaking change (MG-REQ-0407) and records no read time (MG-REQ-0419): a run
/// MUST extract with [`extract_source_timed`](crate::extract_source_timed), which does both.
///
/// # Errors
///
/// Fails with the first [`PortError`] of a read.
pub fn extract_source(
    attached: &mut AttachedSource,
    source: &Source,
    options: &dyn Fn(&DeclaredTable) -> ReadOptions,
) -> Result<Vec<TableExtraction>, PortError> {
    let mut extracted = Vec::new();
    for table in copied_tables(source) {
        let table_options = options(&table);
        let chunks = read_table(attached, &table, &table_options).collect::<Result<Vec<_>, _>>()?;
        extracted.push(TableExtraction {
            raw: table.raw(&source.name),
            table: table.name,
            batches: chunks.concat(),
        });
    }
    Ok(extracted)
}

/// Returns the chunks of `table` read from `attached`, each one statement of its own, so
/// that no transaction outlives a chunk (MG-REQ-0417).
pub fn read_table<'a>(
    attached: &'a mut AttachedSource,
    table: &'a DeclaredTable,
    options: &'a ReadOptions,
) -> TableReader<'a> {
    TableReader {
        attached,
        table,
        options,
        after: None,
        done: false,
    }
}

/// The chunks of one table, read on demand.
pub struct TableReader<'a> {
    attached: &'a mut AttachedSource,
    table: &'a DeclaredTable,
    options: &'a ReadOptions,
    /// The key of the last row read, once a chunk was read.
    after: Option<Vec<SqlValue>>,
    done: bool,
}

impl std::fmt::Debug for TableReader<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TableReader")
            .field("table", &self.table)
            .field("after", &self.after)
            .finish_non_exhaustive()
    }
}

impl Iterator for TableReader<'_> {
    type Item = Result<Vec<RecordBatch>, PortError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let chunk = self.read_chunk();
        if chunk.is_err() {
            self.done = true;
        }
        Some(chunk)
    }
}

impl TableReader<'_> {
    /// Whether every chunk was read, or the last one failed.
    pub(crate) const fn is_done(&self) -> bool {
        self.done
    }

    /// Sends the chunk that failed again on the next call (MG-REQ-0428).
    pub(crate) fn retry(&mut self) {
        self.done = false;
    }

    /// Reads the next chunk and remembers where the following one starts.
    fn read_chunk(&mut self) -> Result<Vec<RecordBatch>, PortError> {
        let chunk_rows = self
            .options
            .chunk_rows
            .filter(|_| !self.table.key.is_empty());
        let (sql, params) = statement(
            self.table,
            self.options,
            &self.attached.alias,
            chunk_rows,
            self.after.as_deref(),
        );
        let batches = self.attached.connection.query_arrow(&sql, &params)?;
        let Some(limit) = chunk_rows else {
            self.done = true;
            return Ok(batches);
        };
        let read: usize = batches.iter().map(RecordBatch::num_rows).sum();
        if read < limit.get() as usize {
            self.done = true;
        } else {
            self.after = Some(last_key(&batches, &self.table.key)?);
        }
        Ok(batches)
    }
}

/// The statement of one read of `table` attached as `alias`, with its parameters.
pub(crate) fn statement(
    table: &DeclaredTable,
    options: &ReadOptions,
    alias: &str,
    chunk_rows: Option<NonZeroU32>,
    after: Option<&[SqlValue]>,
) -> (String, Vec<SqlValue>) {
    let columns = select_list(table, &options.projection);
    let from = format!(
        "{}.{}",
        quote_identifier(alias),
        quote_identifier(&table.name)
    );
    // sql: identifiers quoted by quote_identifier
    let mut sql = format!("SELECT {columns} FROM {from}");
    let mut params = Vec::new();
    let mut conditions = Vec::new();
    if let (Some(column), Some(since)) = (&table.incremental, &options.since) {
        conditions.push(format!("{} >= ?", quote_identifier(column)));
        params.push(since.clone());
    }
    let key = table
        .key
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>();
    if let (Some(_), Some(after)) = (chunk_rows, after) {
        conditions.push(keyset_condition(&key));
        params.extend_from_slice(after);
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    if let Some(limit) = chunk_rows {
        sql.push_str(" ORDER BY ");
        sql.push_str(&key.join(", "));
        sql.push_str(" LIMIT ?");
        params.push(SqlValue::Integer(i64::from(limit.get())));
    }
    (sql, params)
}

/// `"key" > ?` for a key of one column, `("a", "b") > (?, ?)` for a composite key.
fn keyset_condition(key: &[String]) -> String {
    if let [column] = key {
        return format!("{column} > ?");
    }
    let marks = vec!["?"; key.len()].join(", ");
    format!("({}) > ({marks})", key.join(", "))
}

/// The quoted select list of `projection`, with the key and incremental columns added.
fn select_list(table: &DeclaredTable, projection: &Projection) -> String {
    let Projection::Columns(columns) = projection else {
        return "*".to_owned();
    };
    let mut selected: Vec<&String> = Vec::new();
    let needed = columns.iter().chain(&table.key).chain(&table.incremental);
    for column in needed {
        if !selected.contains(&column) {
            selected.push(column);
        }
    }
    selected
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The key of the last row of `batches`, one bound value per key column.
pub(crate) fn last_key(
    batches: &[RecordBatch],
    key: &[String],
) -> Result<Vec<SqlValue>, PortError> {
    let Some(batch) = batches.iter().rev().find(|batch| batch.num_rows() > 0) else {
        return Err(PortError::Failed("a full chunk holds no row".to_owned()));
    };
    let row = batch.num_rows() - 1;
    key.iter()
        .map(|name| {
            let column = batch.column_by_name(name).ok_or_else(|| {
                PortError::Failed(format!("the chunk has no key column `{name}`"))
            })?;
            value_at(column.as_ref(), row)
        })
        .collect()
}

/// The value at `row` of `column`, bound with its own type when [`SqlValue`] has it, and
/// as its text otherwise, which DuckDB casts to the type of the key column.
pub(crate) fn value_at(column: &dyn Array, row: usize) -> Result<SqlValue, PortError> {
    if column.is_null(row) {
        return Ok(SqlValue::Null);
    }
    let value = match column.data_type() {
        DataType::Boolean => SqlValue::Boolean(column.as_boolean().value(row)),
        DataType::Int8 => SqlValue::Integer(column.as_primitive::<Int8Type>().value(row).into()),
        DataType::Int16 => SqlValue::Integer(column.as_primitive::<Int16Type>().value(row).into()),
        DataType::Int32 => SqlValue::Integer(column.as_primitive::<Int32Type>().value(row).into()),
        DataType::Int64 => SqlValue::Integer(column.as_primitive::<Int64Type>().value(row)),
        DataType::UInt8 => SqlValue::Integer(column.as_primitive::<UInt8Type>().value(row).into()),
        DataType::UInt16 => {
            SqlValue::Integer(column.as_primitive::<UInt16Type>().value(row).into())
        }
        DataType::UInt32 => {
            SqlValue::Integer(column.as_primitive::<UInt32Type>().value(row).into())
        }
        DataType::UInt64 => match i64::try_from(column.as_primitive::<UInt64Type>().value(row)) {
            Ok(value) => SqlValue::Integer(value),
            Err(_) => as_text(column, row)?,
        },
        DataType::Float32 => {
            SqlValue::Double(column.as_primitive::<Float32Type>().value(row).into())
        }
        DataType::Float64 => SqlValue::Double(column.as_primitive::<Float64Type>().value(row)),
        _ => as_text(column, row)?,
    };
    Ok(value)
}

/// The value at `row` of `column` cast to text.
fn as_text(column: &dyn Array, row: usize) -> Result<SqlValue, PortError> {
    let text = cast(&column.slice(row, 1), &DataType::Utf8)
        .map_err(|error| PortError::Failed(format!("cannot bind a key value: {error}")))?;
    Ok(SqlValue::Text(text.as_string::<i32>().value(0).to_owned()))
}

/// Quotes `identifier` for DuckDB: in double quotes, with each double quote doubled.
pub(crate) fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}
