//! The Arrow side of a load by chunks: the unfinished load to resume (MG-REQ-0418.2), the
//! rows a chunk appends to `__changes` (MG-REQ-0412.3, MG-REQ-0415.2) and the current rows
//! its last commit writes (MG-REQ-0418.1).

use std::collections::{BTreeMap, BTreeSet};

use arrow::array::{AsArray, BooleanArray, RecordBatch};
use arrow::compute::{cast, concat_batches, filter_record_batch};
use arrow::datatypes::{DataType, SchemaRef, TimeUnit, TimestampMicrosecondType};
use arrow::error::ArrowError;
use arrow::row::{OwnedRow, RowConverter, SortField};
use mergus_adapter::{AdapterError, StorageAdapter, VersionRef};
use mergus_ports::Timestamp;

use crate::RawError;
use crate::raw_rows::{Keys, METADATA, Metadata, conform, with_metadata};

/// A load of `__changes` with no row in `ops.loads` (MG-REQ-0418.2).
#[derive(Clone, Debug)]
pub(crate) struct Unfinished {
    /// Its load id.
    pub(crate) load_id: String,
    /// Its `_extracted_at`.
    pub(crate) extracted_at: Timestamp,
    /// A batch whose one row holds its largest key.
    pub(crate) last: RecordBatch,
}

/// The rows of `table` at `version`, `None` when there is no version or no such table yet.
pub(crate) fn read_or_none(
    adapter: &dyn StorageAdapter,
    table: &str,
    version: Option<&VersionRef>,
) -> Result<Option<RecordBatch>, RawError> {
    let Some(version) = version else {
        return Ok(None);
    };
    let stream = match adapter.read_at(table, version) {
        Ok(stream) => stream,
        Err(AdapterError::VersionUnavailable { .. }) => return Ok(None),
        Err(error) => return Err(RawError::Storage(error)),
    };
    let rows = |error: ArrowError| RawError::Rows {
        table: table.to_owned(),
        error,
    };
    let batches = stream.collect::<Result<Vec<_>, _>>().map_err(rows)?;
    let Some(first) = batches.first() else {
        return Ok(None);
    };
    Ok(Some(
        concat_batches(&first.schema(), &batches).map_err(rows)?,
    ))
}

/// The `_load_id` of each row of `batch`.
fn load_ids(batch: &RecordBatch) -> Result<Vec<String>, ArrowError> {
    let column = batch
        .column_by_name(METADATA[0])
        .ok_or_else(|| ArrowError::SchemaError("the rows have no `_load_id`".to_owned()))?;
    let column = cast(column, &DataType::Utf8)?;
    Ok(column
        .as_string::<i32>()
        .iter()
        .map(|id| id.unwrap_or_default().to_owned())
        .collect())
}

/// The rows of `history` whose `_load_id` is `load_id`.
pub(crate) fn rows_of(history: &RecordBatch, load_id: &str) -> Result<RecordBatch, ArrowError> {
    let mask: BooleanArray = load_ids(history)?
        .iter()
        .map(|id| Some(id == load_id))
        .collect();
    filter_record_batch(history, &mask)
}

/// The rows of `history` whose `_load_id` is in `recorded`: what readers see of `__changes`
/// (MG-REQ-0518.1).
pub(crate) fn finished(
    history: &RecordBatch,
    recorded: &BTreeSet<String>,
) -> Result<RecordBatch, ArrowError> {
    let mask: BooleanArray = load_ids(history)?
        .iter()
        .map(|id| Some(recorded.contains(id)))
        .collect();
    filter_record_batch(history, &mask)
}

/// The latest unfinished load of `history`, by `_extracted_at` then load id, with its
/// largest key (MG-REQ-0418.2).
pub(crate) fn unfinished(
    history: &RecordBatch,
    recorded: &BTreeSet<String>,
    key: &[String],
) -> Result<Option<Unfinished>, ArrowError> {
    let ids = load_ids(history)?;
    let at = history
        .column_by_name(METADATA[1])
        .ok_or_else(|| ArrowError::SchemaError("the rows have no `_extracted_at`".to_owned()))?;
    let at = cast(
        at,
        &DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
    )?;
    let at = at.as_primitive::<TimestampMicrosecondType>();
    let latest = ids
        .iter()
        .enumerate()
        .filter(|(_, id)| !recorded.contains(*id))
        .map(|(row, id)| (at.value(row), id.clone()))
        .max();
    let Some((micros, load_id)) = latest else {
        return Ok(None);
    };
    let rows = rows_of(history, &load_id)?;
    let keys = Keys::new(&rows.schema(), key)?;
    let largest = keys
        .of(&rows)?
        .into_iter()
        .enumerate()
        .max_by(|left, right| left.1.cmp(&right.1))
        .map_or(0, |(row, _)| row);
    Ok(Some(Unfinished {
        load_id,
        extracted_at: Timestamp::from_unix_nanos(i128::from(micros) * 1_000),
        last: rows.slice(largest, 1),
    }))
}

