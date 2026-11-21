//! The guards that judge a pull request (MG-REQ-1730 to MG-REQ-1738, and
//! `contract-compatibility` for MG-REQ-0205).
//!
//! Each guard reads the pull request from the GitHub event file named by `GITHUB_EVENT_PATH`,
//! or from the file given with `--event <path>`, and anything else through the GitHub REST
//! API. It prints one finding per line as `<guard>: <message>` on standard output and exits
//! with status 1 when it has a finding, 0 otherwise.

mod branch;
mod commit_format;
mod contract_compatibility;
mod deny_rules;
mod github;
mod independent_review;
mod issue_format;
mod markdown;
mod message;
mod owner_queue;
mod pr_format;
mod release_gates;
mod repository_settings;
mod scope;
mod session_identity;
#[cfg(test)]
pub(crate) use repository_settings::{GUARD_JOBS, REQUIRED_CHECKS};
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_compatibility;
#[cfg(test)]
mod tests_format;
#[cfg(test)]
mod tests_queue;
#[cfg(test)]
mod tests_settings;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::failure::Failure;
use crate::json::Json;
use crate::workspace;
use github::Api;

/// The pull request a guard judges, as the event file gives it.
pub(crate) struct Event {
    /// The repository, as `owner/name`.
    pub(crate) repository: String,
    /// The number of the pull request.
    pub(crate) number: u64,
    /// Its head branch.
    pub(crate) branch: String,
    /// The repository of its head branch, as `owner/name`, or `None` when the event gives
    /// none, as for a fork that was deleted.
    pub(crate) head_repository: Option<String>,
    /// Its base commit.
    pub(crate) base: String,
    /// Its head commit.
    pub(crate) head: String,
    /// Its body, empty when it has none.
    pub(crate) body: String,
}

impl Event {
    /// Reads a GitHub `pull_request` event file.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be read, is not JSON, or lacks the repository, the number,
    /// the head branch or the base and head commits of the pull request.
    pub(crate) fn read(path: &Path) -> Result<Self, Failure> {
        let json = Json::parse(&workspace::read(path)?)?;
        let missing = |what: &str| Failure::new(format!("{}: no {what}", workspace::shown(path)));
        let repository = json
            .get("repository")
            .and_then(|repository| repository.get("full_name"))
            .and_then(Json::as_str)
            .ok_or_else(|| missing("repository.full_name"))?;
        let pull_request = json
            .get("pull_request")
            .ok_or_else(|| missing("pull_request"))?;
        Self::from_pull_request(repository, pull_request)
            .map_err(|what| missing(&format!("pull_request.{what}")))
    }

    /// Reads a pull request of `repository` as the REST API and the event file give it.
    ///
    /// # Errors
    ///
    /// Fails with the name of the first missing field: `number`, `head.ref`, `base.sha` or
    /// `head.sha`.
    pub(crate) fn from_pull_request(
        repository: &str,
        pull_request: &Json,
    ) -> Result<Self, &'static str> {
        Ok(Self {
            repository: repository.to_owned(),
            number: pull_request
                .get("number")
                .and_then(Json::as_u64)
                .ok_or("number")?,
            branch: pull_request
                .get("head")
                .and_then(|head| head.get("ref"))
                .and_then(Json::as_str)
                .ok_or("head.ref")?
                .to_owned(),
            head_repository: pull_request
                .get("head")
                .and_then(|head| head.get("repo"))
                .and_then(|repo| repo.get("full_name"))
                .and_then(Json::as_str)
                .map(str::to_owned),
            base: sha(pull_request, "base").ok_or("base.sha")?,
            head: sha(pull_request, "head").ok_or("head.sha")?,
            body: github::text(pull_request.get("body")),
        })
    }

    /// Whether the pull request comes from a fork: its head repository is not its base
    /// repository, or is unknown (MG-REQ-1748).
    pub(crate) fn is_from_fork(&self) -> bool {
        self.head_repository.as_deref() != Some(self.repository.as_str())
    }
}

/// The commit `pull_request.<side>.sha` of an event.
fn sha(pull_request: &Json, side: &str) -> Option<String> {
    pull_request
        .get(side)
        .and_then(|side| side.get("sha"))
        .and_then(Json::as_str)
        .map(str::to_owned)
}

/// A guard: its name, which is also its command, and its findings on a pull request.
#[derive(Clone, Copy)]
pub(crate) struct Guard {
    /// The command and the prefix of every finding.
    pub(crate) name: &'static str,
    judge: fn(&Event, &dyn Api) -> Result<Vec<String>, Failure>,
}

/// The `scope` guard (MG-REQ-1731).
pub(crate) const SCOPE: Guard = Guard {
    name: "scope",
    judge: scope::judge,
};

/// The `commit-format` guard (MG-REQ-1732).
pub(crate) const COMMIT_FORMAT: Guard = Guard {
    name: "commit-format",
    judge: commit_format::judge,
};

/// The `session-identity` guard (MG-REQ-1733).
pub(crate) const SESSION_IDENTITY: Guard = Guard {
    name: "session-identity",
    judge: session_identity::judge,
};

/// The `independent-review` guard (MG-REQ-1734).
pub(crate) const INDEPENDENT_REVIEW: Guard = Guard {
    name: "independent-review",
    judge: independent_review::judge,
};

/// The `issue-format` guard (MG-REQ-1735).
pub(crate) const ISSUE_FORMAT: Guard = Guard {
    name: "issue-format",
    judge: issue_format::judge,
};

