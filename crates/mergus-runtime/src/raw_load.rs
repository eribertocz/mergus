//! One load of a declared table, recorded in the load registry (MG-REQ-0514, MG-REQ-0517).

use mergus_adapter::{StorageAdapter, VersionRef};
use mergus_ports::Clock;

use crate::registry::{LoadMode, LoadRecord, LoadResult, micros, record_load};
use crate::{RawError, RawLoad, write_raw};

/// The watermarks of a load: the incremental column's value it started from and the one it
/// reached, in text. Both are `None` for a table without `incremental`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watermarks {
    /// `watermark_before`: `None` for a first load.
    pub before: Option<String>,
    /// `watermark_after`.
    pub after: Option<String>,
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
        Ok(_) => record(clock, load, mode, watermarks, None),
        Err(error) => {
            let failed = Watermarks {
                before: watermarks.before.clone(),
                after: watermarks.before.clone(),
            };
            record(clock, load, mode, &failed, Some(error))
        }
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
    let record = record(clock, load, LoadMode::RawOff, watermarks, None);
    record_load(ops, &record).map_err(RawError::Registry)
}

/// The row of `ops.loads` of `load`, read in `mode`, ending now by `clock`, failed with
/// `error` when there is one.
fn record(
    clock: &dyn Clock,
    load: &RawLoad,
    mode: LoadMode,
    watermarks: &Watermarks,
    error: Option<&RawError>,
) -> LoadRecord {
    let rows: usize = load.rows.iter().map(|batch| batch.num_rows()).sum();
    let elapsed = micros(clock.now()).saturating_sub(micros(load.extracted_at));
    LoadRecord {
        load_id: load.load_id.clone(),
        source: load.source.clone(),
        table: load.table.name.clone(),
        mode,
        rows: match error {
            Some(_) => 0,
            None => i64::try_from(rows).unwrap_or(i64::MAX),
        },
        watermark_before: watermarks.before.clone(),
        watermark_after: watermarks.after.clone(),
        started_at: load.extracted_at,
        duration_ms: elapsed.max(0) / 1_000,
        result: match error {
            Some(_) => LoadResult::Failed,
            None => LoadResult::Ok,
        },
        error_code: error.and_then(RawError::code).map(str::to_owned),
    }
}
