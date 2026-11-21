//! `cargo xtask ci`: every blocking check that needs no GitHub API, in order (MG-REQ-0166).
//! It runs the checks that judge the tree alone and never a guard that judges a pull request
//! against its base (MG-REQ-1724).
//!
//! `just check` calls this command instead of listing the steps, so the local run and CI
//! cannot drift apart.

use std::path::Path;
use std::process::{Command, ExitCode};

use crate::failure::Failure;
use crate::{cargo, traceability, workspace};

/// The steps, in the order they run.
pub(crate) const STEPS: [&str; 18] = [
    "fmt",
    "contracts-generated",
    "clippy",
    "unsafe",
    "missing-docs",
    "test",
    "check-boundaries",
    "language",
    "size",
    "secrets",
    "sql-concatenation",
    "crate-docs",
    "license-headers",
    "governance-files",
    "release-trigger",
    "cargo-deny",
    "spec-check",
    "traceability",
];

/// Runs every step in order and stops at the first that fails.
///
/// # Errors
///
/// Fails with the failure of the first step that fails or cannot run.
pub(crate) fn run(args: &[String]) -> Result<ExitCode, Failure> {
    if !args.is_empty() {
        return Err(Failure::new("usage: cargo xtask ci"));
    }
    let root = workspace::root()?;
    for step in STEPS {
        announce(step);
        let status = run_step(&root, step)?;
        if status != ExitCode::SUCCESS {
            return Ok(status);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Runs one step and returns its exit status.
///
/// # Errors
///
/// Fails if the step fails or cannot run.
fn run_step(root: &Path, step: &str) -> Result<ExitCode, Failure> {
    match step {
        "fmt" => cargo::run(root, &["fmt", "--all", "--check"], &[]),
        "test" => cargo::run(
            root,
            &["test", "--workspace", "--all-targets", "--locked"],
            &[],
        ),
        "cargo-deny" => cargo::run(root, &["deny", "check"], &[]),
        "spec-check" => spec_check(root),
        "traceability" => return traceability::run(&[]),
        _ => crate::check(step),
    }
    .map(|()| ExitCode::SUCCESS)
}

/// Runs `python3 -I scripts/spec_tool.py check`, as `just spec-check` does.
///
/// # Errors
///
/// Fails if Python cannot start or the check fails.
fn spec_check(root: &Path) -> Result<(), Failure> {
    let status = Command::new("python3")
        .args(["-I", "scripts/spec_tool.py", "check"])
        .current_dir(root)
        .status()
        .map_err(|error| Failure::new(format!("cannot run python3: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::new(format!(
            "`spec_tool.py check` failed: {status}"
        )))
    }
}

/// Prints the name of the step that starts on standard error.
#[expect(
    clippy::print_stderr,
    reason = "MG-REQ-0166: `cargo xtask ci` shows which step runs, as `just` did"
)]
fn announce(step: &str) {
    eprintln!("ci: {step}");
}

#[cfg(test)]
mod tests {
    use super::STEPS;
    use crate::guards::{GUARD_JOBS, REQUIRED_CHECKS};
    use crate::workspace;

    /// The job names of the guard workflow: each `name:` of a job, and each guard of a
    /// matrix, whose job is named after it.
    fn guard_workflow_jobs() -> Vec<String> {
        let root = workspace::root().unwrap();
        let path = root.join(".github").join("workflows").join("guards.yml");
        let workflow = workspace::read(&path).unwrap();
        workflow
            .lines()
            .filter_map(|line| {
                line.strip_prefix("    name: ")
                    .or_else(|| line.strip_prefix("          - "))
            })
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn mg_req_1724_ci_runs_no_guard() {
        for guard in GUARD_JOBS {
            assert!(
                !STEPS.contains(&guard),
                "`cargo xtask ci` runs the guard `{guard}`"
            );
        }
    }

    #[test]
    fn mg_req_1724_checks_of_the_tree_run_in_ci_not_as_guards() {
        let jobs = guard_workflow_jobs();
        for check in ["spec-check", "language"] {
            assert!(STEPS.contains(&check), "`cargo xtask ci` lacks `{check}`");
            assert!(
                !jobs.iter().any(|job| job == check),
                "the guard workflow runs `{check}`"
            );
        }
    }

    #[test]
    fn mg_req_1724_the_guard_workflow_runs_every_guard_job() {
        let jobs = guard_workflow_jobs();
        for guard in GUARD_JOBS {
            assert!(
                jobs.iter().any(|job| job == guard),
                "the guard workflow has no job `{guard}`"
            );
        }
    }

    #[test]
    fn mg_req_1727_required_checks() {
        assert_eq!(
            REQUIRED_CHECKS,
            [
                "scope",
                "session-identity",
                "independent-review",
                "issue-format",
                "pr-format",
                "commit-format",
                "deny-rules",
                "repository-settings",
                "ci (ubuntu-latest)",
                "ci (macos-latest)",
                "ci (windows-latest)",
                "traceability",
                "benchmarks",
            ]
        );
        assert_eq!(REQUIRED_CHECKS[..GUARD_JOBS.len()], GUARD_JOBS);
    }

    #[test]
    fn mg_req_1727_setup_script_requires_the_required_checks() {
        let root = workspace::root().unwrap();
        let script = workspace::read(&root.join("scripts").join("setup_repository.sh")).unwrap();
        let required: Vec<&str> = script
            .lines()
            .skip_while(|line| !line.starts_with("readonly REQUIRED_CHECKS=("))
            .skip(1)
            .take_while(|line| *line != ")")
            .map(|line| line.trim().trim_matches('"'))
            .collect();
        assert_eq!(required, REQUIRED_CHECKS);
    }

    #[test]
    fn mg_req_1747_mg_req_1749_ci_runs_the_license_and_governance_checks() {
        for check in ["license-headers", "governance-files"] {
            assert!(STEPS.contains(&check), "`cargo xtask ci` lacks `{check}`");
            assert!(
                crate::check(check).is_ok(),
                "`{check}` fails on this repository"
            );
        }
    }

    #[test]
    fn mg_req_1715_ci_runs_the_release_trigger_check() {
        assert!(STEPS.contains(&"release-trigger"));
        assert!(crate::check("release-trigger").is_ok());
    }

    #[test]
    fn mg_req_0166_ci_runs_the_checks_then_traceability() {
        assert_eq!(STEPS.first(), Some(&"fmt"));
        assert_eq!(STEPS.last(), Some(&"traceability"));
        for step in STEPS {
            assert_eq!(STEPS.iter().filter(|other| **other == step).count(), 1);
        }
    }

    #[test]
    fn mg_req_0166_just_check_runs_xtask_ci() {
        let root = workspace::root().unwrap();
        let justfile = workspace::read(&root.join("justfile")).unwrap();
        let check: Vec<_> = justfile
            .lines()
            .skip_while(|line| !line.starts_with("check:"))
            .take_while(|line| line.starts_with("check:") || line.starts_with(' '))
            .collect();
        assert_eq!(check, ["check:", "    cargo xtask ci"]);
    }
}
