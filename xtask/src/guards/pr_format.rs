//! The `pr-format` guard: the body fills every section of the pull request template and, on
//! an implementation pull request, closes its issue and names the issue's rules
//! (MG-REQ-1736).

use super::github::{self, Api};
use super::issue_format::RULES;
use super::{Event, branch, markdown};
use crate::failure::Failure;

/// The pull request template, read at the base commit.
const TEMPLATE: &str = ".github/PULL_REQUEST_TEMPLATE.md";

/// The section that closes the issue.
const ISSUE: &str = "## Issue";

/// The section that names the rules covered.
const RULES_COVERED: &str = "## Rules covered";

/// Returns the findings of the `pr-format` guard.
///
/// # Errors
///
/// Fails if the API cannot be read.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    let template = github::file(api, &event.repository, TEMPLATE, &event.base)?;
    let body = &event.body;
    let mut findings = Vec::new();
    for heading in markdown::headings(&template) {
        match markdown::section(body, heading) {
            None => findings.push(format!(
                "MG-REQ-1736: the body lacks the heading `{heading}`"
            )),
            Some(text) if !markdown::has_text(&text) => findings.push(format!(
                "MG-REQ-1736: the body has no text under `{heading}`"
            )),
            Some(_) => {}
        }
    }
    if let Some(number) = branch::issue(&event.branch) {
        findings.extend(issue_findings(event, api, number)?);
    }
    Ok(findings)
}

/// The findings about the issue of an implementation pull request.
fn issue_findings(event: &Event, api: &dyn Api, number: u64) -> Result<Vec<String>, Failure> {
    let mut findings = Vec::new();
    if let Some(text) = markdown::section(&event.body, ISSUE)
        && !closes(&text, number)
    {
        findings.push(format!("MG-REQ-1736: `{ISSUE}` lacks `Closes #{number}`"));
    }
    let Some(covered) = markdown::section(&event.body, RULES_COVERED) else {
        return Ok(findings);
    };
    let issue = github::issue(api, &event.repository, number)?;
    let listed = markdown::section(&issue.body, RULES).unwrap_or_default();
    let named = markdown::rule_ids(&covered);
    let mut missing: Vec<&str> = markdown::rule_ids(&listed)
        .into_iter()
        .filter(|id| !named.contains(id))
        .collect();
    missing.dedup();
    findings.extend(missing.into_iter().map(|id| {
        format!(
            "MG-REQ-1736: `{RULES_COVERED}` does not name {id}, listed under `{RULES}` of \
             issue #{number}"
        )
    }));
    Ok(findings)
}

/// Whether a text holds `Closes #<number>` not followed by another digit.
fn closes(text: &str, number: u64) -> bool {
    let needle = format!("Closes #{number}");
    text.match_indices(&needle).any(|(at, _)| {
        !text
            .as_bytes()
            .get(at + needle.len())
            .is_some_and(u8::is_ascii_digit)
    })
}
