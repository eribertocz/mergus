//! The codes written in the Rust sources under `crates/` (MG-REQ-1335.4, MG-REQ-1335.5).

use std::collections::BTreeSet;
use std::path::{Component, Path};

use super::catalog::Entry;
use super::{CATALOG, Code, Finding, codes};
use crate::failure::Failure;
use crate::workspace;

/// One code written in a `.rs` file under `crates/`.
pub(super) struct Written {
    /// The path of the file, relative to the root, with `/` separators.
    path: String,
    /// The line, starting at 1.
    line: usize,
    /// The code.
    code: Code,
    /// Whether the line is in a test source.
    in_test: bool,
}

/// Returns every code written in a `.rs` file under `crates/` in `root`, in path and line
/// order. Test sources are the files under a `tests/` directory, the files named `tests.rs`,
/// and the lines of any other file from its first `#[cfg(test)]` line to its end.
///
/// # Errors
///
/// Fails if a directory or file cannot be read.
pub(super) fn read(root: &Path) -> Result<Vec<Written>, Failure> {
    let crates = root.join("crates");
    if !crates.is_dir() {
        return Ok(Vec::new());
    }
    let mut written = Vec::new();
    for path in workspace::rust_files(&crates)? {
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let mut in_test = relative.parent().is_some_and(|parent| {
            parent
                .components()
                .any(|part| part == Component::Normal("tests".as_ref()))
        }) || relative.file_name().is_some_and(|name| name == "tests.rs");
        let shown = workspace::show(root, &path);
        for (text, line) in workspace::read(&path)?.lines().zip(1..) {
            in_test = in_test || text.trim() == "#[cfg(test)]";
            written.extend(codes(text).into_iter().map(|code| Written {
                path: shown.clone(),
                line,
                code,
                in_test,
            }));
        }
    }
    Ok(written)
}

/// MG-REQ-1335.4: a code written outside test sources has a file that is not retired.
pub(super) fn undocumented(written: &[Written], entries: &[Entry], findings: &mut Vec<Finding>) {
    for code in written.iter().filter(|code| !code.in_test) {
        let entry = entries.iter().find(|entry| entry.code == code.code);
        let problem = match entry {
            None => format!("has no file under {CATALOG}/"),
            Some(entry) if entry.retired => "is retired".to_owned(),
            Some(_) => continue,
        };
        let message = format!("line {}: {} {problem}", code.line, code.code);
        findings.push(Finding::new(4, &code.path, message));
    }
}

/// MG-REQ-1335.5: the code of every file that is not retired is written in a test source.
pub(super) fn untested(written: &[Written], entries: &[Entry], findings: &mut Vec<Finding>) {
    let tested: BTreeSet<Code> = written
        .iter()
        .filter(|code| code.in_test)
        .map(|code| code.code)
        .collect();
    for entry in entries.iter().filter(|entry| !entry.retired) {
        if !tested.contains(&entry.code) {
            let message = format!("{} is written in no test source", entry.code);
            findings.push(Finding::new(5, &entry.path, message));
        }
    }
}
