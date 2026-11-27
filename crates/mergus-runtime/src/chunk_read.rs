//! The chunks of a load: each read, checked for a repeated key and committed to `__changes`
//! before the read moves on (MG-REQ-0412.3, MG-REQ-0415.2, MG-REQ-0418.1, MG-REQ-0518.3).

use std::collections::BTreeSet;

use arrow::array::RecordBatch;
use arrow::datatypes::SchemaRef;
use arrow::error::ArrowError;
use arrow::row::OwnedRow;
use mergus_source::{Chunk, IncrementalReader};

use crate::RawError;
use crate::chunk_load::{Stores, TableLoad, append, rows_error, source_schema};
use crate::chunk_rows::{Current, count, stamped};
use crate::chunk_step::{Progress, Shape, Step, record_failed};
use crate::load_error::LoadError;
use crate::raw_rows::{Metadata, raw_schema};

/// Reads every chunk of `reader`, appending and committing what each changes to `changes`
/// before the read moves on (MG-REQ-0418.1). A key read twice in the load, across all its
/// chunks and those committed before a resume, fails it with `MG-0504`, recorded `failed`
/// even after commits, so readers ignore its chunks (MG-REQ-0518.3).
pub(crate) fn read_chunks(
    stores: &mut Stores<'_>,
    reader: &mut IncrementalReader<'_>,
    step: &Step<'_>,
    current_rows: Option<&RecordBatch>,
    changes: &str,
    prior: Option<RecordBatch>,
) -> Result<Progress, LoadError> {
    let load = step.load;
    let mut progress = start(load, prior.as_ref());
    let mut committed = step.resumed;
    let metadata = step.metadata();
    loop {
        let chunk = match reader.next_chunk() {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return Ok(progress),
            Err(error) => return Err(record_failed(stores, step, committed, error.into())),
        };
        progress.read += chunk
            .rows
            .iter()
            .chain(&chunk.deleted)
            .map(count)
            .sum::<i64>();
        if progress.shape.is_none() {
            let seen = &mut progress.seen;
            let shape = shape_of(load, &chunk, current_rows, prior.as_ref(), seen);
            progress.shape = shape.map_err(|error| rows_error(load, error))?;
        }
        let Some((source, schema, rows)) = &progress.shape else {
            reader.committed();
            continue;
        };
        if let Some(error) = repeated_key(load, rows, &mut progress.seen, &chunk)? {
            return Err(record_failed(stores, step, false, error.into()));
        }
        let appended = chunk_changes(load, rows, source, schema, &metadata, &chunk)
            .map_err(|error| rows_error(load, error))?;
        if appended.num_rows() > 0 {
            let version = append(stores.raw, changes, appended);
            let version =
                version.map_err(|error| record_failed(stores, step, committed, error.into()))?;
            progress.raw_version = Some(version);
            committed = true;
        }
        reader.committed();
    }
}

/// The progress of a load before its first chunk: `prior`, the rows a resumed load committed,
/// count as read.
fn start(load: &TableLoad<'_>, prior: Option<&RecordBatch>) -> Progress {
    Progress {
        shape: None,
        raw_version: load.raw_version.clone(),
        read: prior.map_or(0, count),
        seen: BTreeSet::new(),
    }
}

/// The repeated-key error of `chunk` when it holds a key already read by the load, or twice
/// (MG-REQ-0518.3); a table without `key` has none.
fn repeated_key(
    load: &TableLoad<'_>,
    current: &Current,
    seen: &mut BTreeSet<OwnedRow>,
    chunk: &Chunk,
) -> Result<Option<RawError>, RawError> {
    if load.table.key.is_empty() {
        return Ok(None);
    }
    let read: Vec<RecordBatch> = chunk.rows.iter().chain(&chunk.deleted).cloned().collect();
    let key = current
        .repeated(seen, &read)
        .map_err(|error| rows_error(load, error))?;
    Ok(key.map(|key| RawError::RepeatedKey {
        source_name: load.source.name.clone(),
        table: load.table.name.clone(),
        key,
    }))
}

/// The shape of the rows of `chunk`, or of the current table when it has none, with the keys
/// of `prior`, the rows a resumed load committed, added to `seen`.
fn shape_of(
    load: &TableLoad<'_>,
    chunk: &Chunk,
    current_rows: Option<&RecordBatch>,
    prior: Option<&RecordBatch>,
    seen: &mut BTreeSet<OwnedRow>,
) -> Result<Option<Shape>, ArrowError> {
    let Some(source) = source_schema(&chunk.rows, &chunk.deleted, current_rows) else {
        return Ok(None);
    };
    let schema = raw_schema(&source);
    let count = source.fields().len();
    let rows = Current::new(current_rows, &schema, count, &load.table.key)?;
    if let Some(prior) = prior {
        *seen = rows.keys_of(std::slice::from_ref(prior))?;
    }
    Ok(Some((source, schema, rows)))
}

/// The rows a chunk appends to `__changes`: for a keyed table, its rows that are not the
/// current row of their key (MG-REQ-0412.3) and its deleted rows whose key is current, as
/// deletes (MG-REQ-0415.2); for a table without `key`, every row read.
fn chunk_changes(
    load: &TableLoad<'_>,
    current: &Current,
    source: &SchemaRef,
    schema: &SchemaRef,
    metadata: &Metadata,
    chunk: &Chunk,
) -> Result<RecordBatch, ArrowError> {
    let rows = stamped(&chunk.rows, source, schema, metadata, false)?;
    let deleted = stamped(&chunk.deleted, source, schema, metadata, true)?;
    if load.table.key.is_empty() {
        return arrow::compute::concat_batches(schema, &[rows, deleted]);
    }
    let rows = current.changed(&rows)?;
    let deleted = current.present(&deleted)?;
    arrow::compute::concat_batches(schema, &[rows, deleted])
}
