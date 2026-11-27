//! How a boundary between two segments carries its rows (MG-REQ-0633).

use std::io::Write;
use std::path::{Path, PathBuf};

use arrow::datatypes::SchemaRef;
use arrow::error::ArrowError;
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use mergus_ports::{FileSystem, SeekRead};

use crate::EngineError;

/// The memory budget of one boundary: 1 GiB of Arrow. It holds until the `[resources]`
/// section of the project sets it.
pub const MEMORY_BUDGET: u64 = 1 << 30;

/// How a boundary carries its rows. The plan records it (MG-REQ-0704); execution only applies
/// it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Transport {
    /// An Arrow IPC file in the run's work directory, read as Arrow.
    SharedMemory,
    /// A Parquet file in the work zone, read lazily.
    Parquet,
}

impl Transport {
    /// Returns the transport of a boundary of `arrow_bytes` bytes of Arrow: shared memory
    /// when it fits [`MEMORY_BUDGET`], Parquet otherwise.
    #[must_use]
    pub const fn for_size(arrow_bytes: u64) -> Self {
        if arrow_bytes <= MEMORY_BUDGET {
            Self::SharedMemory
        } else {
            Self::Parquet
        }
    }
}

/// A boundary file: where the rows of one segment are left for the next.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Boundary {
    /// How the file carries its rows.
    pub transport: Transport,
    /// The file: in the run's work directory for [`Transport::SharedMemory`], in the work
    /// zone for [`Transport::Parquet`].
    pub path: PathBuf,
}

/// Writes `batches`, of `schema`, as the Arrow IPC file at `path`, one batch at a time through
/// a stream of the file system. When a write fails, the partial file is removed.
pub(crate) fn write_arrow(
    files: &dyn FileSystem,
    path: &Path,
    schema: &SchemaRef,
    batches: &[RecordBatch],
) -> Result<(), EngineError> {
    let stream = files.create(path)?;
    let written = stream_arrow(stream, schema, batches).map_err(|error| boundary(path, &error));
    if written.is_err() {
        // The write error is the one reported; the partial file is removed on a best effort,
        // so that no reader finds a truncated boundary.
        files.remove_file(path).ok();
    }
    written
}

/// Writes `batches` as an Arrow IPC file to `stream`, then flushes it.
fn stream_arrow(
    stream: Box<dyn Write + Send>,
    schema: &SchemaRef,
    batches: &[RecordBatch],
) -> Result<(), ArrowError> {
    let mut writer = FileWriter::try_new(stream, schema)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;
    writer.into_inner()?.flush()?;
    Ok(())
}

/// Opens the Arrow IPC file at `path` as a reader that decodes one batch at a time through a
/// stream of the file system, so that the file is never held whole in memory.
pub(crate) fn open_arrow(
    files: &dyn FileSystem,
    path: &Path,
) -> Result<FileReader<Box<dyn SeekRead>>, EngineError> {
    let stream = files.open(path)?;
    FileReader::try_new(stream, None).map_err(|error| boundary(path, &error))
}

/// Returns the error of the boundary file at `path`.
pub(crate) fn boundary(path: &Path, error: &dyn std::error::Error) -> EngineError {
    EngineError::Boundary {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}
