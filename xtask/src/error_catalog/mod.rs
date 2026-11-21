//! The `error-catalog` check: every error code has a complete file in
//! `crates/mergus-cli/errors/`, in the range of its rules and emitted by a test, every code
//! written in the code has a file, and the codes of an area have no gap (MG-REQ-1335).

mod catalog;
mod sources;
mod spec;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use crate::failure::Failure;
use catalog::Entry;

/// The catalog directory, relative to the workspace root.
const CATALOG: &str = "crates/mergus-cli/errors";

/// One problem, printed as `error-catalog: MG-REQ-1335.<item>: <path>: <message>`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    /// The path the finding is about, relative to the root, with `/` separators.
    path: String,
    /// The item of MG-REQ-1335 that fails.
    item: u8,
    /// What is wrong.
    message: String,
}

impl Finding {
    fn new(item: u8, path: &str, message: String) -> Self {
        Self {
            path: path.to_owned(),
            item,
            message,
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "error-catalog: MG-REQ-1335.{}: {}: {}",
            self.item, self.path, self.message
        )
    }
}

/// An error code, `MG-` and four digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Code(u16);

impl Code {
    /// Parses exactly `MG-<dddd>`.
    fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix("MG-")?;
        four_digits(digits).map(Self)
    }

    /// The area of the code: its first two digits.
    fn area(self) -> u16 {
        self.0 / 100
    }
}

impl fmt::Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MG-{:04}", self.0)
    }
}

/// Parses exactly four ASCII digits.
fn four_digits(text: &str) -> Option<u16> {
    (text.len() == 4 && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

/// Returns the codes written in `line`: `MG-` followed by exactly four digits and not by
/// another digit. `MG-REQ-` identifiers are never codes.
fn codes(line: &str) -> Vec<Code> {
    line.match_indices("MG-")
        .filter_map(|(start, _)| {
            let digits = line.get(start + 3..start + 7)?;
            let next = line.as_bytes().get(start + 7);
            if next.is_some_and(u8::is_ascii_digit) {
                return None;
            }
            four_digits(digits).map(Code)
        })
        .collect()
}

/// Checks the error catalog of the workspace at `root`.
///
/// # Errors
///
/// Fails with one line per finding, or if a file cannot be read.
pub(crate) fn check(root: &Path) -> Result<(), Failure> {
    let findings = findings(root)?;
    if findings.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    Err(Failure::new(format!(
        "check `error-catalog` failed:\n{}",
        lines.join("\n")
    )))
}

/// Returns the findings for the workspace at `root`, sorted. With no catalog directory, only
/// item 4 applies.
///
/// # Errors
///
/// Fails if a file cannot be read, or the specification files the catalog needs are missing.
fn findings(root: &Path) -> Result<Vec<Finding>, Failure> {
    let mut findings = Vec::new();
    let dir = root.join(CATALOG);
    let has_catalog = dir.is_dir();
    let entries = if has_catalog {
        catalog::read(root, &dir, &mut findings)?
    } else {
        Vec::new()
    };
    let sources = sources::read(root)?;
    sources::undocumented(&sources, &entries, &mut findings);
    if has_catalog {
        sources::untested(&sources, &entries, &mut findings);
        gaps(&entries, &mut findings);
    }
    findings.sort();
    findings.dedup();
    Ok(findings)
}

/// MG-REQ-1335.6: the codes of each area run from `<aa>01` to their highest without a gap.
fn gaps(entries: &[Entry], findings: &mut Vec<Finding>) {
    let mut areas: BTreeMap<u16, BTreeSet<u16>> = BTreeMap::new();
    for entry in entries {
        areas
            .entry(entry.code.area())
            .or_default()
            .insert(entry.code.0);
    }
    for entry in entries.iter().filter(|entry| entry.code.0 % 100 == 0) {
        let first = Code(entry.code.0 + 1);
        let message = format!(
            "{} is below {first}, where the codes of its area start",
            entry.code
        );
        findings.push(Finding::new(6, &entry.path, message));
    }
    for (area, numbers) in areas {
        let Some(highest) = numbers.last().copied() else {
            continue;
        };
        for number in area * 100 + 1..=highest {
            if !numbers.contains(&number) {
                let message = format!(
                    "{} is missing, so the codes of its area have a gap",
                    Code(number)
                );
                findings.push(Finding::new(6, &format!("{CATALOG}/"), message));
            }
        }
    }
}

#[cfg(test)]
mod tests;
