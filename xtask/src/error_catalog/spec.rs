//! The parts of the specification the catalog files are checked against: the rule IDs of
//! `spec/RULES.md` and the exit codes of `spec/contracts/events.md` section 2.

use std::collections::BTreeSet;
use std::path::Path;

use crate::failure::Failure;
use crate::workspace;

/// The rule numbers listed in the first column of `spec/RULES.md`.
///
/// # Errors
///
/// Fails if the file cannot be read.
pub(super) fn rules(root: &Path) -> Result<BTreeSet<u16>, Failure> {
    let text = workspace::read(&root.join("spec").join("RULES.md"))?;
    Ok(text
        .lines()
        .filter_map(|line| first_cell(line)?.strip_prefix("MG-REQ-"))
        .filter_map(super::four_digits)
        .collect())
}

/// Parses exactly `MG-REQ-<dddd>`.
pub(super) fn rule(text: &str) -> Option<u16> {
    super::four_digits(text.strip_prefix("MG-REQ-")?)
}

/// The exit codes of the first table of section 2 of `spec/contracts/events.md`, a range
/// `<a>–<b>` standing for every code from `a` to `b`.
///
/// # Errors
///
/// Fails if the file cannot be read or section 2 has no table.
pub(super) fn exit_codes(root: &Path) -> Result<BTreeSet<u32>, Failure> {
    let path = root.join("spec").join("contracts").join("events.md");
    let text = workspace::read(&path)?;
    let table = text
        .lines()
        .skip_while(|line| !line.starts_with("## 2."))
        .skip(1)
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'));
    let mut codes = BTreeSet::new();
    for cell in table.filter_map(first_cell) {
        if let Some((low, high)) = cell.split_once('–') {
            if let (Ok(low), Ok(high)) = (low.trim().parse::<u32>(), high.trim().parse()) {
                codes.extend(low..=high);
            }
        } else if let Ok(code) = cell.parse() {
            codes.insert(code);
        }
    }
    if codes.is_empty() {
        return Err(Failure::new(format!(
            "{}: section 2 has no table of exit codes",
            workspace::shown(&path)
        )));
    }
    Ok(codes)
}

/// The trimmed first cell of a Markdown table row.
fn first_cell(line: &str) -> Option<&str> {
    line.strip_prefix('|')?.split('|').next().map(str::trim)
}
