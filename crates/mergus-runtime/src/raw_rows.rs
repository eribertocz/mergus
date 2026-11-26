//! The Arrow side of raw: the metadata columns (MG-REQ-0512) and the keys of rows.

use std::collections::BTreeSet;
use std::sync::Arc;

use arrow::array::{
    ArrayRef, BooleanArray, RecordBatch, StringArray, TimestampMicrosecondArray, new_null_array,
};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::error::ArrowError;
use arrow::row::{OwnedRow, RowConverter, SortField};
use arrow::util::display::{ArrayFormatter, FormatOptions};

/// The metadata columns every raw table ends with, in order (MG-REQ-0512).
pub(crate) const METADATA: [&str; 4] = ["_load_id", "_extracted_at", "_deleted", "_deleted_at"];

/// The time zone of the metadata timestamps.
const UTC: &str = "UTC";

/// The values of the metadata columns of one load.
#[derive(Clone, Debug)]
pub(crate) struct Metadata {
    /// `_load_id`.
    pub(crate) load_id: String,
    /// `_extracted_at`, in microseconds since the epoch.
    pub(crate) extracted_at: i64,
}

/// A `TIMESTAMPTZ` in UTC (MG-REQ-0512.2).
fn utc() -> DataType {
    DataType::Timestamp(TimeUnit::Microsecond, Some(UTC.into()))
}

/// The schema of the raw tables of a source table of schema `source`: its columns, then the
/// metadata columns (MG-REQ-0512.1 to MG-REQ-0512.3).
pub(crate) fn raw_schema(source: &Schema) -> SchemaRef {
    let mut fields: Vec<Field> = source
        .fields()
        .iter()
        .map(|field| field.as_ref().clone())
        .collect();
    fields.extend([
        Field::new(METADATA[0], DataType::Utf8, false),
        Field::new(METADATA[1], utc(), false),
        Field::new(METADATA[2], DataType::Boolean, false),
        Field::new(METADATA[3], utc(), true),
    ]);
    Arc::new(Schema::new(fields))
}

/// The first `count` columns of `batch`, the source's, followed by the metadata columns of
/// `metadata`: `_deleted` is `deleted`, and `_deleted_at` is `_extracted_at` for a delete and
/// NULL otherwise (MG-REQ-0512.3, MG-REQ-0518.2).
pub(crate) fn with_metadata(
    batch: &RecordBatch,
    count: usize,
    schema: &SchemaRef,
    metadata: &Metadata,
    deleted: bool,
) -> Result<RecordBatch, ArrowError> {
    let rows = batch.num_rows();
    let at =
        || TimestampMicrosecondArray::from(vec![metadata.extracted_at; rows]).with_timezone(UTC);
    let deleted_at: ArrayRef = if deleted {
        Arc::new(at())
    } else {
        new_null_array(&utc(), rows)
    };
    let mut columns: Vec<ArrayRef> = batch.columns().iter().take(count).cloned().collect();
    columns.extend([
        Arc::new(StringArray::from(vec![metadata.load_id.as_str(); rows])) as ArrayRef,
        Arc::new(at()),
        Arc::new(BooleanArray::from(vec![deleted; rows])),
        deleted_at,
    ]);
    RecordBatch::try_new(Arc::clone(schema), columns)
}

/// `batch` with the columns of `schema`, found by name and cast to their type, as rows read
/// back from storage may carry other Arrow types for the same DuckDB type.
pub(crate) fn conform(batch: &RecordBatch, schema: &SchemaRef) -> Result<RecordBatch, ArrowError> {
    let mut columns = Vec::new();
    for field in schema.fields() {
        let column = batch.column_by_name(field.name()).ok_or_else(|| {
            ArrowError::SchemaError(format!("the rows have no column `{}`", field.name()))
        })?;
        columns.push(cast(column, field.data_type())?);
    }
    RecordBatch::try_new(Arc::clone(schema), columns)
}

/// The key columns of a table, and the comparable form of their values.
pub(crate) struct Keys {
    columns: Vec<Field>,
    converter: RowConverter,
}

impl Keys {
    /// The key `columns` of the tables of `schema`.
    pub(crate) fn new(schema: &Schema, columns: &[String]) -> Result<Self, ArrowError> {
        let columns = columns
            .iter()
            .map(|column| schema.field_with_name(column).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let sort = columns
            .iter()
            .map(|field| SortField::new(field.data_type().clone()))
            .collect();
        Ok(Self {
            columns,
            converter: RowConverter::new(sort)?,
        })
    }

    /// The key columns of `batch`, found by name and cast to their type.
    fn columns(&self, batch: &RecordBatch) -> Result<Vec<ArrayRef>, ArrowError> {
        let mut columns = Vec::new();
        for field in &self.columns {
            let column = batch.column_by_name(field.name()).ok_or_else(|| {
                ArrowError::SchemaError(format!("the rows have no key column `{}`", field.name()))
            })?;
            columns.push(cast(column, field.data_type())?);
        }
        Ok(columns)
    }

    /// The key of each row of `batch`, in order.
    pub(crate) fn of(&self, batch: &RecordBatch) -> Result<Vec<OwnedRow>, ArrowError> {
        let rows = self.converter.convert_columns(&self.columns(batch)?)?;
        Ok(rows.iter().map(|row| row.owned()).collect())
    }

    /// The keys of every row of `batches`.
    pub(crate) fn set(&self, batches: &[RecordBatch]) -> Result<BTreeSet<OwnedRow>, ArrowError> {
        let mut keys = BTreeSet::new();
        for batch in batches {
            keys.extend(self.of(batch)?);
        }
        Ok(keys)
    }

    /// The key of row `row` of `batch`, as `column = value` pairs.
    pub(crate) fn show(&self, batch: &RecordBatch, row: usize) -> Result<String, ArrowError> {
        let options = FormatOptions::default();
        let mut pairs = Vec::new();
        for (field, column) in self.columns.iter().zip(self.columns(batch)?) {
            let formatter = ArrayFormatter::try_new(column.as_ref(), &options)?;
            pairs.push(format!("{} = {}", field.name(), formatter.value(row)));
        }
        Ok(pairs.join(", "))
    }
}
