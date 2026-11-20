//! Read the rules and the current phase from the specification, in the format that
//! `scripts/spec_tool.py` parses (MG-REQ-0165, MG-REQ-0167).

use std::fmt;
use std::path::Path;

use crate::failure::Failure;
use crate::workspace;

/// Files at the top of `spec/` that are generated from the rules, not sources of rules.
const GENERATED: [&str; 3] = ["RULES.md", "crates.md", "status.md"];

/// The prefix of the line of `spec/roadmap.md` that names the current phase (MG-REQ-0167).
const CURRENT_PHASE: &str = "Current phase: ";

/// A phase of the roadmap, `<major>.<minor>`, ordered by number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Phase {
    major: u32,
    minor: u32,
}

impl Phase {
    /// Parses `<major>.<minor>`, `None` when the text has another form.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let (major, minor) = text.split_once('.')?;
        let number = |part: &str| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse().ok())
                .flatten()
        };
        Some(Self {
            major: number(major)?,
            minor: number(minor)?,
        })
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

/// One rule of the specification, with what the report needs.
pub(crate) struct Rule {
    /// The number of `MG-REQ-<number>`.
    pub(crate) number: u16,
    /// Its `Kind`, empty when it has none.
    kind: String,
    /// Its `Verify`, empty when it has none.
    verify: String,
    /// Its `Phase`, or else the phase of its file.
    pub(crate) phase: Phase,
}

impl Rule {
    /// The rule's ID, `MG-REQ-<number>`.
    pub(crate) fn id(&self) -> String {
        format!("MG-REQ-{:04}", self.number)
    }

    /// Whether the rule needs a `mg_req_<id>_` test: a behavior rule verified by a test
    /// (MG-REQ-0167).
    pub(crate) fn needs_test(&self) -> bool {
        self.kind == "behavior" && self.verify.starts_with("test ")
    }
}

/// Reads every rule under the specification directory `spec`, file by file in path order.
/// Decision records and generated files are skipped, as `scripts/spec_tool.py` does.
///
/// # Errors
///
/// Fails if a file cannot be read, or a rule has no phase that parses.
pub(crate) fn rules(spec: &Path) -> Result<Vec<Rule>, Failure> {
    let mut files = Vec::new();
    markdown_files(spec, &mut files)?;
    let mut rules = Vec::new();
    for file in files {
        let relative = file.strip_prefix(spec).unwrap_or(&file);
        let top = relative.components().count() == 1;
        let name = relative.to_string_lossy();
        if (top && GENERATED.contains(&name.as_ref())) || relative.starts_with("decisions") {
            continue;
        }
        let text = workspace::read(&file)?;
        rules.extend(
            parse_file(&text)
                .map_err(|message| Failure::new(format!("{}: {message}", file.display())))?,
        );
    }
    Ok(rules)
}

/// Reads the current phase from the line `Current phase: <major>.<minor>` of
/// `<spec>/roadmap.md` (MG-REQ-0167).
///
/// # Errors
///
/// Fails if the roadmap cannot be read or has no such line.
pub(crate) fn current_phase(spec: &Path) -> Result<Phase, Failure> {
    let path = spec.join("roadmap.md");
    workspace::read(&path)?
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix(CURRENT_PHASE)
                .and_then(Phase::parse)
        })
        .ok_or_else(|| {
            Failure::new(format!(
                "{}: no line `{CURRENT_PHASE}<major>.<minor>`",
                path.display()
            ))
        })
}

/// Collects every `.md` file under `dir`, sorted by path.
fn markdown_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), Failure> {
    for path in workspace::sorted_entries(dir)? {
        if path.is_dir() {
            markdown_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    Ok(())
}

/// The rules of one file. A rule starts at `### MG-REQ-<number> · <title>` outside a code
/// fence and ends at the next heading of level 1 to 3.
fn parse_file(text: &str) -> Result<Vec<Rule>, String> {
    let (file_phase, body) = header(text);
    let mut rules = Vec::new();
    let mut current: Option<(u16, Vec<(&str, &str)>)> = None;
    let mut fenced = false;
    for line in body {
        if line.starts_with("```") {
            fenced = !fenced;
        }
        if fenced {
            continue;
        }
        if let Some(number) = rule_number(line) {
            rules.extend(current.take().map(|rule| finish(rule, file_phase)));
            current = Some((number, Vec::new()));
        } else if is_heading(line) {
            rules.extend(current.take().map(|rule| finish(rule, file_phase)));
        } else if let Some((_, meta)) = current.as_mut() {
            meta.extend(meta_line(line));
        }
    }
    rules.extend(current.map(|rule| finish(rule, file_phase)));
    rules.into_iter().collect()
}

/// Builds a rule from its number and its metadata lines.
fn finish(
    (number, meta): (u16, Vec<(&str, &str)>),
    file_phase: Option<&str>,
) -> Result<Rule, String> {
    let value = |key: &str| {
        meta.iter()
            .rev()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    };
    let phase_text = value("Phase").or(file_phase).unwrap_or_default();
    let phase = Phase::parse(phase_text)
        .ok_or_else(|| format!("MG-REQ-{number:04}: phase `{phase_text}` does not parse"))?;
    Ok(Rule {
        number,
        kind: value("Kind").unwrap_or_default().to_owned(),
        verify: value("Verify").unwrap_or_default().to_owned(),
        phase,
    })
}

/// The `phase` of the YAML header, if any, and the lines after the header.
fn header(text: &str) -> (Option<&str>, Vec<&str>) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.first() != Some(&"---") {
        return (None, lines);
    }
    let Some(end) = lines.iter().skip(1).position(|line| *line == "---") else {
        return (None, lines);
    };
    let phase = lines[1..=end].iter().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        let value = value.split('#').next().unwrap_or_default().trim();
        (key.trim() == "phase").then(|| value.trim_matches('"'))
    });
    (phase, lines[end + 2..].to_vec())
}

/// The number of a rule heading `### MG-REQ-<4 digits> · <title>`.
fn rule_number(line: &str) -> Option<u16> {
    let rest = line.strip_prefix("### MG-REQ-")?;
    let digits = rest.get(..4)?;
    let title = rest.get(4..)?.strip_prefix(" · ")?;
    let valid = digits.bytes().all(|byte| byte.is_ascii_digit()) && !title.is_empty();
    valid.then(|| digits.parse().ok()).flatten()
}

/// Whether a line is a heading of level 1 to 3.
fn is_heading(line: &str) -> bool {
    ["# ", "## ", "### "]
        .iter()
        .any(|prefix| line.starts_with(prefix))
}

/// A metadata line `- <Key>: <value>`.
fn meta_line(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.strip_prefix("- ")?.split_once(": ")?;
    let word = !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || c == '_');
    word.then_some((key, value.trim()))
}
