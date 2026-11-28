//! The run limits of `--max-wall-time`, `--max-rows-written` and `--max-bytes-scanned`
//! (MG-REQ-0718.1): a wall time is a positive integer and `s`, `m` or `h`; rows and bytes are
//! positive integers.

use std::time::Duration;

use mergus_project::Configuration;
use mergus_runtime::Limits;

use crate::args::Invocation;
use crate::commands::LIMITS;
use crate::error::CliError;

/// A wall time: a positive integer and a unit, `s`, `m` or `h` (`"90s"`, `"30m"`, `"2h"`).
pub(crate) fn wall_time(text: &str) -> Option<Duration> {
    let split = text.len().checked_sub(1)?;
    let (number, unit) = (text.get(..split)?, text.get(split..)?);
    let seconds_per_unit = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        _ => return None,
    };
    let count = positive(number)?;
    count.checked_mul(seconds_per_unit).map(Duration::from_secs)
}

/// A positive integer, written in decimal digits only.
pub(crate) fn positive(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u64>().ok().filter(|value| *value > 0)
}

/// Fails when `value`, given to the limit option `name`, is not in its format.
///
/// # Errors
///
/// Fails with [`CliError::InvalidLimit`], an invalid command line (MG-REQ-0297).
pub(crate) fn check(name: &'static str, value: &str) -> Result<(), CliError> {
    let valid = if name == LIMITS[0] {
        wall_time(value).is_some()
    } else if LIMITS.contains(&name) {
        positive(value).is_some()
    } else {
        true
    };
    if valid {
        Ok(())
    } else {
        Err(CliError::InvalidLimit {
            option: name,
            given: value.to_owned(),
        })
    }
}

/// The limits the flags of `invocation` set, each the smaller of the flag and of `file`,
/// the `[limits]` of `project.toml`, when both are set (MG-REQ-0718.1).
pub(crate) fn of(invocation: &Invocation, file: Limits) -> Limits {
    Limits {
        wall_time: smaller(
            invocation.value(LIMITS[0]).and_then(wall_time),
            file.wall_time,
        ),
        rows_written: smaller(
            invocation.value(LIMITS[1]).and_then(positive),
            file.rows_written,
        ),
        bytes_scanned: smaller(
            invocation.value(LIMITS[2]).and_then(positive),
            file.bytes_scanned,
        ),
    }
}

/// The `[limits]` of `project.toml`, which its schema has checked (MG-REQ-0718.1).
pub(crate) fn of_project(configuration: &Configuration) -> Limits {
    let table = configuration.project.value.get("limits");
    let value = |key: &str| table.and_then(|table| table.get(key));
    Limits {
        wall_time: value("wall_time")
            .and_then(|text| text.as_str())
            .and_then(wall_time),
        rows_written: value("rows_written").and_then(serde_json::Value::as_u64),
        bytes_scanned: value("bytes_scanned").and_then(serde_json::Value::as_u64),
    }
}

fn smaller<T: Ord>(flag: Option<T>, file: Option<T>) -> Option<T> {
    match (flag, file) {
        (Some(flag), Some(file)) => Some(flag.min(file)),
        (flag, file) => flag.or(file),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{positive, wall_time};

    #[test]
    fn mg_req_0718_limit_formats() {
        assert_eq!(wall_time("90s"), Some(Duration::from_secs(90)));
        assert_eq!(wall_time("30m"), Some(Duration::from_secs(1800)));
        assert_eq!(wall_time("2h"), Some(Duration::from_secs(7200)));
        for invalid in ["", "s", "0s", "10", "10d", "-5m", "1.5h", "+3s", "10 s"] {
            assert_eq!(wall_time(invalid), None, "{invalid}");
        }
        assert_eq!(positive("1000"), Some(1000));
        for invalid in ["", "0", "-1", "1e3", "12x"] {
            assert_eq!(positive(invalid), None, "{invalid}");
        }
    }
}
