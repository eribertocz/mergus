//! The `issue-format` guard: the issue of an implementation pull request has the label
//! `impl` and every section of MG-REQ-1704, with text (MG-REQ-1735).

use super::github::{self, Api};
use super::{Event, branch, markdown};
use crate::failure::Failure;

/// The label every implementation issue carries.
const LABEL: &str = "impl";

/// The headings every implementation issue has, each with text.
const HEADINGS: [&str; 8] = [
    "## Rules",
    "## Crate",
    "## Read",
    "## Acceptance criteria",
    "## Out of scope",
    "## Files",
    "## Depends on",
    "## Parallel with",
];

/// The heading under which the issue names its rules.
pub(crate) const RULES: &str = "## Rules";

/// Returns the findings of the `issue-format` guard. Branches that are not `issue-<n>-`
/// branches pass.
///
/// # Errors
///
/// Fails if the API cannot be read.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    let Some(number) = branch::issue(&event.branch) else {
        return Ok(Vec::new());
    };
    let issue = github::issue(api, &event.repository, number)?;
    let mut findings = Vec::new();
    if !issue.labels.iter().any(|label| label == LABEL) {
        findings.push(format!(
            "MG-REQ-1735: issue #{number} lacks the label `{LABEL}`"
        ));
    }
    for heading in HEADINGS {
        match markdown::section(&issue.body, heading) {
            None => findings.push(format!(
                "MG-REQ-1735: issue #{number} lacks the heading `{heading}`"
            )),
            Some(text) if !markdown::has_text(&text) => findings.push(format!(
                "MG-REQ-1735: issue #{number} has no text under `{heading}`"
            )),
            Some(text) if heading == RULES && markdown::rule_ids(&text).is_empty() => findings
                .push(format!(
                    "MG-REQ-1735: issue #{number} names no MG-REQ- ID under `{RULES}`"
                )),
            Some(_) => {}
        }
    }
    Ok(findings)
}
