//! How a boundary between two segments carries its rows (MG-REQ-0633).

use std::io::Cursor;
use std::path::{Path, PathBuf};

use arrow::datatypes::SchemaRef;
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use mergus_ports::FileSystem;

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

/// Writes `batches`, of `schema`, as the Arrow IPC file at `path`.
pub(crate) fn write_arrow(
    files: &dyn FileSystem,
    path: &Path,
    schema: &SchemaRef,
    batches: &[RecordBatch],
) -> Result<(), EngineError> {
    let mut bytes = Vec::new();
    let mut writer =
        FileWriter::try_new(&mut bytes, schema).map_err(|error| boundary(path, &error))?;
    for batch in batches {
        writer
            .write(batch)
            .map_err(|error| boundary(path, &error))?;
    }
    writer.finish().map_err(|error| boundary(path, &error))?;
    drop(writer);
    files.write(path, &bytes)?;
    Ok(())
}

/// Reads the Arrow IPC file at `path`.
pub(crate) fn read_arrow(
    files: &dyn FileSystem,
    path: &Path,
) -> Result<Vec<RecordBatch>, EngineError> {
    let bytes = files.read(path)?;
    let reader =
        FileReader::try_new(Cursor::new(bytes), None).map_err(|error| boundary(path, &error))?;
    reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| boundary(path, &error))
}

fn boundary(path: &Path, error: &dyn std::error::Error) -> EngineError {
    EngineError::Boundary {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}