/// The `pr-format` guard (MG-REQ-1736).
pub(crate) const PR_FORMAT: Guard = Guard {
    name: "pr-format",
    judge: pr_format::judge,
};

/// The `contract-compatibility` guard (MG-REQ-0204, MG-REQ-0205).
pub(crate) const CONTRACT_COMPATIBILITY: Guard = Guard {
    name: "contract-compatibility",
    judge: contract_compatibility::judge,
};

/// What a guard found: findings, which fail it, and warnings, which do not.
#[derive(Default)]
pub(crate) struct Report {
    /// The findings, without the guard's prefix.
    pub(crate) findings: Vec<String>,
    /// The warnings, without the guard's prefix.
    pub(crate) warnings: Vec<String>,
}

/// Runs a guard with the command-line arguments that follow its name.
///
/// # Errors
///
/// Fails if the arguments are wrong, or the event or the API cannot be read.
pub(crate) fn run(guard: Guard, args: &[String]) -> Result<ExitCode, Failure> {
    let event = Event::read(&event_path(args)?)?;
    let lines = findings(guard, &event, &github::Rest::from_env())?;
    lines.iter().for_each(|line| print_finding(line));
    Ok(exit_code(&lines))
}

/// Runs the `deny-rules` guard (MG-REQ-1737). With `--base <revision>` it compares the
/// working tree with that revision; otherwise it compares the head commit of the event with
/// its base commit. Both are read with `git`, without the network.
///
/// # Errors
///
/// Fails if the arguments are wrong, the event cannot be read, a commit is missing, or the
/// base has no settings file that parses.
pub(crate) fn run_deny_rules(args: &[String]) -> Result<ExitCode, Failure> {
    let root = workspace::root()?;
    let (base, head) = match args {
        [flag, revision] if flag == "--base" => (
            deny_rules::at_revision(&root, revision)?,
            deny_rules::in_working_tree(&root)?,
        ),
        _ => {
            let event = Event::read(&event_path(args)?)?;
            (
                deny_rules::at_revision(&root, &event.base)?,
                deny_rules::at_revision(&root, &event.head)?,
            )
        }
    };
    let base = base
        .ok_or_else(|| Failure::new(format!("{} is missing at the base", deny_rules::SETTINGS)))?;
    let lines = prefixed(DENY_RULES, deny_rules::judge(&base, head.as_deref())?);
    lines.iter().for_each(|line| print_finding(line));
    Ok(exit_code(&lines))
}

/// Runs `cargo xtask owner-queue` (MG-REQ-1713).
///
/// # Errors
///
/// Fails if an argument is given, the repository is unknown, or the API cannot be read.
pub(crate) fn run_owner_queue(args: &[String]) -> Result<ExitCode, Failure> {
    owner_queue::run(args, &github::Rest::from_env())
}

/// Runs the `repository-settings` guard (MG-REQ-1738). Its warnings go to standard error and
/// do not change the exit status.
///
/// # Errors
///
/// Fails if the arguments are wrong, or the event or the API cannot be read.
pub(crate) fn run_repository_settings(args: &[String]) -> Result<ExitCode, Failure> {
    let event = Event::read(&event_path(args)?)?;
    let report = repository_settings::judge(&event, &github::Rest::from_env())?;
    for warning in report.warnings {
        print_warning(&format!("{REPOSITORY_SETTINGS}: warning: {warning}"));
    }
    let lines = prefixed(REPOSITORY_SETTINGS, report.findings);
    lines.iter().for_each(|line| print_finding(line));
    Ok(exit_code(&lines))
}

/// The name of the `deny-rules` guard.
pub(crate) const DENY_RULES: &str = "deny-rules";

/// The name of the `repository-settings` guard.
pub(crate) const REPOSITORY_SETTINGS: &str = "repository-settings";

/// Returns the findings of a guard, each one prefixed with `<guard>: `.
///
/// # Errors
///
/// Fails if the API cannot be read.
fn findings(guard: Guard, event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    Ok(prefixed(guard.name, (guard.judge)(event, api)?))
}

/// Prefixes each message with `<guard>: `.
fn prefixed(guard: &str, messages: Vec<String>) -> Vec<String> {
    messages
        .into_iter()
        .map(|message| format!("{guard}: {message}"))
        .collect()
}

/// Status 1 when there is a finding, 0 otherwise.
fn exit_code(findings: &[String]) -> ExitCode {
    if findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// The event file: `--event <path>`, or else `GITHUB_EVENT_PATH`.
fn event_path(args: &[String]) -> Result<PathBuf, Failure> {
    match args {
        [flag, path] if flag == "--event" => Ok(PathBuf::from(path)),
        [] => std::env::var_os("GITHUB_EVENT_PATH")
            .map(PathBuf::from)
            .ok_or_else(|| Failure::new("give `--event <path>` or set GITHUB_EVENT_PATH")),
        _ => Err(Failure::new(
            "usage: cargo xtask <guard> [--event <path>], or cargo xtask deny-rules \
             --base <revision>",
        )),
    }
}

/// Prints one finding on standard output.
// reason: MG-REQ-1730 asks each guard to print its findings, one per line.
#[expect(
    clippy::print_stdout,
    reason = "MG-REQ-1730: a guard prints its findings, one per line"
)]
fn print_finding(finding: &str) {
    println!("{finding}");
}

/// Prints one warning on standard error.
// reason: MG-REQ-1738 has the `repository-settings` guard warn without failing.
#[expect(
    clippy::print_stderr,
    reason = "MG-REQ-1738: the `repository-settings` guard warns without failing"
)]
fn print_warning(warning: &str) {
    eprintln!("{warning}");
}
