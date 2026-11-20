//! The guards that judge a pull request (MG-REQ-1730 to MG-REQ-1733).
//!
//! Each guard reads the pull request from the GitHub event file named by `GITHUB_EVENT_PATH`,
//! or from the file given with `--event <path>`, and anything else through the GitHub REST
//! API. It prints one finding per line as `<guard>: <message>` on standard output and exits
//! with status 1 when it has a finding, 0 otherwise.

mod branch;
mod commit_format;
mod github;
mod message;
mod scope;
mod session_identity;
#[cfg(test)]
mod tests;

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
}

impl Event {
    /// Reads a GitHub `pull_request` event file.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be read, is not JSON, or lacks the repository, the number or
    /// the head branch of the pull request.
    pub(crate) fn read(path: &Path) -> Result<Self, Failure> {
        let json = Json::parse(&workspace::read(path)?)?;
        let missing = |what: &str| Failure::new(format!("{}: no {what}", path.display()));
        let pull_request = json
            .get("pull_request")
            .ok_or_else(|| missing("pull_request"))?;
        Ok(Self {
            repository: json
                .get("repository")
                .and_then(|repository| repository.get("full_name"))
                .and_then(Json::as_str)
                .ok_or_else(|| missing("repository.full_name"))?
                .to_owned(),
            number: pull_request
                .get("number")
                .and_then(Json::as_u64)
                .ok_or_else(|| missing("pull_request.number"))?,
            branch: pull_request
                .get("head")
                .and_then(|head| head.get("ref"))
                .and_then(Json::as_str)
                .ok_or_else(|| missing("pull_request.head.ref"))?
                .to_owned(),
        })
    }
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

/// Returns the findings of a guard, each one prefixed with `<guard>: `.
///
/// # Errors
///
/// Fails if the API cannot be read.
fn findings(guard: Guard, event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    Ok((guard.judge)(event, api)?
        .into_iter()
        .map(|message| format!("{}: {message}", guard.name))
        .collect())
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
        _ => Err(Failure::new("usage: cargo xtask <guard> [--event <path>]")),
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
