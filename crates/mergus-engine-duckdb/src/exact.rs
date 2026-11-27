//! Exact values: a table reaches DuckDB, and leaves it, in the types its schema declares, or
//! the step fails (MG-REQ-0180).

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, make_array};
use arrow::compute::{CastOptions, cast_with_options};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::record_batch::RecordBatch;

use crate::EngineError;

/// How a column with a time zone meets a declared column with another time zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Zones {
    /// The zone must be the declared one: an input keeps the zone it was written with.
    Keep,
    /// The declared zone is restored on the same instants: DuckDB's `TIMESTAMPTZ` keeps only
    /// the instant and gives it back in UTC.
    Restore,
}

/// Returns the type DuckDB describes for the column that holds every value of `data_type`
/// exactly, or why there is none.
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
        DataType::Timestamp(TimeUnit::Microsecond, Some(_)) => "TIMESTAMP WITH TIME ZONE",
        DataType::Decimal128(precision, scale) if *precision <= 38 && *scale >= 0 => {
            return Ok(format!("DECIMAL({precision},{scale})"));
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
/// `table` names the table in errors.
///
/// A column already in its type is kept. Another is converted only when every value comes
/// back unchanged when converted back and no value becomes NULL. A timestamp keeps its time
/// zone, or gets the declared one back on the same instants with [`Zones::Restore`].
pub(crate) fn conform(
    table: &str,
    batch: &RecordBatch,
    schema: &SchemaRef,
    zones: Zones,
) -> Result<RecordBatch, EngineError> {
    let mut columns = Vec::with_capacity(schema.fields().len());
    for field in schema.fields() {
        let fail = |reason: String| unrepresentable(table, field.name(), reason);
        let column = batch
            .column_by_name(field.name())
            .ok_or_else(|| fail("the rows have no such column".to_owned()))?;
        duckdb_type(field.data_type()).map_err(&fail)?;
        columns.push(convert(column, field, zones).map_err(fail)?);
    }
    RecordBatch::try_new(nullable(schema), columns)
        .map_err(|error| unrepresentable(table, "", error.to_string()))
}

/// Returns `schema` with every column nullable, as DuckDB gives its columns back.
pub(crate) fn nullable(schema: &Schema) -> SchemaRef {
    let fields: Vec<Field> = schema
        .fields()
        .iter()
        .map(|field| field.as_ref().clone().with_nullable(true))
        .collect();
    Arc::new(Schema::new(fields))
}

/// Returns the error of a value of `column` of `table` that cannot be represented.
pub(crate) fn unrepresentable(table: &str, column: &str, reason: String) -> EngineError {
    EngineError::Unrepresentable {
        table: table.to_owned(),
        column: column.to_owned(),
        reason,
    }
}

/// Converts `column` to the type of `field`, or says which value cannot be.
fn convert(column: &ArrayRef, field: &Field, zones: Zones) -> Result<ArrayRef, String> {
    let source = column.data_type();
    let target = field.data_type();
    if source == target {
        return Ok(Arc::clone(column));
    }
    if let (DataType::Timestamp(from_unit, from), DataType::Timestamp(to_unit, to)) =
        (source, target)
        && from != to
    {
        return match (zones, from, to) {
            (Zones::Restore, Some(_), Some(_)) if from_unit == to_unit => {
                let data = column
                    .to_data()
                    .into_builder()
                    .data_type(target.clone())
                    .build()
                    .map_err(|error| error.to_string())?;
                Ok(make_array(data))
            }
            _ => Err(format!(
                "a {source} value would lose its time zone as {target}"
            )),
        };
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
