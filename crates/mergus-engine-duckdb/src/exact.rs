//! Exact values: an input reaches DuckDB in the types its schema declares, or the step fails
//! (MG-REQ-0180).

use std::sync::Arc;

use arrow::array::{Array, ArrayRef};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::{DataType, Field, SchemaRef, TimeUnit};
use arrow::record_batch::RecordBatch;

use crate::EngineError;

/// Returns the DuckDB type that holds every value of `data_type` exactly, or why there is
/// none.
pub(crate) fn duckdb_type(data_type: &DataType) -> Result<String, String> {
    let name = match data_type {
        DataType::Boolean => "BOOLEAN",
        DataType::Int8 => "TINYINT",
        DataType::Int16 => "SMALLINT",
        DataType::Int32 => "INTEGER",
        DataType::Int64 => "BIGINT",
        DataType::UInt8 => "UTINYINT",
        DataType::UInt16 => "USMALLINT",
        DataType::UInt32 => "UINTEGER",
        DataType::UInt64 => "UBIGINT",
        DataType::Float32 => "FLOAT",
        DataType::Float64 => "DOUBLE",
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => "VARCHAR",
        DataType::Binary | DataType::LargeBinary | DataType::BinaryView => "BLOB",
        DataType::Date32 => "DATE",
        DataType::Timestamp(TimeUnit::Microsecond, None) => "TIMESTAMP",
        DataType::Timestamp(TimeUnit::Microsecond, Some(_)) => "TIMESTAMPTZ",
        DataType::Decimal128(precision, scale) if *precision <= 38 && *scale >= 0 => {
            return Ok(format!("DECIMAL({precision}, {scale})"));
        }
        other => {
            return Err(format!(
                "DuckDB has no exact type for the Arrow type {other}"
            ));
        }
    };
    Ok(name.to_owned())
}

/// Returns `batch` with each column in the type `schema` declares for it, matched by name.
///
/// A column already in its type is kept. Another is converted only when every value comes
/// back unchanged when converted back and no value becomes NULL; a timestamp keeps its time
/// zone.
pub(crate) fn conform(
    input: &str,
    batch: &RecordBatch,
    schema: &SchemaRef,
) -> Result<RecordBatch, EngineError> {
    let mut columns = Vec::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let fail = |reason: String| EngineError::Unrepresentable {
            input: input.to_owned(),
            column: field.name().clone(),
            reason,
        };
        let column = batch
            .column_by_name(field.name())
            .ok_or_else(|| fail("the rows have no such column".to_owned()))?;
        duckdb_type(field.data_type()).map_err(&fail)?;
        columns.push(convert(column, field).map_err(fail)?);
    }
    let fields: Vec<Field> = schema
        .fields()
        .iter()
        .map(|field| field.as_ref().clone().with_nullable(true))
        .collect();
    let target = Arc::new(arrow::datatypes::Schema::new(fields));
    RecordBatch::try_new(target, columns).map_err(|error| EngineError::Unrepresentable {
        input: input.to_owned(),
        column: String::new(),
        reason: error.to_string(),
    })
}

/// Converts `column` to the type of `field`, or says which value cannot be.
fn convert(column: &ArrayRef, field: &Field) -> Result<ArrayRef, String> {
    let source = column.data_type();
    let target = field.data_type();
    if source == target {
        return Ok(Arc::clone(column));
    }
    if let (DataType::Timestamp(_, from), DataType::Timestamp(_, to)) = (source, target)
        && from != to
    {
        return Err(format!(
            "a {source} value would lose its time zone as {target}"
        ));
    }
    let options = CastOptions {
        safe: true,
        ..CastOptions::default()
    };
    let cast = |array: &ArrayRef, to: &DataType| {
        cast_with_options(array, to, &options)
            .map_err(|_| format!("a {source} value cannot be represented exactly as {target}"))
    };
    let converted = cast(column, target)?;
    let back = cast(&converted, source)?;
    for row in 0..column.len() {
        let lost = converted.is_null(row) && !column.is_null(row);
        if lost || !same(column, &back, row) {
            return Err(format!(
                "the {source} value of row {row} cannot be represented exactly as {target}"
            ));
        }
    }
    Ok(converted)
}

/// Returns whether row `row` of `left` and `right` holds the same value, or NULL in both.
fn same(left: &ArrayRef, right: &ArrayRef, row: usize) -> bool {
    left.slice(row, 1).to_data() == right.slice(row, 1).to_data()
}
