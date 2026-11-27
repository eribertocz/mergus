//! The load registry, `ops.loads` (MG-REQ-0514). Until `mergus-ops` exists, it is written
//! through the storage adapter of the ops zone by [`record_load`] alone, so that `mergus-ops`
//! can take it over.

use std::fmt::Write as _;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, AsArray, Int64Array, RecordBatch, RecordBatchIterator, StringArray,
    TimestampMicrosecondArray,
};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow::error::ArrowError;
use mergus_adapter::{AdapterError, StorageAdapter, VersionRef, WriteMode};
use mergus_ports::{Clock, Random, Timestamp};

use crate::RawError;

/// The table of the load registry inside the ops catalog: `ops.loads`.
pub const LOADS: &str = "loads";

/// The time zone of `started_at`.
const UTC: &str = "UTC";

/// How a load read its table: the `mode` of `ops.loads`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadMode {
    /// Every row of the table.
    Full,
    /// The rows from the last watermark.
    Incremental,
    /// A source with `raw = false`: nothing was written to raw (MG-REQ-0517.1).
    RawOff,
}

impl LoadMode {
    /// The value of the mode in `ops.loads`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Incremental => "incremental",
            Self::RawOff => "raw_off",
        }
    }
}

/// How a load ended: the `result` of `ops.loads`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LoadResult {
    /// The load was written.
    Ok,
    /// The load failed (MG-REQ-0514.1).
    Failed,
}

impl LoadResult {
    /// The value of the result in `ops.loads`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
        }
    }
}

/// One row of `ops.loads` (MG-REQ-0514).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadRecord {
    /// The load's id, a UUID version 7 from [`new_load_id`].
    pub load_id: String,
    /// The source's name.
    pub source: String,
    /// The declared table's name.
    pub table: String,
    /// How the table was read.
    pub mode: LoadMode,
    /// The rows read; 0 for a failed load.
    pub rows: i64,
    /// The incremental column's value the load started from, in text; `None` without one.
    pub watermark_before: Option<String>,
    /// The incremental column's value the load reached, in text; equal to
    /// `watermark_before` when the load failed.
    pub watermark_after: Option<String>,
    /// The load's `_extracted_at`.
    pub started_at: Timestamp,
    /// Milliseconds from `started_at` to the end of the load's commit.
    pub duration_ms: i64,
    /// How the load ended.
    pub result: LoadResult,
    /// The `MG-####` code of a failed load.
    pub error_code: Option<String>,
}

/// Appends `record` to `ops.loads` through `ops`, the adapter of the ops zone, and commits.
/// Returns the version of the commit.
///
/// # Errors
///
/// Returns the [`AdapterError`] of the write or of the commit.
pub fn record_load(
    ops: &mut dyn StorageAdapter,
    record: &LoadRecord,
) -> Result<VersionRef, AdapterError> {
    let batch = row(record).map_err(|error| AdapterError::Storage(Box::new(error)))?;
    let schema = batch.schema();
    ops.write(
        LOADS,
        WriteMode::Append,
        Box::new(RecordBatchIterator::new([Ok(batch)], schema)),
    )?;
    ops.commit()
}

/// The schema of `ops.loads`, in the order of MG-REQ-0514.
fn schema() -> SchemaRef {
    let text = |name: &str, nullable: bool| Field::new(name, DataType::Utf8, nullable);
    Arc::new(Schema::new(vec![
        text("load_id", false),
        text("source", false),
        text("table", false),
        text("mode", false),
        Field::new("rows", DataType::Int64, false),
        text("watermark_before", true),
        text("watermark_after", true),
        Field::new(
            "started_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some(UTC.into())),
            false,
        ),
        Field::new("duration_ms", DataType::Int64, false),
        text("result", false),
        text("error_code", true),
    ]))
}

/// `record` as a batch of one row of `ops.loads`.
fn row(record: &LoadRecord) -> Result<RecordBatch, ArrowError> {
    let text = |value: &str| Arc::new(StringArray::from(vec![value])) as ArrayRef;
    let optional =
        |value: &Option<String>| Arc::new(StringArray::from(vec![value.clone()])) as ArrayRef;
    let started =
        TimestampMicrosecondArray::from(vec![micros(record.started_at)]).with_timezone(UTC);
    RecordBatch::try_new(
        schema(),
        vec![
            text(&record.load_id),
            text(&record.source),
            text(&record.table),
            text(record.mode.name()),
            Arc::new(Int64Array::from(vec![record.rows])),
            optional(&record.watermark_before),
            optional(&record.watermark_after),
            Arc::new(started),
            Arc::new(Int64Array::from(vec![record.duration_ms])),
            text(record.result.name()),
            optional(&record.error_code),
        ],
    )
}

/// The `watermark_after` of the last load of `table` of `source` recorded in `ops.loads` at
/// `version`, where the next load starts (MG-REQ-0514.2). `None` when no load of the table is
/// recorded, or the last one has no watermark.
///
/// # Errors
///
/// Returns [`RawError::Registry`] when `ops.loads` cannot be read at `version`.
pub fn last_watermark(
    ops: &dyn StorageAdapter,
    version: &VersionRef,
    source: &str,
    table: &str,
) -> Result<Option<String>, RawError> {
    let unreadable = |error: ArrowError| RawError::Registry(AdapterError::Storage(Box::new(error)));
    let mut last = None;
    for batch in ops.read_at(LOADS, version).map_err(RawError::Registry)? {
        let batch = batch.map_err(unreadable)?;
        let column = |name: &str| {
            batch
                .column_by_name(name)
                .and_then(|column| column.as_string_opt::<i32>().cloned())
                .ok_or_else(|| unreadable(ArrowError::SchemaError(format!("no column `{name}`"))))
        };
        let (sources, tables, marks) = (
            column("source")?,
            column("table")?,
            column("watermark_after")?,
        );
        for index in 0..batch.num_rows() {
            if sources.value(index) == source && tables.value(index) == table {
                last = Some((!marks.is_null(index)).then(|| marks.value(index).to_owned()));
            }
        }
    }
    Ok(last.flatten())
}

/// A new load id: a UUID version 7, lowercase with hyphens, from the time of `clock` and the
/// bits of `random` (MG-REQ-0514).
#[must_use]
pub fn new_load_id(clock: &dyn Clock, random: &dyn Random) -> String {
    let millis = clock.now().unix_nanos().div_euclid(1_000_000);
    let millis = u64::try_from(millis.clamp(0, 0xFFFF_FFFF_FFFF)).unwrap_or(0);
    let mut bytes = [0_u8; 16];
    random.fill(&mut bytes[6..]);
    bytes[..6].copy_from_slice(&millis.to_be_bytes()[2..]);
    bytes[6] = 0x70 | (bytes[6] & 0x0F);
    bytes[8] = 0x80 | (bytes[8] & 0x3F);
    let mut id = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            id.push('-');
        }
        // Writing to a `String` cannot fail.
        let _ = write!(id, "{byte:02x}");
    }
    id
}

/// `at` in microseconds since the epoch, saturated to the range of `i64`.
pub(crate) fn micros(at: Timestamp) -> i64 {
    let micros = at.unix_nanos().div_euclid(1_000);
    i64::try_from(micros).unwrap_or(if micros < 0 { i64::MIN } else { i64::MAX })
}
