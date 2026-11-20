//! The `independent-review` guard: the latest verdict on an implementation pull request is
//! `ready`, newer than the head commit, and posted by a session that wrote none of its
//! commits (MG-REQ-1734).

use std::collections::BTreeSet;

use super::github::{self, Api, Comment, Commit};
use super::{Event, branch, message};
use crate::failure::Failure;

/// The key of the verdict line.
const VERDICT: &str = "Verdict:";

/// The key of the session line.
const SESSION: &str = "Claude-Session:";

/// Returns the findings of the `independent-review` guard. Branches that are not `issue-`
/// branches pass.
///
/// # Errors
///
/// Fails if the API cannot be read or a date is not RFC 3339.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    if !branch::is_implementation(&event.branch) {
        return Ok(Vec::new());
    }
    let comments = github::comments(api, &event.repository, event.number)?;
    let Some((comment, verdict)) = comments
        .iter()
        .filter_map(|comment| line_value(&comment.body, VERDICT).map(|v| (comment, v)))
        .next_back()
    else {
        return Ok(vec![
            "MG-REQ-1734: no pull request comment has a `Verdict:` line".to_owned(),
        ]);
    };
    let commits = github::commits(api, &event.repository, event.number)?;
    let mut findings = Vec::new();
    if verdict != "ready" {
        findings.push(format!(
            "MG-REQ-1734: the latest verdict is `{verdict}`, not `ready`"
        ));
    }
    findings.extend(timing(comment, head(&commits, &event.head))?);
    findings.extend(session(comment, &commits));
    Ok(findings)
}

/// The head commit: the one with the event's head hash, or else the last one.
fn head<'a>(commits: &'a [Commit], sha: &str) -> Option<&'a Commit> {
    commits
        .iter()
        .find(|commit| commit.sha == sha)
        .or_else(|| commits.last())
}

/// The finding when the verdict was not created after the head commit's committer date.
fn timing(comment: &Comment, head: Option<&Commit>) -> Result<Option<String>, Failure> {
    let Some(head) = head else {
        return Ok(None);
    };
    if seconds(&comment.created)? > seconds(&head.committed)? {
        return Ok(None);
    }
    let sha = head.sha.get(..7).unwrap_or(&head.sha);
    Ok(Some(format!(
        "MG-REQ-1734: the latest verdict ({}) is not after the head commit {sha} ({})",
        comment.created, head.committed
    )))
}

/// The finding when the verdict carries no `Claude-Session:` value outside the commits'.
fn session(comment: &Comment, commits: &[Commit]) -> Option<String> {
    let authors: BTreeSet<&str> = commits
        .iter()
        .flat_map(|commit| message::values(&commit.message, "Claude-Session"))
        .collect();
    let values: Vec<&str> = comment
        .body
        .lines()
        .filter_map(|line| key_value(line, SESSION))
        .collect();
    if values.is_empty() {
        return Some("MG-REQ-1734: the latest verdict has no `Claude-Session:` line".to_owned());
    }
    if values.iter().any(|value| !authors.contains(value)) {
        return None;
    }
    Some(format!(
        "MG-REQ-1734: the latest verdict carries no Claude-Session value that differs from \
         those of the commits: {}",
        values.join(", ")
    ))
}

/// The value of the last line `<key> <value>` of a text.
fn line_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter_map(|line| key_value(line, key))
        .next_back()
}

/// The value of a line `<key> <value>`, trimmed; `None` when the line has another key or no
/// value.
fn key_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let value = line.trim().strip_prefix(key)?.trim();
    (!value.is_empty()).then_some(value)
}

/// Seconds since 1970-01-01T00:00:00Z of an RFC 3339 date such as `2026-01-02T10:00:00Z`
/// or `2026-01-02T12:00:00+02:00`. Fractions of a second are ignored.
///
/// # Errors
///
/// Fails if the date has another form.
fn seconds(date: &str) -> Result<i64, Failure> {
    let bad = || Failure::new(format!("`{date}` is not an RFC 3339 date"));
    let number = |from: usize, to: usize| -> Result<i64, Failure> {
        let text = date.get(from..to).ok_or_else(bad)?;
        if text.bytes().all(|byte| byte.is_ascii_digit()) {
            text.parse().map_err(|_| bad())
        } else {
            Err(bad())
        }
    };
    let separators = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];
    if separators
        .iter()
        .any(|(at, byte)| date.as_bytes().get(*at) != Some(byte))
    {
        return Err(bad());
    }
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let clock = number(11, 13)? * 3600 + number(14, 16)? * 60 + number(17, 19)?;
    let zone = date.get(19..).ok_or_else(bad)?;
    let zone = zone.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match zone.as_bytes().first() {
        Some(b'Z') if zone.len() == 1 => 0,
        Some(sign @ (b'+' | b'-')) if zone.len() == 6 && zone.as_bytes().get(3) == Some(&b':') => {
            let start = date.len() - 5;
            let minutes = number(start, start + 2)? * 60 + number(start + 3, start + 5)?;
            if *sign == b'+' {
                minutes * 60
            } else {
                -minutes * 60
            }
        }
        _ => return Err(bad()),
    };
    Ok(days(year, month, day) * 86_400 + clock - offset)
}

/// Days since 1970-01-01 of a date of the proleptic Gregorian calendar.
fn days(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_cycle - 719_468
}

#[cfg(test)]
mod tests {
    use super::seconds;

    #[test]
    fn mg_req_1734_dates_are_compared_in_utc() {
        assert_eq!(seconds("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(seconds("2000-03-01T00:00:00Z").unwrap(), 951_868_800);
        assert_eq!(
            seconds("2026-01-02T12:00:00+02:00").unwrap(),
            seconds("2026-01-02T10:00:00Z").unwrap()
        );
        assert_eq!(
            seconds("2026-01-02T08:30:00.123-01:30").unwrap(),
            seconds("2026-01-02T10:00:00Z").unwrap()
        );
        for bad in [
            "",
            "2026-01-02",
            "2026-01-02 10:00:00Z",
            "2026-01-02T10:00:00",
            "x",
        ] {
            assert!(seconds(bad).is_err(), "{bad}");
        }
    }
}
