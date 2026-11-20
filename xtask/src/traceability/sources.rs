//! Find the `mg_req_<id>_` test functions in the Rust sources and run them (MG-REQ-0165).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::failure::Failure;
use crate::workspace;

/// The prefix of every test named after a rule (MG-REQ-0011).
const PREFIX: &str = "mg_req_";

/// What `cargo test` reported for a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// It ran and passed.
    Passed,
    /// It ran and failed.
    Failed,
    /// It did not run.
    Ignored,
}

/// A test function named `mg_req_<4 digits>_<what>`.
pub(crate) struct Test {
    /// The function name.
    pub(crate) name: String,
    /// The rule number its name gives.
    pub(crate) number: u16,
}

/// Lists the test functions of every workspace member, skipping `fixtures` directories,
/// sorted by name.
///
/// # Errors
///
/// Fails if a directory or a file cannot be read.
pub(crate) fn find(root: &Path) -> Result<Vec<Test>, Failure> {
    let mut tests = Vec::new();
    for member in workspace::members(root)? {
        for file in workspace::rust_files(&member.dir)? {
            let relative = file.strip_prefix(root).unwrap_or(&file);
            if relative
                .components()
                .any(|part| part.as_os_str() == "fixtures")
            {
                continue;
            }
            tests.extend(tests_in(&workspace::read(&file)?));
        }
    }
    tests.sort_by(|left, right| left.name.cmp(&right.name));
    tests.dedup_by(|left, right| left.name == right.name);
    Ok(tests)
}

/// Lists the functions `fn mg_req_<4 digits>_<what>` of a Rust source, in order.
pub(crate) fn tests_in(source: &str) -> Vec<Test> {
    let needle = format!("fn {PREFIX}");
    let mut tests = Vec::new();
    for (start, _) in source.match_indices(&needle) {
        let before = source[..start].chars().next_back();
        if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let name: String = source[start + 3..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if let Some(number) = rule_number(&name) {
            tests.push(Test { name, number });
        }
    }
    tests
}

/// The rule number of a test name `mg_req_<4 digits>_<what>`.
fn rule_number(name: &str) -> Option<u16> {
    let rest = name.strip_prefix(PREFIX)?;
    let digits = rest.get(..4)?;
    let what = rest.get(4..)?.strip_prefix('_')?;
    let valid = digits.bytes().all(|byte| byte.is_ascii_digit()) && !what.is_empty();
    valid.then(|| digits.parse().ok()).flatten()
}

/// Reads the outcome of each test from the output of `cargo test`, by function name. A
/// name that failed anywhere stays failed.
pub(crate) fn outcomes(output: &str) -> BTreeMap<String, Outcome> {
    let mut outcomes = BTreeMap::new();
    for line in output.lines() {
        let Some((path, result)) = line
            .strip_prefix("test ")
            .and_then(|rest| rest.split_once(" ... "))
        else {
            continue;
        };
        let outcome = if result.starts_with("ok") {
            Outcome::Passed
        } else if result.starts_with("FAILED") {
            Outcome::Failed
        } else if result.starts_with("ignored") {
            Outcome::Ignored
        } else {
            continue;
        };
        let name = path.rsplit("::").next().unwrap_or(path).to_owned();
        let entry = outcomes.entry(name).or_insert(outcome);
        if outcome == Outcome::Failed || *entry == Outcome::Ignored {
            *entry = outcome;
        }
    }
    outcomes
}

/// Runs the tests named after a rule with `cargo test` and returns their outcomes.
///
/// # Errors
///
/// Fails if Cargo cannot start, prints non-UTF-8 output, or fails without any failed test,
/// for example when a test does not compile.
pub(crate) fn run(root: &Path) -> Result<BTreeMap<String, Outcome>, Failure> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let args = [
        "test",
        "--workspace",
        "--all-targets",
        "--locked",
        "--no-fail-fast",
        "--",
        PREFIX,
    ];
    let output = Command::new(&cargo)
        .args(args)
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| Failure::new(format!("cannot run cargo: {error}")))?;
    let text = String::from_utf8(output.stdout)
        .map_err(|_| Failure::new("`cargo test` printed non-UTF-8"))?;
    let outcomes = outcomes(&text);
    if !output.status.success() && !outcomes.values().any(|o| *o == Outcome::Failed) {
        return Err(Failure::new(format!(
            "`cargo {}` failed: {}",
            args.join(" "),
            output.status
        )));
    }
    Ok(outcomes)
}
