//! The run limits of `--max-wall-time`, `--max-rows-written` and `--max-bytes-scanned`
//! (MG-REQ-0718.1): a wall time is a positive integer and `s`, `m` or `h`; rows and bytes are
//! positive integers.

use mergus_project::{Configuration, parse_positive as positive, parse_wall_time as wall_time};
use mergus_runtime::Limits;

use crate::args::Invocation;
use crate::commands::LIMITS;
use crate::error::CliError;

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
    let limits = configuration.limits();
    Limits {
        wall_time: limits.wall_time,
        rows_written: limits.rows_written,
        bytes_scanned: limits.bytes_scanned,
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

    use mergus_project::{parse_positive as positive, parse_wall_time as wall_time};

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
