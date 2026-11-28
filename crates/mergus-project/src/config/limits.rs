//! The `[limits]` of `project.toml` (MG-REQ-0718.1).

use std::time::Duration;

use super::Configuration;

/// The run limits `project.toml` sets in `[limits]`, each `None` when unset (MG-REQ-0718.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProjectLimits {
    /// `wall_time`: the longest a run may take.
    pub wall_time: Option<Duration>,
    /// `rows_written`: the most rows a run may write.
    pub rows_written: Option<u64>,
    /// `bytes_scanned`: the most bytes of DuckLake files the plan may read.
    pub bytes_scanned: Option<u64>,
}

impl Configuration {
    /// Returns the `[limits]` of `project.toml`, which its schema has validated.
    #[must_use]
    pub fn limits(&self) -> ProjectLimits {
        let table = self.project.value.get("limits");
        let value = |key: &str| table.and_then(|table| table.get(key));
        ProjectLimits {
            wall_time: value("wall_time")
                .and_then(serde_json::Value::as_str)
                .and_then(parse_wall_time),
            rows_written: value("rows_written").and_then(serde_json::Value::as_u64),
            bytes_scanned: value("bytes_scanned").and_then(serde_json::Value::as_u64),
        }
    }
}

/// A wall time: a positive integer and a unit, `s`, `m` or `h` (`"90s"`, `"30m"`, `"2h"`);
/// `None` for any other text.
#[must_use]
pub fn parse_wall_time(text: &str) -> Option<Duration> {
    let split = text.len().checked_sub(1)?;
    let (number, unit) = (text.get(..split)?, text.get(split..)?);
    let seconds_per_unit = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        _ => return None,
    };
    let count = parse_positive(number)?;
    count.checked_mul(seconds_per_unit).map(Duration::from_secs)
}

/// A positive integer written in decimal digits only; `None` for any other text.
#[must_use]
pub fn parse_positive(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u64>().ok().filter(|value| *value > 0)
}
