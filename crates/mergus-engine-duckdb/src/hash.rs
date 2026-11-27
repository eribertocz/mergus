//! The hash of a table, independent of the order of its rows (MG-REQ-0638).

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, AsArray, make_array};
use arrow::datatypes::{
    DataType, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type, UInt8Type,
    UInt16Type, UInt32Type, UInt64Type,
};
use arrow::record_batch::RecordBatch;
use arrow::util::display::{ArrayFormatter, FormatOptions};
use mergus_contracts::canonical_json;
use mergus_contracts::hash::{sha256, sha256_hex};
use serde_json::{Map, Number, Value};

use crate::EngineError;

/// The largest integer a JSON number holds exactly as a double: 2^53.
const EXACT_INTEGER: u64 = 1 << 53;

/// Returns the hash of the rows of `batches`, in lowercase hexadecimal: the SHA-256 of the
/// sorted SHA-256 hashes of each row's canonical JSON (RFC 8785). The order of the rows, and
/// how they are split into batches, does not change it.
///
/// A row is an object of its column names and values. NULL is `null`, a boolean is a JSON
/// boolean, an integer up to 2^53 in magnitude and a finite float are JSON numbers, and any
/// other value is the string Arrow displays for it, so that decimals and large integers stay
/// exact; a timestamp with a time zone is its UTC value followed by the zone's name.
///
/// # Errors
///
/// Fails with [`EngineError::Hash`] when a value cannot be displayed.
pub fn table_hash(batches: &[RecordBatch]) -> Result<String, EngineError> {
    let mut rows = Vec::new();
    for batch in batches {
        rows.extend(row_hashes(batch)?);
    }
    rows.sort_unstable();
    Ok(sha256_hex(&rows.concat()))
}

/// Returns the SHA-256 of each row of `batch`, in row order.
fn row_hashes(batch: &RecordBatch) -> Result<Vec<[u8; 32]>, EngineError> {
    let schema = batch.schema();
    let shown = batch
        .columns()
        .iter()
        .map(displayable)
        .collect::<Result<Vec<_>, _>>()?;
    let options = FormatOptions::default().with_display_error(false);
    let formatters = shown
        .iter()
        .map(|(column, _)| ArrayFormatter::try_new(column.as_ref(), &options))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| display(&error))?;
    let mut hashes = Vec::with_capacity(batch.num_rows());
    for row in 0..batch.num_rows() {
        let mut object = Map::new();
        let columns = schema.fields().iter().zip(batch.columns());
        for (((field, column), formatter), (_, zone)) in columns.zip(&formatters).zip(&shown) {
            let value = match json_value(column.as_ref(), row) {
                Some(value) => value,
                None => {
                    let text = formatter
                        .value(row)
                        .try_to_string()
                        .map_err(|error| display(&error))?;
                    Value::String(match zone {
                        Some(zone) => format!("{text} {zone}"),
                        None => text,
                    })
                }
            };
            object.insert(field.name().clone(), value);
        }
        hashes.push(sha256(canonical_json(&Value::Object(object)).as_bytes()));
    }
    Ok(hashes)
}

/// Returns the array whose display is the text of `column`, with the time zone written after
/// it: a timestamp with a time zone is shown as its UTC value followed by the zone's name.
fn displayable(column: &ArrayRef) -> Result<(ArrayRef, Option<String>), EngineError> {
    let DataType::Timestamp(unit, Some(zone)) = column.data_type() else {
        return Ok((Arc::clone(column), None));
    };
    let data = column
        .to_data()
        .into_builder()
        .data_type(DataType::Timestamp(*unit, None))
        .build()
        .map_err(|error| display(&error))?;
    Ok((make_array(data), Some(zone.to_string())))
}

/// Returns the JSON value of row `row` of `column` when it is NULL, a boolean, an exact
/// integer or a finite float; `None` when it is written as text.
fn json_value(column: &dyn Array, row: usize) -> Option<Value> {
    if column.is_null(row) {
        return Some(Value::Null);
    }
    let integer = |value: i128| {
        (value.unsigned_abs() <= u128::from(EXACT_INTEGER))
            .then(|| {
                i64::try_from(value)
                    .ok()
                    .map(|value| Value::Number(value.into()))
            })
            .flatten()
    };
    match column.data_type() {
        DataType::Boolean => Some(Value::Bool(column.as_boolean().value(row))),
        DataType::Int8 => integer(column.as_primitive::<Int8Type>().value(row).into()),
        DataType::Int16 => integer(column.as_primitive::<Int16Type>().value(row).into()),
        DataType::Int32 => integer(column.as_primitive::<Int32Type>().value(row).into()),
        DataType::Int64 => integer(column.as_primitive::<Int64Type>().value(row).into()),
        DataType::UInt8 => integer(column.as_primitive::<UInt8Type>().value(row).into()),
        DataType::UInt16 => integer(column.as_primitive::<UInt16Type>().value(row).into()),
        DataType::UInt32 => integer(column.as_primitive::<UInt32Type>().value(row).into()),
        DataType::UInt64 => integer(column.as_primitive::<UInt64Type>().value(row).into()),
        DataType::Float32 => {
            Number::from_f64(column.as_primitive::<Float32Type>().value(row).into())
                .map(Value::Number)
        }
        DataType::Float64 => {
            Number::from_f64(column.as_primitive::<Float64Type>().value(row)).map(Value::Number)
        }
        _ => None,
    }
}

fn display(error: &dyn std::error::Error) -> EngineError {
    EngineError::Hash {
        reason: error.to_string(),
    }
}
