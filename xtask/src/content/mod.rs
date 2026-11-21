//! The content checks `language`, `size`, `secrets` and `sql-concatenation`: they read the
//! files tracked by git, not the build (MG-REQ-0176).

mod language;
mod lexer;
mod secrets;
mod size;
mod sql;

use std::fmt;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::failure::Failure;

/// The generated files of the specification, never scanned (MG-REQ-0176).
const GENERATED: [&str; 2] = ["spec/RULES.md", "spec/crates.md"];

/// One problem found in a file, printed as `<path>:<line>: <check>: <message>`.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding {
    /// The path of the file, relative to the scanned root, with `/` separators.
    pub(crate) path: String,
    /// The line, starting at 1. Findings about a whole file use line 1.
    pub(crate) line: usize,
    /// The name of the check.
    pub(crate) check: &'static str,
    /// What is wrong.
    pub(crate) message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}: {}: {}",
            self.path, self.line, self.check, self.message
        )
    }
}

/// A file to scan: its relative path and its text, or `None` when it is not UTF-8.
pub(crate) struct File {
    /// The path relative to the scanned root, with `/` separators.
    pub(crate) path: String,
    /// The contents, when the file is UTF-8 text.
    pub(crate) text: Option<String>,
}

impl File {
    /// Creates a finding about this file.
    fn finding(&self, line: usize, check: &'static str, message: String) -> Finding {
        Finding {
            path: self.path.clone(),
            line,
            check,
            message,
        }
    }
}

/// A content check: its name and the function that reports the findings of one file.
#[derive(Clone, Copy)]
pub(crate) struct Check {
    name: &'static str,
    findings: fn(&File) -> Vec<Finding>,
}

/// The `language` check (MG-REQ-0177). It only warns.
pub(crate) const LANGUAGE: Check = Check {
    name: "language",
    findings: language::findings,
};

/// The `size` check (MG-REQ-0178).
pub(crate) const SIZE: Check = Check {
    name: "size",
    findings: size::findings,
};

/// The `secrets` check (MG-REQ-0179).
pub(crate) const SECRETS: Check = Check {
    name: "secrets",
    findings: secrets::findings,
};

/// The `sql-concatenation` check (MG-REQ-0184).
pub(crate) const SQL_CONCATENATION: Check = Check {
    name: "sql-concatenation",
    findings: sql::findings,
};

/// Runs a failing check on the files tracked by git in `root`.
///
/// # Errors
///
/// Fails with one line per finding, or if git or a file cannot be read.
pub(crate) fn check(root: &Path, check: Check) -> Result<(), Failure> {
    report(check.name, &scan(root, &tracked(root)?, check)?)
}

/// Turns the findings of a failing check into its result.
///
/// # Errors
///
/// Fails with a line naming the check, then one line per finding, when there is any.
pub(crate) fn report(check: &str, findings: &[Finding]) -> Result<(), Failure> {
    if findings.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    Err(Failure::new(format!(
        "check `{check}` failed:\n{}",
        lines.join("\n")
    )))
}

/// Runs a warning check on the files tracked by git in `root` and returns its findings.
///
/// # Errors
///
/// Fails if git or a file cannot be read.
pub(crate) fn warnings(root: &Path, check: Check) -> Result<Vec<Finding>, Failure> {
    scan(root, &tracked(root)?, check)
}

/// Scans the in-scope files among `paths`, relative to `root`, and returns the findings
/// sorted by path and line. Tracked files missing from the working tree are skipped.
///
/// # Errors
///
/// Fails if a file exists but cannot be read.
pub(crate) fn scan(root: &Path, paths: &[String], check: Check) -> Result<Vec<Finding>, Failure> {
    let mut findings = Vec::new();
    for path in paths.iter().filter(|path| in_scope(path)) {
        let full = root.join(path);
        if !full.is_file() {
            continue;
        }
        let bytes = fs::read(&full).map_err(|error| Failure::io(&full, &error))?;
        let file = File {
            path: path.clone(),
            text: String::from_utf8(bytes).ok(),
        };
        findings.extend((check.findings)(&file));
    }
    findings.sort();
    Ok(findings)
}

