//! Copies a step's rows into its candidate one batch at a time, checking the cancel token
//! (MG-REQ-0708.1), the wall time and the rows written (MG-REQ-0718.2) before each batch.

use std::path::Path;

use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use mergus_ports::{FileSystem, PortError};

use super::limits::Meter;
use super::{CancelToken, RunError};

/// Copies the Arrow IPC file `from` to `to`, batch by batch, and returns its rows; `None`
/// when `cancel` was set between two batches, leaving `to` incomplete. A limit stops it
/// before the batch that would exceed it, also leaving `to` incomplete; `failed` turns a
/// port error into the run's.
pub(crate) fn copy_result(
    files: &dyn FileSystem,
    from: &Path,
    to: &Path,
    cancel: &CancelToken,
    meter: &Meter<'_>,
    failed: impl Fn(PortError) -> RunError,
) -> Result<Option<u64>, RunError> {
    let arrow = |error: arrow::error::ArrowError| {
        failed(PortError::Failed(format!("`{}`: {error}", from.display())))
    };
    let reader = FileReader::try_new(files.open(from).map_err(&failed)?, None).map_err(arrow)?;
    let target = files.create(to).map_err(&failed)?;
    let mut writer = FileWriter::try_new(target, &reader.schema()).map_err(arrow)?;
    let mut rows: u64 = 0;
    for batch in reader {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        meter.check_time().map_err(RunError::Limit)?;
        let batch = batch.map_err(arrow)?;
        let batch_rows = u64::try_from(batch.num_rows()).unwrap_or(u64::MAX);
        meter.write_rows(batch_rows).map_err(RunError::Limit)?;
        writer.write(&batch).map_err(arrow)?;
        rows = rows.saturating_add(batch_rows);
    }
    writer.finish().map_err(arrow)?;
    Ok(Some(rows))
}
