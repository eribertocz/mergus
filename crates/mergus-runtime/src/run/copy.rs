//! Copies a step's rows into its candidate one batch at a time, checking the cancel token
//! between batches (MG-REQ-0708.1).

use std::path::Path;

use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use mergus_ports::{FileSystem, PortError};

use super::CancelToken;

/// Copies the Arrow IPC file `from` to `to`, batch by batch, and returns its rows; `None`
/// when `cancel` was set between two batches, leaving `to` incomplete.
pub(crate) fn copy_result(
    files: &dyn FileSystem,
    from: &Path,
    to: &Path,
    cancel: &CancelToken,
) -> Result<Option<u64>, PortError> {
    let arrow = |error: arrow::error::ArrowError| {
        PortError::Failed(format!("`{}`: {error}", from.display()))
    };
    let reader = FileReader::try_new(files.open(from)?, None).map_err(arrow)?;
    let mut writer = FileWriter::try_new(files.create(to)?, &reader.schema()).map_err(arrow)?;
    let mut rows: u64 = 0;
    for batch in reader {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        let batch = batch.map_err(arrow)?;
        writer.write(&batch).map_err(arrow)?;
        rows = rows.saturating_add(u64::try_from(batch.num_rows()).unwrap_or(u64::MAX));
    }
    writer.finish().map_err(arrow)?;
    Ok(Some(rows))
}