/// Returns whether a path is scanned: not under a directory named `fixtures`, and not a
/// generated file of the specification (MG-REQ-0176).
fn in_scope(path: &str) -> bool {
    let path = Path::new(path);
    let in_fixtures = path
        .parent()
        .is_some_and(|dir| dir.components().any(|part| part.as_os_str() == "fixtures"));
    let generated = GENERATED.iter().any(|name| path == Path::new(name));
    !in_fixtures && !generated
}

/// Lists the files tracked by git in `root` (`git ls-files`).
///
/// # Errors
///
/// Fails if git cannot run, fails, or prints a non-UTF-8 path.
fn tracked(root: &Path) -> Result<Vec<String>, Failure> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .map_err(|error| Failure::new(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return Err(Failure::new(format!(
            "`git ls-files` failed: {}",
            output.status
        )));
    }
    let list = String::from_utf8(output.stdout)
        .map_err(|_| Failure::new("`git ls-files` printed a non-UTF-8 path"))?;
    Ok(list
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Returns the extension of the last part of a path, if it has one.
fn extension(path: &str) -> Option<&str> {
    let name = Path::new(path).file_name()?.to_str()?;
    name.rsplit_once('.')
        .and_then(|(stem, extension)| (!stem.is_empty()).then_some(extension))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::{Path, PathBuf};

    use super::{Check, in_scope, scan, tracked};
    use crate::workspace;

    /// Lists every file under `dir`, relative to `root`, with `/` separators.
    fn walk(root: &Path, dir: &Path, files: &mut Vec<String>) {
        for path in workspace::sorted_entries(dir).unwrap() {
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                let parts: Vec<_> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                files.push(parts.join("/"));
            }
        }
    }

    /// The directory of a fixture of the content checks.
    pub(crate) fn fixture_dir(check: &str, case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/content")
            .join(check)
            .join(case)
    }

    /// Runs a check on every file of a fixture and returns the printed findings.
    pub(crate) fn fixture_findings(check: Check, case: &str) -> Vec<String> {
        let root = fixture_dir(check.name, case);
        let mut files = Vec::new();
        walk(&root, &root, &mut files);
        scan(&root, &files, check)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Runs a check on the files tracked by git in this repository.
    pub(crate) fn repository_findings(check: Check) -> Vec<String> {
        let root = workspace::root().unwrap();
        scan(&root, &tracked(&root).unwrap(), check)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn mg_req_0176_fixtures_and_generated_files_are_skipped() {
        assert!(in_scope("xtask/src/main.rs"));
        assert!(in_scope("spec/kernel.md"));
        assert!(in_scope("docs/fixtures"));
        assert!(!in_scope("xtask/fixtures/content/a.rs"));
        assert!(!in_scope("fixtures/a.rs"));
        assert!(!in_scope("spec/RULES.md"));
        assert!(!in_scope("spec/crates.md"));
        assert!(in_scope("spec/status.md"));
    }

    #[test]
    fn mg_req_0176_tracked_files_include_this_module() {
        let root = workspace::root().unwrap();
        assert!(
            tracked(&root)
                .unwrap()
                .contains(&"xtask/src/main.rs".to_owned())
        );
    }

    #[test]
    fn mg_req_0176_findings_are_sorted_and_formatted() {
        let findings = fixture_findings(super::SECRETS, "order");
        assert_eq!(
            findings,
            [
                "a/key.pem:1: secrets: secret file (MG-REQ-0179.2)",
                "b.rs:2: secrets: commented-out code (MG-REQ-0179.5)",
                "b.rs:10: secrets: commented-out code (MG-REQ-0179.5)",
            ]
        );
    }
}
