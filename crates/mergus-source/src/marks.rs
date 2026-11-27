//! The watermark of committed rows (MG-REQ-0412.1) and the text DuckDB gives for an
//! incremental value with `CAST(value AS VARCHAR)` (MG-REQ-0413).

use arrow::array::{Array, ArrayRef, AsArray};
use arrow::error::ArrowError;
use arrow::record_batch::RecordBatch;
use arrow::row::{OwnedRow, RowConverter, SortField};
use mergus_ports::{PortError, SqlValue};

use crate::incremental::IncrementalReader;
use crate::incremental_error::ExtractError;
use crate::read::quote_identifier;
use crate::temporal::bound_text;

/// The value, as an array of one, of the largest non-NULL value of `column` in `batches`, or
/// of the smallest when `largest` is false; `None` when every value is NULL or there is no
/// row. Values are ordered in Arrow's row format, which keeps the order of the values.
pub(crate) fn extreme(
    batches: &[RecordBatch],
    column: &str,
    largest: bool,
) -> Result<Option<ArrayRef>, PortError> {
    let failed = |error: ArrowError| PortError::Failed(error.to_string());
    let mut best: Option<(OwnedRow, ArrayRef)> = None;
    for batch in batches {
        let values = batch.column_by_name(column).ok_or_else(|| {
            PortError::Failed(format!("the rows have no incremental column `{column}`"))
        })?;
        let converter =
            RowConverter::new(vec![SortField::new(values.data_type().clone())]).map_err(failed)?;
        let rows = converter
            .convert_columns(std::slice::from_ref(values))
            .map_err(failed)?;
        for index in (0..values.len()).filter(|index| !values.is_null(*index)) {
            let row = rows.row(index);
            let better = best.as_ref().is_none_or(|(known, _)| {
                if largest {
                    row > known.row()
                } else {
                    row < known.row()
                }
            });
            if better {
                best = Some((row.owned(), values.slice(index, 1)));
            }
        }
    }
    Ok(best.map(|(_, value)| value))
}

impl IncrementalReader<'_> {
    /// The watermark of `batches`, the rows a load committed: the largest non-NULL value of
    /// the incremental column (MG-REQ-0412.1), in the text DuckDB gives for
    /// `CAST(value AS VARCHAR)` (MG-REQ-0413). `None` for a table without `incremental`, or
    /// when every value is NULL: the load keeps the previous watermark.
    ///
    /// # Errors
    ///
    /// Fails with [`ExtractError::Port`] when the rows lack the column or DuckDB cannot cast
    /// the value.
    pub fn watermark(&mut self, batches: &[RecordBatch]) -> Result<Option<String>, ExtractError> {
        let Some(column) = self.table.incremental.clone() else {
            return Ok(None);
        };
        match extreme(batches, &column, true)? {
            Some(value) => Ok(Some(self.duckdb_text(&column, value.as_ref())?)),
            None => Ok(None),
        }
    }

    /// The text DuckDB gives for `CAST(value AS VARCHAR)` of `value`, a value of the column
    /// `column` of the table. The value is bound as text and cast to the column's type by
    /// `COALESCE` with the column itself, read from no row, so no type name enters the SQL.
    pub(crate) fn duckdb_text(
        &mut self,
        column: &str,
        value: &dyn Array,
    ) -> Result<String, PortError> {
        let from = [
            quote_identifier(&self.attached.alias),
            ".".to_owned(),
            quote_identifier(&self.table.name),
        ]
        .concat();
        let sql = [
            "SELECT CAST(COALESCE((SELECT ",
            &quote_identifier(column),
            " FROM ",
            &from,
            " WHERE false), ?) AS VARCHAR)",
        ]
        .concat();
        let params = [SqlValue::Text(bound_text(value, 0)?)];
        let rows = self.attached.connection.query_arrow(&sql, &params)?;
        let text = rows
            .iter()
            .find(|batch| batch.num_rows() > 0)
            .and_then(|batch| batch.columns().first())
            .and_then(|column| {
                column
                    .as_string_opt::<i32>()
                    .map(|text| text.value(0).to_owned())
            });
        text.ok_or_else(|| PortError::Failed(format!("DuckDB returned no text for `{column}`")))
    }
}
