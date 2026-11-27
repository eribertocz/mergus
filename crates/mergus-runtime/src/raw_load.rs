//! One load of a declared table, recorded in the load registry (MG-REQ-0514, MG-REQ-0517).

use mergus_adapter::{StorageAdapter, VersionRef};
use mergus_ports::{Clock, Timestamp};

use crate::registry::{LoadMode, LoadRecord, LoadResult, micros, record_load};
use crate::{RawError, RawLoad, write_raw};

/// The watermarks of a load: the incremental column's value it started from and the one it
/// reached, in text. Both are `None` for a table without `incremental`. With them, when the
/// last `compare` of the table ran.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watermarks {
    /// `watermark_before`: `None` for a first load.
    pub before: Option<String>,
    /// `watermark_after`.
    pub after: Option<String>,
    /// `deletes_checked_at`: the load's `_extracted_at` when it compared, else that of the
    /// table's previous row (MG-REQ-0415.3).
    pub deletes_checked_at: Option<Timestamp>,
}

/// Writes `load` into raw through `raw` with [`write_raw`], then records it in `ops.loads`
/// through `ops`, after the raw commit (MG-REQ-0514.2). A load that fails is recorded as
/// `failed`, with 0 rows, `watermark_after` equal to `watermark_before` and its code
/// (MG-REQ-0514.1). Its `mode` is `incremental` for a table with `incremental` and a
/// watermark to start from, and `full` otherwise.
///
/// Returns the version of the raw commit.
///
/// # Errors
///
/// - the [`RawError`] of [`write_raw`], once the failed load is recorded;
/// - [`RawError::Registry`] when the load cannot be recorded; a raw version written before is
///   then ignored, and the next load starts from the last recorded `watermark_after`.
pub fn load_raw(
    raw: &mut dyn StorageAdapter,
    ops: &mut dyn StorageAdapter,
    clock: &dyn Clock,
    load: &RawLoad,
    previous: Option<&VersionRef>,
    watermarks: &Watermarks,
) -> Result<VersionRef, RawError> {
    let mode = if load.table.incremental.is_some() && watermarks.before.is_some() {
        LoadMode::Incremental
    } else {
        LoadMode::Full
    };
    let written = write_raw(raw, load, previous);
    let record = match &written {
        Ok(_) => record(clock, load, mode, watermarks, Outcome::Ok),
        Err(error) => record(clock, load, mode, watermarks, Outcome::Failed(error.code())),
    };
    if let Err(error) = record_load(ops, &record) {
        return Err(written.err().unwrap_or(RawError::Registry(error)));
    }
    written
}

/// Records in `ops.loads`, through `ops`, the extraction `load` of a source with
/// `raw = false`, with `mode = 'raw_off'`; nothing is written to raw (MG-REQ-0517.1).
/// Returns the version of the ops commit.
///
/// # Errors
///
/// Returns [`RawError::Registry`] when the load cannot be recorded.
pub fn load_raw_off(
    ops: &mut dyn StorageAdapter,
    clock: &dyn Clock,
    load: &RawLoad,
    watermarks: &Watermarks,
) -> Result<VersionRef, RawError> {
    let record = record(clock, load, LoadMode::RawOff, watermarks, Outcome::Ok);
    record_load(ops, &record).map_err(RawError::Registry)
}

/// Records in `ops.loads`, through `ops`, the failed extraction `load` of a source with
/// `raw = false`, with `mode = 'raw_off'`, `result = 'failed'`, 0 rows, `watermark_after`
/// equal to `watermark_before` and the failure's `code` (MG-REQ-0514.1, MG-REQ-0517.1).
/// Returns the version of the ops commit.
///
/// # Errors
///
/// Returns [`RawError::Registry`] when the load cannot be recorded.
pub fn load_raw_off_failed(
    ops: &mut dyn StorageAdapter,
    clock: &dyn Clock,
    load: &RawLoad,
    watermarks: &Watermarks,
    code: &str,
) -> Result<VersionRef, RawError> {
    let outcome = Outcome::Failed(Some(code));
    let record = record(clock, load, LoadMode::RawOff, watermarks, outcome);
    record_load(ops, &record).map_err(RawError::Registry)
}

/// How a load ended, for its row of `ops.loads`.
#[derive(Clone, Copy)]
pub(crate) enum Outcome<'a> {
    /// It was written.
    Ok,
    /// It failed, with the `MG-####` code of its error.
    Failed(Option<&'a str>),
}

/// The row of `ops.loads` of `load`, read in `mode`, ending now by `clock`. A failed load has
/// 0 rows and keeps its `watermark_before` (MG-REQ-0514.1).
pub(crate) fn record(
    clock: &dyn Clock,
    load: &RawLoad,
    mode: LoadMode,
    watermarks: &Watermarks,
    outcome: Outcome<'_>,
) -> LoadRecord {
    let rows: usize = load.rows.iter().map(|batch| batch.num_rows()).sum();
    let elapsed = micros(clock.now()).saturating_sub(micros(load.extracted_at));
    let (rows, after, result, error_code) = match outcome {
        Outcome::Ok => (
            i64::try_from(rows).unwrap_or(i64::MAX),
            watermarks.after.clone(),
            LoadResult::Ok,
            None,
        ),
        Outcome::Failed(code) => (
            0,
            watermarks.before.clone(),
            LoadResult::Failed,
            code.map(str::to_owned),
        ),
    };
    LoadRecord {
        load_id: load.load_id.clone(),
        source: load.source.clone(),
        table: load.table.name.clone(),
        mode,
        rows,
        watermark_before: watermarks.before.clone(),
        watermark_after: after,
        started_at: load.extracted_at,
        duration_ms: elapsed.max(0) / 1_000,
        result,
        error_code,
        deletes_checked_at: watermarks.deletes_checked_at,
    }
}
