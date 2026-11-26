//! The rows the kit writes and expects: one table with one `BIGINT` column `id`.

use std::sync::Arc;

use arrow::array::{ArrayRef, AsArray, Int64Array, RecordBatch, RecordBatchIterator};
use arrow::datatypes::{DataType, Field, Int64Type, Schema};
use mergus_adapter::{BatchStream, Capabilities, WriteMode};

/// The table every check writes.
pub(crate) const TABLE: &str = "mergus_certify";

/// The column every check writes.
const COLUMN: &str = "id";

/// Returns a stream of one batch holding `ids`.
pub(crate) fn stream(ids: &[i64]) -> Result<BatchStream, String> {
    let schema = Arc::new(Schema::new(vec![Field::new(COLUMN, DataType::Int64, true)]));
    let column: ArrayRef = Arc::new(Int64Array::from(ids.to_vec()));
    let batch = RecordBatch::try_new(Arc::clone(&schema), vec![column])
        .map_err(|error| format!("the kit cannot build its rows: {error}"))?;
    Ok(Box::new(RecordBatchIterator::new([Ok(batch)], schema)))
}

/// Returns the ids of `stream`, sorted: storages need not keep the order of rows.
pub(crate) fn ids(stream: BatchStream) -> Result<Vec<i64>, String> {
    let mut ids = Vec::new();
    for batch in stream {
        let batch = batch.map_err(|error| format!("read_at stream failed: {error}"))?;
        if batch.num_rows() == 0 {
            continue;
        }
        let column = batch
            .column_by_name(COLUMN)
            .and_then(|column| column.as_primitive_opt::<Int64Type>())
            .ok_or_else(|| format!("read_at returned no BIGINT column `{COLUMN}`"))?;
        ids.extend(column.iter().flatten());
    }
    ids.sort_unstable();
    Ok(ids)
}

/// Returns the ids of the table after writing `rows` with `mode` over `before`, sorted.
/// `None` for the modes the kit cannot drive: the contract does not carry their period or
/// key yet.
pub(crate) fn apply(mode: WriteMode, before: &[i64], rows: &[i64]) -> Option<Vec<i64>> {
    let mut after = match mode {
        WriteMode::Append => [before, rows].concat(),
        WriteMode::Replace => rows.to_vec(),
        WriteMode::ReplacePeriod | WriteMode::MergeByKey => return None,
    };
    after.sort_unstable();
    Some(after)
}

/// Returns the mode the checks write with: append when declared, else replace.
pub(crate) fn base_mode(capabilities: &Capabilities) -> Result<WriteMode, String> {
    [WriteMode::Append, WriteMode::Replace]
        .into_iter()
        .find(|mode| capabilities.write_modes.contains(mode))
        .ok_or_else(|| "the adapter declares neither append nor replace".to_owned())
}
