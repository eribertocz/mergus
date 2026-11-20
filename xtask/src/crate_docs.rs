//! The `crate-docs` check: every crate states its one responsibility and carries its
//! `README.md` and `ARCHITECTURE.md` (MG-REQ-0115).

use std::path::Path;

use crate::boundaries::crates;
use crate::content::{self, Finding};
use crate::failure::Failure;
use crate::workspace;

/// Documents must have fewer lines than this (MG-REQ-0115.4).
const MAX_DOCUMENT_LINES: usize = 150;

/// The headings `README.md` must contain (MG-REQ-0115.2).
const README_HEADINGS: [&str; 2] = ["## What it does", "## What it does not"];

/// The headings `ARCHITECTURE.md` must contain (MG-REQ-0115.3).
const ARCHITECTURE_HEADINGS: [&str; 3] = ["## Main types", "## Data flow", "## Dependencies"];

/// Checks the documentation of every member under `crates/` in `root`.
///
/// # Errors
///
/// Fails with one line per finding, or if a directory or file cannot be read.
pub(crate) fn check(root: &Path) -> Result<(), Failure> {
    content::report("crate-docs", &findings(root)?)
}

/// Returns the findings of every member under `crates/` in `root`, sorted by path and line.
///
/// # Errors
///
/// Fails if a directory or file cannot be read.
fn findings(root: &Path) -> Result<Vec<Finding>, Failure> {
    let mut findings = Vec::new();
    for member in workspace::members(root)? {
        if !member.dir.starts_with(root.join("crates")) {
            continue;
        }
        let Some(row) = crates::find(&member.name) else {
            continue;
        };
        let docs = Docs {
            dir: &member.dir,
            prefix: format!("crates/{}", member.name),
            responsibility: row.responsibility,
        };
        docs.source(&mut findings)?;
        docs.document("README.md", &README_HEADINGS, true, "0115.2", &mut findings)?;
        docs.document(
            "ARCHITECTURE.md",
            &ARCHITECTURE_HEADINGS,
            false,
            "0115.3",
            &mut findings,
        )?;
    }
    findings.sort();
    Ok(findings)
}

/// The documentation of one crate.
struct Docs<'a> {
    /// The directory of the crate.
    dir: &'a Path,
    /// The path of the crate relative to the root, such as `crates/mergus-plan`.
    prefix: String,
    /// The crate's responsibility in the crate table.
    responsibility: &'static str,
}

