//! The retention of raw (MG-REQ-0513, MG-REQ-0516) and the sources without raw
//! (MG-REQ-0517).

use std::time::Duration;

use mergus_adapter::{Retention, StorageAdapter, VersionRef};
use mergus_project::{Configuration, Source};
use serde_json::Value;

use crate::RawError;

/// The retention of a source that sets none: 30 days of loads (MG-REQ-0516).
pub const DEFAULT_RETENTION: Duration = Duration::from_secs(30 * DAY);

/// Seconds in a day, an hour and a minute.
const DAY: u64 = 86_400;
const HOUR: u64 = 3_600;
const MINUTE: u64 = 60;

/// Whether `source` keeps its loads in raw: it does unless it sets `raw = false`
/// (MG-REQ-0517).
#[must_use]
pub fn keeps_raw(source: &Source) -> bool {
    source.file.value.get("raw").and_then(Value::as_bool) != Some(false)
}

/// The names of the sources of `configuration` with `raw = false`, in its order.
#[must_use]
pub fn raw_off_sources(configuration: &Configuration) -> Vec<&str> {
    configuration
        .sources
        .iter()
        .filter(|source| !keeps_raw(source))
        .map(|source| source.name.as_str())
        .collect()
}

/// The `retention` of `source`, a number followed by `m`, `h` or `d`, or
/// [`DEFAULT_RETENTION`] when it sets none (MG-REQ-0516).
#[must_use]
pub fn source_retention(source: &Source) -> Duration {
    source
        .file
        .value
        .get("retention")
        .and_then(Value::as_str)
        .and_then(parse)
        .unwrap_or(DEFAULT_RETENTION)
}

/// The retention the raw catalog is expired with: the longest of the sources that keep raw,
/// since they share it (MG-REQ-0516.2). `None` when no source keeps raw.
#[must_use]
pub fn raw_retention(configuration: &Configuration) -> Option<Duration> {
    configuration
        .sources
        .iter()
        .filter(|source| keeps_raw(source))
        .map(source_retention)
        .max()
}

/// Expires the raw catalog through `raw`: maintenance work, never part of a run
/// (MG-REQ-0513). Versions younger than [`raw_retention`], counted from their commit
/// (MG-REQ-0516.1), and the versions `live` are kept; with no source keeping raw, the
/// default retention applies.
///
/// # Errors
///
/// Returns [`RawError::Storage`] when the adapter cannot expire.
pub fn expire_raw(
    raw: &mut dyn StorageAdapter,
    configuration: &Configuration,
    live: Vec<VersionRef>,
) -> Result<(), RawError> {
    let retention = Retention {
        live,
        rollback_window: raw_retention(configuration).unwrap_or(DEFAULT_RETENTION),
    };
    Ok(raw.expire(&retention)?)
}

/// `retention` as `project.toml` writes it: whole days as `<n>d`, else whole hours as
/// `<n>h`, else minutes as `<n>m`.
#[must_use]
pub fn show_retention(retention: Duration) -> String {
    let seconds = retention.as_secs();
    if seconds.is_multiple_of(DAY) {
        format!("{}d", seconds / DAY)
    } else if seconds.is_multiple_of(HOUR) {
        format!("{}h", seconds / HOUR)
    } else {
        format!("{}m", seconds / MINUTE)
    }
}

/// The duration `text`, such as `30d`, `12h` or `90m`.
fn parse(text: &str) -> Option<Duration> {
    let unit = match text.chars().last()? {
        'd' => DAY,
        'h' => HOUR,
        'm' => MINUTE,
        _ => return None,
    };
    let count: u64 = text.get(..text.len().saturating_sub(1))?.parse().ok()?;
    Some(Duration::from_secs(count.saturating_mul(unit)))
}
