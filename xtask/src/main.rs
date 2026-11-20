//! Repository automation for Mergus, run as `cargo xtask <command>`.
//!
//! Each command runs one check of the specification and exits with a non-zero status when
//! the check fails:
//!
//! - `clippy`: every member inherits the workspace lints, and Clippy passes with
//!   `-D warnings` (MG-REQ-0009, MG-REQ-0171, MG-REQ-0172).
//! - `unsafe`: `unsafe` appears only in FFI crates, behind `// SAFETY:` comments
//!   (MG-REQ-0170).
//! - `missing-docs`: every member inherits the workspace lints, and the documentation builds
//!   with warnings denied (MG-REQ-0175).
//! - `reproducible-build`: two release builds produce identical binaries (MG-REQ-0113).
//! - `check-boundaries`: the crates respect the boundaries that `cargo metadata` shows
//!   (MG-REQ-0126).
//! - `language`: lists the lines with a letter outside ASCII; it only warns (MG-REQ-0177).
//! - `size`: Rust files and functions stay within the size limits (MG-REQ-0178).
//! - `secrets`: no secret, secret file, build artifact, data file or commented-out code is
//!   tracked (MG-REQ-0179).
//! - `sql-concatenation`: no SQL text is built with formatting macros (MG-REQ-0184).
//! - `crate-docs`: every crate documents its one responsibility (MG-REQ-0115).
//! - `scope`, `commit-format`, `session-identity`, `independent-review`, `issue-format`,
//!   `pr-format`, `deny-rules`, `repository-settings`: the guards that judge a pull request
//!   (MG-REQ-1730 to MG-REQ-1738). They print their findings and exit with status 1 when
//!   they have one.

#![forbid(unsafe_code)]

mod boundaries;
mod cargo;
mod content;
mod crate_docs;
mod failure;
mod guards;
mod json;
mod lints;
mod metadata;
mod reproducible;
mod unsafe_code;
mod workspace;

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

use failure::Failure;

const USAGE: &str = "usage: cargo xtask <clippy|unsafe|missing-docs|reproducible-build|\
     check-boundaries|language|size|secrets|sql-concatenation|crate-docs|scope|commit-format|\
     session-identity|independent-review|issue-format|pr-format|deny-rules|repository-settings>";

fn main() -> Result<ExitCode, Failure> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = args
        .split_first()
        .map_or(("", &[][..]), |(command, rest)| (command.as_str(), rest));
    match command {
        "scope" => guards::run(guards::SCOPE, rest),
        "commit-format" => guards::run(guards::COMMIT_FORMAT, rest),
        "session-identity" => guards::run(guards::SESSION_IDENTITY, rest),
        "independent-review" => guards::run(guards::INDEPENDENT_REVIEW, rest),
        "issue-format" => guards::run(guards::ISSUE_FORMAT, rest),
        "pr-format" => guards::run(guards::PR_FORMAT, rest),
        "deny-rules" => guards::run_deny_rules(rest),
        "repository-settings" => guards::run_repository_settings(rest),
        _ => check(command).map(|()| ExitCode::SUCCESS),
    }
}

/// Runs a check of the working tree.
fn check(command: &str) -> Result<(), Failure> {
    let root = workspace::root()?;
    match command {
        "clippy" => clippy(&root),
        "unsafe" => unsafe_code::check(&workspace::members(&root)?),
        "missing-docs" => missing_docs(&root),
        "reproducible-build" => reproducible::check(&root),
        "check-boundaries" => boundaries::check(&root),
        "language" => language(&root),
        "size" => content::check(&root, content::SIZE),
        "secrets" => content::check(&root, content::SECRETS),
        "sql-concatenation" => content::check(&root, content::SQL_CONCATENATION),
        "crate-docs" => crate_docs::check(&root),
        _ => Err(Failure::new(USAGE)),
    }
}

fn clippy(root: &Path) -> Result<(), Failure> {
    lints::check("clippy", &workspace::members(root)?)?;
    let args = [
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ];
    cargo::run(root, &args, &[])
}

fn missing_docs(root: &Path) -> Result<(), Failure> {
    lints::check("missing-docs", &workspace::members(root)?)?;
    let flags = OsString::from("-D warnings");
    let args = ["doc", "--workspace", "--no-deps", "--locked"];
    cargo::run(root, &args, &[("RUSTDOCFLAGS", &flags)])
}

/// Prints the findings of the `language` check on standard error, and always succeeds, even
/// when the files cannot be listed (MG-REQ-0177).
fn language(root: &Path) -> Result<(), Failure> {
    match content::warnings(root, content::LANGUAGE) {
        Ok(findings) => findings
            .iter()
            .for_each(|finding| report_warning(&finding.to_string())),
        Err(failure) => report_warning(&format!("{failure:?}")),
    }
    Ok(())
}

/// Prints one warning on standard error.
#[expect(
    clippy::print_stderr,
    reason = "MG-REQ-0177: the `language` check reports warnings without failing, so it \
              prints them itself; xtask is a developer tool, not a crate that logs"
)]
fn report_warning(warning: &str) {
    eprintln!("{warning}");
}
