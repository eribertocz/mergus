//! The `scope` guard: a pull request changes only the files its branch allows, and adds no
//! unexplained lint exception or ignored test (MG-REQ-1731).

use super::Event;
use super::branch;
use super::github::{self, Api, ChangedFile};
use crate::failure::Failure;

/// The specification scope of MG-REQ-1723. A path ending in `/` covers everything under it.
const SPECIFICATION_SCOPE: [&str; 7] = [
    "spec/",
    "AGENTS.md",
    "CLAUDE.md",
    ".claude/",
    ".github/ISSUE_TEMPLATE/",
    ".github/PULL_REQUEST_TEMPLATE.md",
    "scripts/spec_tool.py",
];

/// The owner's files of MG-REQ-1723, except `CODEOWNERS`, which [`is_owner_file`] finds in
/// any directory.
const OWNER_FILES: [&str; 2] = [".github/workflows/", "deny.toml"];

/// The label that authorizes an issue to change owner files (MG-REQ-1731).
const OWNER_AUTHORIZED: &str = "owner-authorized";

/// Lint attributes that need a `// reason:` comment. Written in two parts so that this file
/// does not trip the guard it defines.
const LINT_ATTRIBUTES: [&str; 3] = [
    concat!("#", "[allow("),
    concat!("#", "![allow("),
    concat!("#", "[expect("),
];

/// The attribute that ignores a test, written in two parts for the same reason.
const IGNORE: &str = concat!("#", "[ignore");

/// The comment that explains a lint attribute.
const REASON: &str = "// reason:";

/// Returns the findings of the `scope` guard.
///
/// # Errors
///
/// Fails if the API cannot be read.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    let name = &event.branch;
    let mut findings = Vec::new();
    if !branch::is_valid(name) {
        findings.push(format!(
            "MG-REQ-1731.1: branch `{name}` is not named issue-<n>-<words>, spec/<words> or \
             release/<major>.<minor>.<patch>"
        ));
    }
    let files = github::files(api, &event.repository, event.number)?;
    let allowed = Allowed::of(event, api)?;
    for file in &files {
        for path in &file.paths {
            findings.extend(allowed.finding(path));
        }
        findings.extend(attribute_findings(file));
    }
    Ok(findings)
}

/// The files a branch may change.
enum Allowed {
    /// An implementation branch: the issue's Files section, outside the specification scope,
    /// and owner files only with the label `owner-authorized` (MG-REQ-1731.2).
    Issue {
        number: u64,
        files: Vec<String>,
        owner_authorized: bool,
    },
    /// A `spec/` branch: the specification scope (MG-REQ-1731.3).
    Spec,
    /// A `release/` branch: version files and the changelog (MG-REQ-1731.4).
    Release,
    /// Any other branch, already reported by MG-REQ-1731.1.
    Any,
}

impl Allowed {
    fn of(event: &Event, api: &dyn Api) -> Result<Self, Failure> {
        let name = &event.branch;
        if let Some(number) = branch::issue(name) {
            let issue = github::issue(api, &event.repository, number)?;
            return Ok(Self::Issue {
                number,
                files: files_section(&issue.body),
                owner_authorized: issue.labels.iter().any(|label| label == OWNER_AUTHORIZED),
            });
        }
        Ok(if name.starts_with("spec/") {
            Self::Spec
        } else if name.starts_with("release/") {
            Self::Release
        } else {
            Self::Any
        })
    }

    fn finding(&self, path: &str) -> Option<String> {
        match self {
            Self::Issue {
                number,
                files,
                owner_authorized,
            } => {
                if !files.iter().any(|entry| covers(entry, path)) {
                    Some(format!(
                        "MG-REQ-1731.2: `{path}` is not in the Files section of issue #{number}"
                    ))
                } else if in_specification_scope(path) {
                    Some(format!(
                        "MG-REQ-1731.2: `{path}` is in the specification scope"
                    ))
                } else if is_owner_file(path) && !owner_authorized {
                    Some(format!(
                        "MG-REQ-1731.2: `{path}` is an owner file and issue #{number} lacks \
                         the label {OWNER_AUTHORIZED}"
                    ))
                } else {
                    None
                }
            }
            Self::Spec => (!in_specification_scope(path))
                .then(|| format!("MG-REQ-1731.3: `{path}` is outside the specification scope")),
            Self::Release => (!is_release_file(path)).then(|| {
                format!(
                    "MG-REQ-1731.4: `{path}` is not a Cargo.toml, a pyproject.toml or \
                     CHANGELOG.md"
                )
            }),
            Self::Any => None,
        }
    }
}

/// The paths of an issue's "Files" section: one per line, skipping blank lines and HTML
/// comments, up to the next heading.
fn files_section(body: &str) -> Vec<String> {
    body.lines()
        .skip_while(|line| line.trim() != "## Files")
        .skip(1)
        .take_while(|line| !line.starts_with('#'))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("<!--"))
        .map(str::to_owned)
        .collect()
}

/// Whether `entry` covers `path`: the same path, or a directory ending in `/` above it.
fn covers(entry: &str, path: &str) -> bool {
    entry == path || (entry.ends_with('/') && path.starts_with(entry))
}

fn in_specification_scope(path: &str) -> bool {
    SPECIFICATION_SCOPE.iter().any(|entry| covers(entry, path))
}

fn is_owner_file(path: &str) -> bool {
    OWNER_FILES.iter().any(|entry| covers(entry, path)) || file_name(path) == "CODEOWNERS"
}

fn is_release_file(path: &str) -> bool {
    matches!(file_name(path), "Cargo.toml" | "pyproject.toml") || path == "CHANGELOG.md"
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The findings of MG-REQ-1731.5 in the added lines of a file.
fn attribute_findings(file: &ChangedFile) -> Vec<String> {
    let (Some(path), Some(patch)) = (file.paths.first(), &file.patch) else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    let mut previous = "";
    for (number, line, added) in new_lines(patch) {
        if added {
            let lint = LINT_ATTRIBUTES.iter().any(|text| line.contains(text));
            if lint && !has_reason(line) && !has_reason(previous) {
                findings.push(format!(
                    "MG-REQ-1731.5: {path}:{number} adds an allow or expect attribute \
                     without a `// reason: <text>` comment"
                ));
            }
            if line.contains(IGNORE) {
                findings.push(format!(
                    "MG-REQ-1731.5: {path}:{number} adds an ignored test"
                ));
            }
        }
        previous = line;
    }
    findings
}

/// Whether a line carries `// reason:` followed by some text.
fn has_reason(line: &str) -> bool {
    line.split_once(REASON)
        .is_some_and(|(_, text)| !text.trim().is_empty())
}

/// The lines of the new file that a unified diff shows: its line number, its text and
/// whether the diff adds it. Removed lines are skipped.
fn new_lines(patch: &str) -> Vec<(usize, &str, bool)> {
    let mut lines = Vec::new();
    let mut number = 0;
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            number = hunk_start(header);
        } else if let Some(text) = line.strip_prefix('+') {
            lines.push((number, text, true));
            number += 1;
        } else if let Some(text) = line.strip_prefix(' ') {
            lines.push((number, text, false));
            number += 1;
        }
    }
    lines
}

/// The first line of the new file in a hunk header `-a,b +c,d @@`.
fn hunk_start(header: &str) -> usize {
    header
        .split_whitespace()
        .find_map(|part| part.strip_prefix('+'))
        .and_then(|range| range.split(',').next())
        .and_then(|start| start.parse().ok())
        .unwrap_or(0)
}
