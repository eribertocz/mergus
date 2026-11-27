//! The SQL fragments and scalar reads of applying a history run.

use arrow::array::{Array, AsArray};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Int64Type};
use arrow::record_batch::RecordBatch;
use mergus_ports::DatabaseConnection;

use crate::EngineError;
use crate::history::ChangeDetection;
use crate::segment::quote_identifier;

/// The DuckDB types, as `DESCRIBE` writes them, a `by = "<column>"` column may have, since
/// its value becomes `_valid_from` (MG-REQ-0663.6).
pub(crate) const BY_TYPES: [&str; 3] = ["DATE", "TIMESTAMP", "TIMESTAMP WITH TIME ZONE"];

/// The `_valid_from` of a version that `incoming` opens, given `latest`, the key's current
/// version, a tombstone included: a `by` value that is not later than `latest`'s
/// `_valid_from` gives way to the detection time, so no range is empty or inverted
/// (MG-REQ-0663.5).
pub(crate) fn valid_from(by: &ChangeDetection) -> String {
    match by {
        ChangeDetection::Increases(column) => {
            let column = quote_identifier(column);
            format!(
                "CASE WHEN latest.\"_valid_from\" IS NOT NULL \
                 AND CAST(incoming.{column} AS TIMESTAMPTZ) <= latest.\"_valid_from\" \
                 THEN getvariable('mergus_detected_at') ELSE incoming._mergus_from END"
            )
        }
        ChangeDetection::Differs(_) => "incoming._mergus_from".to_owned(),
    }
}

/// The SQL condition under which the row `incoming` is a new version of `current`
/// (MG-REQ-0662.2, MG-REQ-0662.3).
pub(crate) fn changed(by: &ChangeDetection) -> String {
    match by {
        ChangeDetection::Increases(column) => {
            let column = quote_identifier(column);
            format!(
                "(incoming.{column} IS NOT NULL AND (current.{column} IS NULL \
                 OR incoming.{column} > current.{column}))"
            )
        }
        ChangeDetection::Differs(columns) => {
            let tests: Vec<String> = columns
                .iter()
                .map(|column| {
                    let column = quote_identifier(column);
                    format!("incoming.{column} IS DISTINCT FROM current.{column}")
                })
                .collect();
            format!("({})", tests.join(" OR "))
        }
    }
}

/// The single integer `sql` returns, `None` when it is NULL.
pub(crate) fn scalar_i64(
    session: &mut dyn DatabaseConnection,
    sql: &str,
) -> Result<Option<i64>, EngineError> {
    let batches = session.query_arrow(sql, &[])?;
    let Some(batch) = batches.iter().find(|batch| batch.num_rows() > 0) else {
        return Ok(None);
    };
    let Some(value) = batch.column(0).as_primitive_opt::<Int64Type>() else {
        return Ok(None);
    };
    Ok((!value.is_null(0)).then(|| value.value(0)))
}

/// The single text `sql` returns, `None` when it is NULL.
pub(crate) fn scalar_text(
    session: &mut dyn DatabaseConnection,
    sql: &str,
) -> Result<Option<String>, EngineError> {
    Ok(first_text(&session.query_arrow(sql, &[])?))
}

/// The first value of the first column of `batches`, as text; `None` when it is NULL or
/// there is no row.
pub(crate) fn first_text(batches: &[RecordBatch]) -> Option<String> {
    let batch = batches.iter().find(|batch| batch.num_rows() > 0)?;
    let value = cast(batch.column(0), &DataType::Utf8).ok()?;
    let value = value.as_string::<i32>();
    (!value.is_null(0)).then(|| value.value(0).to_owned())
}
