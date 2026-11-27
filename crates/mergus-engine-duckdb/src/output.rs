//! The output of a segment: the step's rows streamed from DuckDB one batch at a time,
//! conformed to the declared schema, and left at the output boundary (MG-REQ-0633).

use std::path::{Path, PathBuf};

use arrow::record_batch::RecordBatch;
use mergus_ports::{Database, DatabaseConnection, DatabaseLocation, FileSystem, PortError};

use crate::exact::{Zones, conform, nullable};
use crate::segment::{create_table, path_value, quote_identifier};
use crate::transport::ArrowSink;
use crate::{EngineError, Segment};

/// The table of the output session a Parquet output is gathered in before it is written.
const OUTPUT_TABLE: &str = "mergus_output";

/// Streams the rows of the step's SQL out of `session`, conforms each batch to the declared
/// output schema and gives it to `sink`, so that Rust never holds more than one batch.
fn stream(
    session: &mut dyn DatabaseConnection,
    segment: &Segment,
    sink: &mut dyn FnMut(&RecordBatch) -> Result<(), EngineError>,
) -> Result<(), EngineError> {
    let output = &segment.output;
    let mut failure = None;
    let streamed = session.query_arrow_each(&segment.sql, &segment.params, &mut |batch| {
        let handled = conform(&output.name, &batch, &output.schema, Zones::Restore)
            .and_then(|batch| sink(&batch));
        handled.map_err(|error| {
            failure = Some(error);
            PortError::Failed("the segment stopped".to_owned())
        })
    });
    match (streamed, failure) {
        (_, Some(error)) => Err(error),
        (Err(error), None) => Err(error.into()),
        (Ok(()), None) => Ok(()),
    }
}

/// Writes the rows of `segment` as its Arrow IPC file, one batch at a time. A file whose write
/// failed is removed, so that no reader finds it truncated.
pub(crate) fn write_shared(
    session: &mut dyn DatabaseConnection,
    files: &dyn FileSystem,
    segment: &Segment,
) -> Result<(), EngineError> {
    let path = &segment.output.boundary.path;
    let mut sink = match ArrowSink::create(files, path, &nullable(&segment.output.schema)) {
        Ok(sink) => sink,
        Err(error) => {
            files.remove_file(path).ok();
            return Err(error);
        }
    };
    let streamed = stream(session, segment, &mut |batch| sink.write(batch));
    let written = match streamed {
        Ok(()) => sink.finish(),
        Err(error) => {
            // The stream is closed before the file is removed, so no buffer writes it again.
            drop(sink);
            Err(error)
        }
    };
    if written.is_err() {
        // The write error is the one reported; the partial file is removed on a best effort.
        files.remove_file(path).ok();
    }
    written
}

/// Returns the DuckDB file that holds a Parquet output before it is written: next to the
/// output file, in the run's work directory, named after it.
pub(crate) fn scratch(segment: &Segment) -> PathBuf {
    let path = &segment.output.boundary.path;
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".duckdb");
    path.with_file_name(name)
}

/// Writes the rows of `segment` as its Parquet file without holding them in memory: each
/// batch is appended to a table of a second DuckDB session backed by [`scratch`], which
/// DuckDB's `COPY` then writes. The scratch file, and its write-ahead log, are removed
/// whether the write succeeds or fails.
pub(crate) fn write_parquet(
    session: &mut dyn DatabaseConnection,
    database: &dyn Database,
    files: &dyn FileSystem,
    segment: &Segment,
) -> Result<(), EngineError> {
    let scratch = scratch(segment);
    remove(files, &scratch);
    let written = gather_and_copy(session, database, segment, &scratch);
    remove(files, &scratch);
    written
}

/// Gathers the rows of `segment` in the DuckDB file `scratch` and copies them to the Parquet
/// file of its output. The second session is closed when this returns.
fn gather_and_copy(
    session: &mut dyn DatabaseConnection,
    database: &dyn Database,
    segment: &Segment,
    scratch: &Path,
) -> Result<(), EngineError> {
    let output = &segment.output;
    let mut gathered = database.connect(&DatabaseLocation::File(scratch.to_path_buf()))?;
    create_table(
        gathered.as_mut(),
        &output.name,
        OUTPUT_TABLE,
        &output.schema,
    )?;
    let table = OUTPUT_TABLE;
    stream(session, segment, &mut |batch| {
        gathered
            .append_arrow(table, batch)
            .map_err(EngineError::from)
    })?;
    let path = path_value(&output.boundary)?;
    let quoted = quote_identifier(OUTPUT_TABLE);
    // sql: identifiers quoted by quote_identifier
    let copy = format!("COPY {quoted} TO ? (FORMAT parquet)");
    gathered.execute(&copy, &[path])?;
    Ok(())
}

/// Removes the DuckDB file `path` and its write-ahead log, when they exist.
fn remove(files: &dyn FileSystem, path: &Path) {
    let mut wal = path.as_os_str().to_os_string();
    wal.push(".wal");
    for file in [path.to_path_buf(), PathBuf::from(wal)] {
        // A file that does not exist needs no removal; any other failure leaves a scratch
        // file the run's cleanup removes with its work directory.
        if files.exists(&file).unwrap_or(false) {
            files.remove_file(&file).ok();
        }
    }
}
