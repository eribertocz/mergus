//! `release-gates`: the extension of the `pr-format` guard on `release/<version>` branches
//! (MG-REQ-1739, MG-REQ-1711). It is not a command of its own; `pr-format` prints its
//! findings. Files are read through the API, so the guard never runs code of the head
//! (MG-REQ-1722).

use super::github::{self, Api};
use super::{Event, markdown};
use crate::failure::Failure;

/// The prefix of a release branch.
pub(crate) const RELEASE_BRANCH: &str = "release/";

/// The root manifest, read at the head commit.
const MANIFEST: &str = "Cargo.toml";

/// The changelog, read at the head commit.
const CHANGELOG: &str = "CHANGELOG.md";

/// The roadmap, read at the base commit.
const ROADMAP: &str = "spec/roadmap.md";

/// The section of the body that links each gate to its evidence.
const GATES: &str = "## Gates";

/// The table of the workspace package in the manifest.
const WORKSPACE_PACKAGE: &str = "[workspace.package]";

/// Returns the findings of MG-REQ-1739 on a pull request from `release/<version>`.
///
/// # Errors
///
/// Fails if the API cannot be read, or the roadmap at the base has no gate table.
pub(crate) fn judge(event: &Event, api: &dyn Api, version: &str) -> Result<Vec<String>, Failure> {
    let repo = &event.repository;
    let mut findings = Vec::new();
    let manifest = github::file(api, repo, MANIFEST, &event.head)?;
    match workspace_version(&manifest) {
        Some(found) if found == version => {}
        Some(found) => findings.push(format!(
            "MG-REQ-1739.1: the workspace version in `{MANIFEST}` is `{found}`, not `{version}`"
        )),
        None => findings.push(format!(
            "MG-REQ-1739.1: `{MANIFEST}` has no workspace version; it should be `{version}`"
        )),
    }
    let changelog = github::file(api, repo, CHANGELOG, &event.head)?;
    if !has_version_section(&changelog, version) {
        findings.push(format!(
            "MG-REQ-1739.2: `{CHANGELOG}` has no `## {version}` section"
        ));
    }
    findings.extend(gate_findings(event, api)?);
    Ok(findings)
}

/// The findings of MG-REQ-1739.3: every gate of the roadmap at the base needs an item in the
/// `## Gates` section of the body.
fn gate_findings(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    let roadmap = github::file(api, &event.repository, ROADMAP, &event.base)?;
    let gates = gates(&roadmap).ok_or_else(|| {
        Failure::new(format!(
            "{ROADMAP}: section 2 has no table with gates in its first column"
        ))
    })?;
    let Some(section) = markdown::section(&event.body, GATES) else {
        return Ok(vec![format!(
            "MG-REQ-1739.3: the body lacks the heading `{GATES}`"
        )]);
    };
    Ok(gates
        .iter()
        .filter(|gate| !section.lines().any(|line| is_gate_item(line, gate)))
        .map(|gate| {
            format!("MG-REQ-1739.3: `{GATES}` has no item `- {gate}: [<text>](https://<url>)`")
        })
        .collect())
}

/// The `version` of the `[workspace.package]` table of a manifest, `None` when it has none.
fn workspace_version(manifest: &str) -> Option<&str> {
    manifest
        .lines()
        .map(str::trim)
        .skip_while(|line| *line != WORKSPACE_PACKAGE)
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "version").then_some(value)
        })
        .and_then(|value| {
            let value = value.trim().strip_prefix('"')?;
            value.split_once('"').map(|(version, _)| version)
        })
}

/// Whether a changelog has a heading `## <version>`, alone or followed by a space and more
/// text, such as a date.
fn has_version_section(changelog: &str, version: &str) -> bool {
    let heading = format!("## {version}");
    markdown::headings(changelog).iter().any(|line| {
        line.strip_prefix(&heading)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

/// The gates of a roadmap: the first column of the data rows of the first table in its
/// section 2, the level-2 heading numbered `2`. `None` when there is no such table.
pub(crate) fn gates(roadmap: &str) -> Option<Vec<String>> {
    let heading = markdown::headings(roadmap)
        .into_iter()
        .find(|line| is_section_two(line))?;
    let section = markdown::section(roadmap, heading)?;
    let gates: Vec<String> = section
        .lines()
        .map(str::trim)
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .skip(2)
        .filter_map(|row| row.trim_start_matches('|').split('|').next())
        .map(|cell| cell.trim().to_owned())
        .filter(|cell| !cell.is_empty())
        .collect();
    (!gates.is_empty()).then_some(gates)
}

/// Whether a heading is `## 2`, followed by anything but another digit, such as `## 2. Gates`.
fn is_section_two(heading: &str) -> bool {
    heading
        .strip_prefix("## 2")
        .is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// Whether a line is `- <gate>: [<text>](https://<url>)`, with nothing after the link but
/// blanks.
fn is_gate_item(line: &str, gate: &str) -> bool {
    line.trim_end()
        .strip_prefix("- ")
        .and_then(|rest| rest.strip_prefix(gate))
        .and_then(|rest| rest.strip_prefix(": "))
        .is_some_and(is_inline_link)
}

/// Whether a text is exactly one inline Markdown link whose URL starts with `https://`.
fn is_inline_link(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('[') else {
        return false;
    };
    let Some((label, rest)) = rest.split_once("](") else {
        return false;
    };
    let Some(url) = rest.strip_suffix(')') else {
        return false;
    };
    !label.is_empty()
        && !label.contains(['[', ']'])
        && url.len() > "https://".len()
        && url.starts_with("https://")
        && !url.contains(|c: char| c.is_whitespace() || c == '(' || c == ')')
}

#[cfg(test)]
mod tests {
    use super::{gates, is_gate_item, workspace_version};
    use crate::workspace;

    #[test]
    fn mg_req_1739_3_gate_items_need_an_https_link() {
        assert!(is_gate_item("- G: [run](https://x.test/1)", "G"));
        assert!(is_gate_item("- G: [a b](https://x.test/1)  ", "G"));
        for line in [
            "- G: [run](http://x.test/1)",
            "- G: [](https://x.test/1)",
            "- G: [run](https://)",
            "- G: [run](https://x.test/1) more",
            "- G: https://x.test/1",
            "- G [run](https://x.test/1)",
            "* G: [run](https://x.test/1)",
            "- Gx: [run](https://x.test/1)",
            "- G: [a](https://x.test/1 \"t\")",
        ] {
            assert!(!is_gate_item(line, "G"), "{line}");
        }
    }

    #[test]
    fn mg_req_1739_1_the_workspace_version_is_read_from_its_table() {
        assert_eq!(
            workspace_version(
                "[package]\nversion = \"1.0.0\"\n[workspace.package]\nversion = \"0.1.0\"\n"
            ),
            Some("0.1.0")
        );
        assert_eq!(
            workspace_version("[workspace.package]\nedition = \"2024\"\n[x]\nversion = \"1\"\n"),
            None
        );
    }

    #[test]
    fn mg_req_1739_3_the_gates_of_this_roadmap_are_read() {
        let root = workspace::root().unwrap();
        let roadmap = workspace::read(&root.join("spec").join("roadmap.md")).unwrap();
        assert!(gates(&roadmap).is_some_and(|gates| !gates.is_empty()));
        assert_eq!(gates("## 20. X\n| G |\n|---|\n| a |\n"), None);
    }
}
