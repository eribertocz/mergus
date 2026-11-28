//! The Arrow schema of a step's columns as DuckDB describes them, which the DuckDB engine
//! needs declared before a segment runs (MG-REQ-0604).

use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use mergus_ports::ResultColumn;

/// The scalar DuckDB types a run carries, with their Arrow type.
fn scalar(name: &str) -> Option<DataType> {
    let micros = TimeUnit::Microsecond;
    Some(match name {
        "BOOLEAN" => DataType::Boolean,
        "TINYINT" => DataType::Int8,
        "SMALLINT" => DataType::Int16,
        "INTEGER" => DataType::Int32,
        "BIGINT" => DataType::Int64,
        "UTINYINT" => DataType::UInt8,
        "USMALLINT" => DataType::UInt16,
        "UINTEGER" => DataType::UInt32,
        "UBIGINT" => DataType::UInt64,
        "FLOAT" => DataType::Float32,
        "DOUBLE" => DataType::Float64,
        "VARCHAR" => DataType::Utf8,
        "BLOB" => DataType::Binary,
        "DATE" => DataType::Date32,
        "TIME" => DataType::Time64(micros),
        "TIMESTAMP" => DataType::Timestamp(micros, None),
        "TIMESTAMP_S" => DataType::Timestamp(TimeUnit::Second, None),
        "TIMESTAMP_MS" => DataType::Timestamp(TimeUnit::Millisecond, None),
        "TIMESTAMP_NS" => DataType::Timestamp(TimeUnit::Nanosecond, None),
        "TIMESTAMP WITH TIME ZONE" => DataType::Timestamp(micros, Some("UTC".into())),
        _ => return None,
    })
}

/// `DECIMAL(p,s)` as Arrow's `Decimal128(p, s)`.
fn decimal(name: &str) -> Option<DataType> {
    let inner = name.strip_prefix("DECIMAL(")?.strip_suffix(')')?;
    let (precision, scale) = inner.split_once(',')?;
    let precision: u8 = precision.trim().parse().ok()?;
    let scale: i8 = scale.trim().parse().ok()?;
    Some(DataType::Decimal128(precision, scale))
}

/// The Arrow schema of `columns`, every column nullable.
///
/// # Errors
///
/// Fails with the first column whose type a run cannot carry, as `(name, type)`.
pub(crate) fn schema(columns: &[ResultColumn]) -> Result<SchemaRef, (String, String)> {
    let mut fields = Vec::with_capacity(columns.len());
    for column in columns {
        let name = column.data_type.as_str();
        let data_type = scalar(name)
            .or_else(|| decimal(name))
            .ok_or_else(|| (column.name.clone(), column.data_type.clone()))?;
        fields.push(Field::new(&column.name, data_type, true));
    }
    Ok(Arc::new(Schema::new(fields)))
}

#[cfg(test)]
mod tests {
    use arrow::datatypes::DataType;
    use mergus_ports::ResultColumn;

    use super::schema;

    fn column(name: &str, data_type: &str) -> ResultColumn {
        ResultColumn {
            name: name.to_owned(),
            data_type: data_type.to_owned(),
        }
    }

    #[test]
    fn duckdb_types_become_arrow_types() {
        let found = schema(&[column("n", "BIGINT"), column("price", "DECIMAL(18,3)")]).unwrap();
        assert_eq!(found.field(0).data_type(), &DataType::Int64);
        assert_eq!(found.field(1).data_type(), &DataType::Decimal128(18, 3));
        let refused = schema(&[column("big", "HUGEINT")]).unwrap_err();
        assert_eq!(refused, ("big".to_owned(), "HUGEINT".to_owned()));
    }
}
