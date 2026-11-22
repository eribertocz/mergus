//! The files of the catalog: their names, titles, list lines and sections
//! (MG-REQ-1335.1 to MG-REQ-1335.3).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{Code, Finding, spec};
use crate::failure::Failure;
use crate::workspace;

/// The headings every file must have, each with text under it (MG-REQ-1335.2).
const HEADINGS: [&str; 3] = ["## Cause", "## Example", "## Fix"];

/// The line that marks a retired code.
const RETIRED: &str = "- Status: retired";

/// One file of the catalog whose name is a code.
pub(super) struct Entry {
    /// The code, from the file name.
    pub(super) code: Code,
    /// The path of the file, relative to the root, with `/` separators.
    pub(super) path: String,
    /// Whether the file has `- Status: retired`.
    pub(super) retired: bool,
}

/// What a file's list lines are checked against.
struct Known {
    /// The rule numbers of `spec/RULES.md`.
    rules: BTreeSet<u16>,
    /// The exit codes of `contracts/events.md` section 2.
    exit_codes: BTreeSet<u32>,
}

/// Reads every file under the catalog directory `dir` and checks it, returning the files
/// whose name is a code, sorted by code.
///
/// # Errors
///
/// Fails if a file cannot be read, or `spec/RULES.md` or `spec/contracts/events.md` cannot.
pub(super) fn read(
    root: &Path,
    dir: &Path,
    findings: &mut Vec<Finding>,
) -> Result<Vec<Entry>, Failure> {
    let known = Known {
        rules: spec::rules(root)?,
        exit_codes: spec::exit_codes(root)?,
    };
    let mut entries = Vec::new();
    for path in files(dir)? {
        let shown = workspace::show(root, &path);
        let code = path
            .parent()
            .filter(|parent| *parent == dir)
            .and(path.file_name())
            .and_then(|name| name.to_str()?.strip_suffix(".md"))
            .and_then(Code::parse);
        let Some(code) = code else {
            let message = "must be named `MG-<dddd>.md`".to_owned();
            findings.push(Finding::new(1, &shown, message));
            continue;
        };
        let text = workspace::read(&path)?;
        let entry = Entry {
            code,
            path: shown,
            retired: text.lines().any(|line| line.trim_end() == RETIRED),
        };
        entry.check(&text, &known, findings);
        entries.push(entry);
    }
    entries.sort_by_key(|entry| entry.code);
    Ok(entries)
}

/// Lists every file under `dir`, at any depth, sorted by path.
fn files(dir: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut files = Vec::new();
    for path in workspace::sorted_entries(dir)? {
        if path.is_dir() {
            files.extend(self::files(&path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

/// The value of the first line that starts with `prefix`.
fn value<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.trim_end().strip_prefix(prefix))
}

impl Entry {
    fn push(&self, findings: &mut Vec<Finding>, item: u8, message: String) {
        findings.push(Finding::new(item, &self.path, message));
    }

    /// Checks the text of the file.
    fn check(&self, text: &str, known: &Known, findings: &mut Vec<Finding>) {
        self.title(text, findings);
        let exit_code = value(text, "- Exit code: ").and_then(|code| code.parse().ok());
        if !exit_code.is_some_and(|code| known.exit_codes.contains(&code)) {
            let message = "lacks the line `- Exit code: <n>` with a code of \
                           `contracts/events.md` section 2";
            self.push(findings, 2, message.to_owned());
        }
        if !matches!(value(text, "- Retryable: "), Some("yes" | "no")) {
            let message = "lacks the line `- Retryable: yes` or `- Retryable: no`";
            self.push(findings, 2, message.to_owned());
        }
        self.rules(text, known, findings);
        self.sections(text, findings);
    }

    /// MG-REQ-1335.1: the first line is `# <code> · <title>`, with a non-empty title.
    fn title(&self, text: &str, findings: &mut Vec<Finding>) {
        let prefix = format!("# {} · ", self.code);
        let titled = text
            .lines()
            .next()
            .and_then(|line| line.strip_prefix(&prefix))
            .is_some_and(|title| !title.trim().is_empty());
        if !titled {
            let message = "the first line must be `# <code> · <title>` with the file's code \
                           and a non-empty title";
            self.push(findings, 1, message.to_owned());
        }
    }

    /// MG-REQ-1335.2 and MG-REQ-1335.3: the line `- Rules: ` lists known rule IDs, separated
    /// by `, `, in the area of the code.
    fn rules(&self, text: &str, known: &Known, findings: &mut Vec<Finding>) {
        let rules: Option<Vec<u16>> =
            value(text, "- Rules: ").and_then(|list| list.split(", ").map(spec::rule).collect());
        let Some(rules) = rules else {
            let message = "lacks the line `- Rules: ` followed by rule IDs separated by `, `";
            self.push(findings, 2, message.to_owned());
            return;
        };
        for rule in rules.iter().filter(|rule| !known.rules.contains(rule)) {
            let message = format!("lists MG-REQ-{rule:04}, which is not in `spec/RULES.md`");
            self.push(findings, 2, message);
        }
        if rules.iter().all(|rule| rule / 100 == 0) {
            if self.code.area() != 1 {
                let message = format!(
                    "{} lists kernel rules only, so it must be in MG-01xx",
                    self.code
                );
                self.push(findings, 3, message);
            }
            return;
        }
        for rule in rules.iter().filter(|rule| *rule / 100 != self.code.area()) {
            let message = format!("{} lists MG-REQ-{rule:04}, whose area differs", self.code);
            self.push(findings, 3, message);
        }
    }

    /// MG-REQ-1335.2: each heading of [`HEADINGS`] is present with text under it.
    fn sections(&self, text: &str, findings: &mut Vec<Finding>) {
        for heading in HEADINGS {
            let mut lines = text.lines().skip_while(|line| line.trim_end() != heading);
            if lines.next().is_none() {
                self.push(findings, 2, format!("lacks the heading `{heading}`"));
                continue;
            }
            let has_text = lines
                .take_while(|line| !line.starts_with('#'))
                .any(|line| !line.trim().is_empty());
            if !has_text {
                self.push(
                    findings,
                    2,
                    format!("has no text under the heading `{heading}`"),
                );
            }
        }
    }
}