/// The current rows of a keyed table, by key, with the comparable form of their source
/// columns.
pub(crate) struct Current {
    rows: RecordBatch,
    keys: Keys,
    converter: RowConverter,
    count: usize,
    by_key: BTreeMap<OwnedRow, (usize, OwnedRow)>,
}

impl Current {
    /// The current rows `rows`, of the raw `schema` whose first `count` columns are the
    /// source's, keyed by `key`; `None` for an empty table.
    pub(crate) fn new(
        rows: Option<&RecordBatch>,
        schema: &SchemaRef,
        count: usize,
        key: &[String],
    ) -> Result<Self, ArrowError> {
        let rows = match rows {
            Some(rows) => conform(rows, schema)?,
            None => RecordBatch::new_empty(schema.clone()),
        };
        let keys = Keys::new(schema, key)?;
        let sort = schema.fields().iter().take(count);
        let converter = RowConverter::new(
            sort.map(|field| SortField::new(field.data_type().clone()))
                .collect(),
        )?;
        let values = converter.convert_columns(&rows.columns()[..count])?;
        let mut by_key = BTreeMap::new();
        for (row, key) in keys.of(&rows)?.into_iter().enumerate() {
            by_key.insert(key, (row, values.row(row).owned()));
        }
        Ok(Self {
            rows,
            keys,
            converter,
            count,
            by_key,
        })
    }

    /// The rows of `chunk`, of the raw schema, that are not the current row of their key:
    /// a re-read row whose source columns are unchanged is not a new version (MG-REQ-0412.3).
    pub(crate) fn changed(&self, chunk: &RecordBatch) -> Result<RecordBatch, ArrowError> {
        let values = self
            .converter
            .convert_columns(&chunk.columns()[..self.count])?;
        let mask: BooleanArray = self
            .keys
            .of(chunk)?
            .iter()
            .enumerate()
            .map(|(row, key)| {
                Some(
                    self.by_key
                        .get(key)
                        .is_none_or(|(_, known)| known.row() != values.row(row)),
                )
            })
            .collect();
        filter_record_batch(chunk, &mask)
    }

    /// The rows of `chunk` whose key is current: a delete of a row that never was current, or
    /// was already deleted, is not a change.
    pub(crate) fn present(&self, chunk: &RecordBatch) -> Result<RecordBatch, ArrowError> {
        let mask: BooleanArray = self
            .keys
            .of(chunk)?
            .iter()
            .map(|key| Some(self.by_key.contains_key(key)))
            .collect();
        filter_record_batch(chunk, &mask)
    }

    /// The current rows once `load` is applied: without the keys it holds, then with its
    /// rows that are not deletes.
    pub(crate) fn merged(&self, load: &RecordBatch) -> Result<RecordBatch, ArrowError> {
        let touched = self.keys.set(std::slice::from_ref(load))?;
        let kept = self.without(&self.rows, &touched)?;
        let live = deleted_flags(load)?
            .iter()
            .map(|deleted| Some(!deleted))
            .collect::<BooleanArray>();
        let live = filter_record_batch(load, &live)?;
        concat_batches(&self.rows.schema(), &[kept, live])
    }

    /// The rows of `rows` whose key is not in `keys`.
    pub(crate) fn without(
        &self,
        rows: &RecordBatch,
        keys: &BTreeSet<OwnedRow>,
    ) -> Result<RecordBatch, ArrowError> {
        let mask: BooleanArray = self
            .keys
            .of(rows)?
            .iter()
            .map(|key| Some(!keys.contains(key)))
            .collect();
        filter_record_batch(rows, &mask)
    }

    /// The keys of `batches`.
    pub(crate) fn keys_of(
        &self,
        batches: &[RecordBatch],
    ) -> Result<BTreeSet<OwnedRow>, ArrowError> {
        self.keys.set(batches)
    }
}

/// The `_deleted` of each row of `batch`.
pub(crate) fn deleted_flags(batch: &RecordBatch) -> Result<Vec<bool>, ArrowError> {
    let column = batch
        .column_by_name(METADATA[2])
        .ok_or_else(|| ArrowError::SchemaError("the rows have no `_deleted`".to_owned()))?;
    Ok(column
        .as_boolean()
        .iter()
        .map(|deleted| deleted == Some(true))
        .collect())
}

/// `batches` of the source's rows, with the metadata of `metadata`, in one batch of the raw
/// `schema`; `deleted` marks them deletes (MG-REQ-0518.2).
pub(crate) fn stamped(
    batches: &[RecordBatch],
    source: &SchemaRef,
    schema: &SchemaRef,
    metadata: &Metadata,
    deleted: bool,
) -> Result<RecordBatch, ArrowError> {
    let count = source.fields().len();
    let mut stamped = Vec::new();
    for batch in batches {
        let batch = conform(batch, source)?;
        stamped.push(with_metadata(&batch, count, schema, metadata, deleted)?);
    }
    concat_batches(schema, &stamped)
}

/// The number of rows of `batch` that hold a value, for counts.
pub(crate) fn count(batch: &RecordBatch) -> i64 {
    i64::try_from(batch.num_rows()).unwrap_or(i64::MAX)
}
