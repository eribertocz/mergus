//! Arrow IPC files of a step's working directory, written and read one batch at a time
//! through the streaming `FileSystem` methods `create` and `open`.

use std::io::Write;
use std::path::Path;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use mergus_ports::FileSystem;

use crate::PolarsError;

/// Writes `batches` of `schema` as the Arrow IPC file `path`.
pub(crate) fn write(
    files: &dyn FileSystem,
    path: &Path,
    schema: &SchemaRef,
    batches: &[RecordBatch],
) -> Result<(), PolarsError> {
    let stream = files.create(path)?;
    let mut writer = FileWriter::try_new(stream, schema).map_err(|error| boundary(path, &error))?;
    for batch in batches {
        writer
            .write(batch)
            .map_err(|error| boundary(path, &error))?;
    }
    writer.finish().map_err(|error| boundary(path, &error))?;
    let mut stream = writer
        .into_inner()
        .map_err(|error| boundary(path, &error))?;
    stream.flush().map_err(|error| boundary(path, &error))
}

/// Reads the schema and the batches of the Arrow IPC file `path`.
pub(crate) fn read(
    files: &dyn FileSystem,
    path: &Path,
) -> Result<(SchemaRef, Vec<RecordBatch>), PolarsError> {
    let stream = files.open(path)?;
    let reader = FileReader::try_new(stream, None).map_err(|error| boundary(path, &error))?;
    let schema = reader.schema();
    let batches = reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| boundary(path, &error))?;
    Ok((schema, batches))
}

/// The error of a file that cannot be encoded or decoded.
fn boundary(path: &Path, error: &dyn std::fmt::Display) -> PolarsError {
    PolarsError::Boundary {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}