impl Docs<'_> {
    fn finding(&self, file: &str, line: usize, message: String) -> Finding {
        Finding {
            path: format!("{}/{file}", self.prefix),
            line,
            check: "crate-docs",
            message,
        }
    }

    /// MG-REQ-0115.1: the first `//!` line of `src/lib.rs`, or else `src/main.rs`.
    fn source(&self, findings: &mut Vec<Finding>) -> Result<(), Failure> {
        let file = ["src/lib.rs", "src/main.rs"]
            .into_iter()
            .find(|file| self.dir.join(file).is_file());
        let Some(file) = file else {
            let message = "missing, with no `src/main.rs` either (MG-REQ-0115.1)".to_owned();
            findings.push(self.finding("src/lib.rs", 1, message));
            return Ok(());
        };
        let text = workspace::read(&self.dir.join(file))?;
        let first = text
            .lines()
            .zip(1..)
            .find_map(|(line, number)| Some((line.trim().strip_prefix("//!")?.trim(), number)));
        let stated = first
            .is_some_and(|(doc, _)| doc.strip_suffix('.').unwrap_or(doc) == self.responsibility);
        if !stated {
            let line = first.map_or(1, |(_, number)| number);
            let message = format!(
                "the first `//!` line must be \"{}\" (MG-REQ-0115.1)",
                self.responsibility
            );
            findings.push(self.finding(file, line, message));
        }
        Ok(())
    }

    /// MG-REQ-0115.2 to MG-REQ-0115.4 for one document: it exists, contains its headings,
    /// and the responsibility when `states_responsibility`, and has fewer than 150 lines.
    fn document(
        &self,
        file: &str,
        headings: &[&str],
        states_responsibility: bool,
        item: &str,
        findings: &mut Vec<Finding>,
    ) -> Result<(), Failure> {
        let path = self.dir.join(file);
        if !path.is_file() {
            findings.push(self.finding(file, 1, format!("missing (MG-REQ-{item})")));
            return Ok(());
        }
        let text = workspace::read(&path)?;
        if states_responsibility && !text.contains(self.responsibility) {
            let message = format!("must contain \"{}\" (MG-REQ-{item})", self.responsibility);
            findings.push(self.finding(file, 1, message));
        }
        for heading in headings {
            if !text.lines().any(|line| line.trim_end() == *heading) {
                let message = format!("must contain the heading `{heading}` (MG-REQ-{item})");
                findings.push(self.finding(file, 1, message));
            }
        }
        let count = text.lines().count();
        if count >= MAX_DOCUMENT_LINES {
            let message = format!(
                "has {count} lines, must have fewer than {MAX_DOCUMENT_LINES} (MG-REQ-0115.4)"
            );
            findings.push(self.finding(file, 1, message));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::findings;
    use crate::workspace;

    fn fixture_findings(case: &str) -> Vec<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/content/crate-docs")
            .join(case);
        findings(&root)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn mg_req_0115_documented_crates_pass() {
        assert_eq!(fixture_findings("valid"), Vec::<String>::new());
    }

    #[test]
    fn mg_req_0115_this_repository_passes() {
        let root = workspace::root().unwrap();
        assert!(findings(&root).unwrap().is_empty());
    }

    #[test]
    fn mg_req_0115_1_wrong_or_missing_responsibility_line_fails() {
        let expected = [
            "crates/mergus-contracts/src/lib.rs:1: crate-docs: the first `//!` line must be \"Rust types generated from `schemas/`\" (MG-REQ-0115.1)",
            "crates/mergus-ports/src/lib.rs:1: crate-docs: missing, with no `src/main.rs` either (MG-REQ-0115.1)",
            "crates/mergus-project/src/lib.rs:1: crate-docs: the first `//!` line must be \"Read and validate the project: files, `project.toml`, folder `mergus.toml`\" (MG-REQ-0115.1)",
            "crates/mergus-sim/src/lib.rs:3: crate-docs: the first `//!` line must be \"Deterministic simulator\" (MG-REQ-0115.1)",
        ];
        assert_eq!(only(&fixture_findings("broken"), "0115.1"), expected);
    }

    #[test]
    fn mg_req_0115_2_incomplete_readme_fails() {
        let expected = [
            "crates/mergus-ports/README.md:1: crate-docs: missing (MG-REQ-0115.2)",
            "crates/mergus-project/README.md:1: crate-docs: must contain \"Read and validate the project: files, `project.toml`, folder `mergus.toml`\" (MG-REQ-0115.2)",
            "crates/mergus-project/README.md:1: crate-docs: must contain the heading `## What it does not` (MG-REQ-0115.2)",
        ];
        assert_eq!(only(&fixture_findings("broken"), "0115.2"), expected);
    }

    #[test]
    fn mg_req_0115_3_incomplete_architecture_fails() {
        let expected = [
            "crates/mergus-ports/ARCHITECTURE.md:1: crate-docs: missing (MG-REQ-0115.3)",
            "crates/mergus-project/ARCHITECTURE.md:1: crate-docs: must contain the heading `## Data flow` (MG-REQ-0115.3)",
        ];
        assert_eq!(only(&fixture_findings("broken"), "0115.3"), expected);
    }

    #[test]
    fn mg_req_0115_4_documents_of_150_lines_fail() {
        let expected = [
            "crates/mergus-plan/ARCHITECTURE.md:1: crate-docs: has 150 lines, must have fewer than 150 (MG-REQ-0115.4)",
            "crates/mergus-plan/README.md:1: crate-docs: has 150 lines, must have fewer than 150 (MG-REQ-0115.4)",
        ];
        assert_eq!(only(&fixture_findings("broken"), "0115.4"), expected);
    }

    fn only(findings: &[String], item: &str) -> Vec<String> {
        let suffix = format!("(MG-REQ-{item})");
        findings
            .iter()
            .filter(|finding| finding.ends_with(&suffix))
            .cloned()
            .collect()
    }
}
