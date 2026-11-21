//! `cargo xtask owner-queue`: the items that wait for the owner, read through the GitHub
//! REST API (MG-REQ-1713, workflow section 5).
//!
//! It lists the open pull requests with green CI and a `ready` verdict (`merge`), the open
//! `needs-owner` issues (`needs-owner`), and the merged release pull requests without a tag
//! (`tag`), one line `<kind> #<number> <title> <url>` each, in that order of kinds and by
//! ascending number within a kind. The repository is `GITHUB_REPOSITORY`, or else the one
//! of the `origin` remote; the token is `GITHUB_TOKEN`, as for the guards.

use std::collections::BTreeSet;
use std::process::{Command, ExitCode};

use super::github::{self, Api};
use super::release_gates::RELEASE_BRANCH;
use super::repository_settings::REQUIRED_CHECKS;
use super::{Event, branch, independent_review};
use crate::failure::Failure;
use crate::json::Json;

/// The label of the issues that wait for the owner.
const NEEDS_OWNER: &str = "needs-owner";

/// The conclusions of a check run that count as green (MG-REQ-1713.1).
const PASSING: [&str; 2] = ["success", "skipped"];

/// Items asked for in each page of check runs.
const PAGE_SIZE: usize = 100;

/// Prints the queue of the repository.
///
/// # Errors
///
/// Fails if an argument is given, the repository is unknown, or the API cannot be read.
pub(crate) fn run(args: &[String], api: &dyn Api) -> Result<ExitCode, Failure> {
    if !args.is_empty() {
        return Err(Failure::new("usage: cargo xtask owner-queue"));
    }
    let repo = repository()?;
    items(api, &repo)?.iter().for_each(|line| print_item(line));
    Ok(ExitCode::SUCCESS)
}

/// The lines of the queue of `repo`, as `owner/name`.
///
/// # Errors
///
/// Fails if the API cannot be read, or returns a pull request or an issue without a number.
pub(crate) fn items(api: &dyn Api, repo: &str) -> Result<Vec<String>, Failure> {
    let mut merge = Vec::new();
    for item in github::list(api, &format!("/repos/{repo}/pulls?state=open"))? {
        let event = Event::from_pull_request(repo, &item).map_err(|what| {
            Failure::new(format!("an open pull request of {repo} has no {what}"))
        })?;
        if is_green(api, repo, &event.head)? && independent_review::judge(&event, api)?.is_empty() {
            merge.push(line("merge", &item)?);
        }
    }
    let mut needs_owner = Vec::new();
    let issues = format!("/repos/{repo}/issues?state=open&labels={NEEDS_OWNER}");
    for item in github::list(api, &issues)? {
        if item.get("pull_request").is_none() {
            needs_owner.push(line(NEEDS_OWNER, &item)?);
        }
    }
    let tags: BTreeSet<String> = github::list(api, &format!("/repos/{repo}/tags"))?
        .iter()
        .map(|tag| github::text(tag.get("name")))
        .collect();
    let mut tag = Vec::new();
    for item in github::list(api, &format!("/repos/{repo}/pulls?state=closed"))? {
        if is_untagged_release(&item, &tags) {
            tag.push(line("tag", &item)?);
        }
    }
    let mut lines = Vec::new();
    for mut kind in [merge, needs_owner, tag] {
        kind.sort_by_key(|(number, _)| *number);
        lines.extend(kind.into_iter().map(|(_, text)| text));
    }
    Ok(lines)
}

/// Whether every required check of MG-REQ-1727 has a check run with conclusion `success` or
/// `skipped` on the commit `sha`, as the ruleset accepts (MG-REQ-1713.1). The API gives the
/// latest run of each name.
fn is_green(api: &dyn Api, repo: &str, sha: &str) -> Result<bool, Failure> {
    let mut succeeded = BTreeSet::new();
    for page in 1.. {
        let url =
            format!("/repos/{repo}/commits/{sha}/check-runs?per_page={PAGE_SIZE}&page={page}");
        let response = api.get(&url)?;
        let runs = response
            .get("check_runs")
            .ok_or_else(|| Failure::new(format!("GET {url} did not return check runs")))?
            .items();
        succeeded.extend(
            runs.iter()
                .filter(|run| {
                    run.get("conclusion")
                        .and_then(Json::as_str)
                        .is_some_and(|conclusion| PASSING.contains(&conclusion))
                })
                .map(|run| github::text(run.get("name"))),
        );
        if runs.len() < PAGE_SIZE {
            break;
        }
    }
    Ok(REQUIRED_CHECKS
        .iter()
        .all(|check| succeeded.contains(*check)))
}

/// Whether a pull request is merged from `release/<version>` and no tag `v<version>` exists
/// (MG-REQ-1713.3).
fn is_untagged_release(item: &Json, tags: &BTreeSet<String>) -> bool {
    let merged = item.get("merged_at").and_then(Json::as_str).is_some();
    let head = github::text(item.get("head").and_then(|head| head.get("ref")));
    head.strip_prefix(RELEASE_BRANCH).is_some_and(|version| {
        merged && branch::is_valid(&head) && !tags.contains(&format!("v{version}"))
    })
}

/// The number of an item and its line `<kind> #<number> <title> <url>` (MG-REQ-1713.4).
fn line(kind: &str, item: &Json) -> Result<(u64, String), Failure> {
    let number = item
        .get("number")
        .and_then(Json::as_u64)
        .ok_or_else(|| Failure::new("an item of the queue has no number"))?;
    let title = github::text(item.get("title"));
    let url = github::text(item.get("html_url"));
    Ok((number, format!("{kind} #{number} {title} {url}")))
}

/// The repository: `GITHUB_REPOSITORY`, or else the `owner/name` of the `origin` remote.
///
/// # Errors
///
/// Fails if neither gives one.
fn repository() -> Result<String, Failure> {
    if let Some(repo) = std::env::var("GITHUB_REPOSITORY")
        .ok()
        .filter(|repo| !repo.is_empty())
    {
        return Ok(repo);
    }
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .map_err(|error| Failure::new(format!("cannot run git: {error}")))?;
    String::from_utf8(output.stdout)
        .ok()
        .filter(|_| output.status.success())
        .and_then(|url| repository_from_remote(&url))
        .ok_or_else(|| {
            Failure::new("set GITHUB_REPOSITORY to <owner>/<name>; the origin remote names none")
        })
}

/// The `owner/name` of a remote URL: its last two path segments, without `.git`.
pub(crate) fn repository_from_remote(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let mut segments = url.rsplit(['/', ':']);
    let name = segments.next().filter(|name| !name.is_empty())?;
    let owner = segments.next().filter(|owner| !owner.is_empty())?;
    Some(format!("{owner}/{name}"))
}

/// Prints one item of the queue on standard output.
#[expect(
    clippy::print_stdout,
    reason = "MG-REQ-1713: `owner-queue` prints its items, one per line"
)]
fn print_item(item: &str) {
    println!("{item}");
}
