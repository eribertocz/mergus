//! The `commit-format` guard: every single-parent commit follows Conventional Commits, names
//! the issue of an implementation branch and carries the trailers (MG-REQ-1732).

use super::github::{self, Api, Commit};
use super::{Event, branch, message};
use crate::failure::Failure;

/// Returns the findings of the `commit-format` guard. Merge commits are not checked.
///
/// # Errors
///
/// Fails if the API cannot be read.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    let issue = branch::issue(&event.branch);
    Ok(github::commits(api, &event.repository, event.number)?
        .iter()
        .filter(|commit| commit.parents == 1)
        .flat_map(|commit| commit_findings(commit, issue))
        .collect())
}

/// The findings of one commit; `issue` is the issue of an implementation branch.
fn commit_findings(commit: &Commit, issue: Option<u64>) -> Vec<String> {
    let sha = commit.sha.get(..7).unwrap_or(&commit.sha);
    let subject = message::subject(&commit.message).trim_end();
    let mut problems: Vec<(&str, String)> = Vec::new();
    if !message::is_conventional(subject) {
        problems.push((
            "1732.1",
            "the subject is not <type>(<scope>)!: <text>".to_owned(),
        ));
    }
    if let Some(number) = issue
        && !subject.ends_with(&format!("(#{number})"))
    {
        problems.push((
            "1732.2",
            format!("the subject does not end with (#{number})"),
        ));
    }
    if message::values(&commit.message, "Co-Authored-By").is_empty() {
        problems.push(("1732.3", "lacks a Co-Authored-By: trailer".to_owned()));
    }
    if !message::values(&commit.message, "Claude-Session")
        .iter()
        .any(|value| message::is_session(value))
    {
        problems.push((
            "1732.3",
            "lacks a Claude-Session: trailer with a valid value".to_owned(),
        ));
    }
    problems
        .into_iter()
        .map(|(rule, text)| format!("MG-REQ-{rule}: commit {sha}: {text}"))
        .collect()
}
