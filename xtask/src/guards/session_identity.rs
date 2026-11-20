//! The `session-identity` guard: an implementation pull request is written by one session
//! that wrote no other implementation pull request (MG-REQ-1733).

use std::collections::BTreeSet;

use super::github::{self, Api};
use super::{Event, branch, message};
use crate::failure::Failure;

/// Returns the findings of the `session-identity` guard. Branches that are not `issue-`
/// branches pass.
///
/// # Errors
///
/// Fails if the API cannot be read.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    if !branch::is_implementation(&event.branch) {
        return Ok(Vec::new());
    }
    let commits = github::commits(api, &event.repository, event.number)?;
    let sessions: BTreeSet<&str> = commits
        .iter()
        .filter(|commit| commit.parents == 1)
        .flat_map(|commit| message::values(&commit.message, "Claude-Session"))
        .collect();
    if sessions.len() > 1 {
        let list: Vec<&str> = sessions.into_iter().collect();
        return Ok(vec![format!(
            "MG-REQ-1733: the commits carry {} Claude-Session values: {}",
            list.len(),
            list.join(", ")
        )]);
    }
    let Some(session) = sessions.first() else {
        return Ok(Vec::new());
    };
    reuses(event, api, session)
}

/// The findings for every other implementation pull request with a commit that carries
/// `session`.
fn reuses(event: &Event, api: &dyn Api, session: &str) -> Result<Vec<String>, Failure> {
    let mut others: Vec<u64> = github::pull_requests(api, &event.repository)?
        .into_iter()
        .filter(|pull| pull.number != event.number && branch::is_implementation(&pull.branch))
        .map(|pull| pull.number)
        .collect();
    others.sort_unstable();
    let mut findings = Vec::new();
    for number in others {
        let commits = github::commits(api, &event.repository, number)?;
        if commits
            .iter()
            .any(|commit| message::values(&commit.message, "Claude-Session").contains(&session))
        {
            findings.push(format!(
                "MG-REQ-1733: Claude-Session {session} also appears in implementation pull \
                 request #{number}"
            ));
        }
    }
    Ok(findings)
}
