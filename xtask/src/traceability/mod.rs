//! The traceability report, `cargo xtask traceability [--issue-rules <ID>...]`
//! (MG-REQ-0165, MG-REQ-0167).
//!
//! It reads the rules from `spec/` and the `mg_req_<id>_` test functions from the Rust
//! sources, runs those tests with `cargo test`, and prints one line per item as
//! `traceability: <message>`:
//!
//! - each behavior rule that needs a test and has none, up to the current phase;
//! - each test whose `mg_req_<id>` prefix names no rule;
//! - each test that fails, with its rule;
//! - with `--issue-rules`, each listed rule that needs a test and has no passing one.
//!
//! It exits with status 1 when any of the last three lists is not empty. The first list
//! does not change the status.

mod sources;
mod spec;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use crate::failure::Failure;
use crate::workspace;
use sources::{Outcome, Test};
use spec::{Phase, Rule};

/// The name of the command and the prefix of every line it prints.
const NAME: &str = "traceability";

/// The usage of the command.
const USAGE: &str = "usage: cargo xtask traceability [--issue-rules <MG-REQ-ID>...]";

/// What the report found.
pub(crate) struct Report {
    /// The rules without a test; they do not fail the report.
    pub(crate) warnings: Vec<String>,
    /// The tests naming no rule, the failing tests and the rules of the issue without a
    /// passing test; any of them fails the report.
    pub(crate) findings: Vec<String>,
}

/// Runs the report with the command-line arguments that follow its name.
///
/// # Errors
///
/// Fails if the arguments are wrong, the specification or the sources cannot be read, or
/// the tests cannot be run.
pub(crate) fn run(args: &[String]) -> Result<ExitCode, Failure> {
    let issue_rules = issue_rules(args)?;
    let root = workspace::root()?;
    let spec = root.join("spec");
    let rules = spec::rules(&spec)?;
    let phase = spec::current_phase(&spec)?;
    let tests = sources::find(&root)?;
    let outcomes = sources::run(&root)?;
    let report = evaluate(&rules, phase, &tests, &outcomes, &issue_rules)?;
    report
        .warnings
        .iter()
        .chain(&report.findings)
        .for_each(|line| print_line(&format!("{NAME}: {line}")));
    Ok(if report.findings.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// Reads the rule numbers given with `--issue-rules`: IDs `MG-REQ-<4 digits>`, separated by
/// commas or given as separate arguments.
///
/// # Errors
///
/// Fails on any other argument.
pub(crate) fn issue_rules(args: &[String]) -> Result<Vec<u16>, Failure> {
    let Some((flag, ids)) = args.split_first() else {
        return Ok(Vec::new());
    };
    if flag != "--issue-rules" || ids.is_empty() {
        return Err(Failure::new(USAGE));
    }
    ids.iter()
        .flat_map(|arg| arg.split(','))
        .filter(|id| !id.trim().is_empty())
        .map(|id| {
            let digits = id.trim().strip_prefix("MG-REQ-").unwrap_or_default();
            let valid = digits.len() == 4 && digits.bytes().all(|byte| byte.is_ascii_digit());
            valid
                .then(|| digits.parse().ok())
                .flatten()
                .ok_or_else(|| Failure::new(format!("`{id}` is not a rule ID; {USAGE}")))
        })
        .collect()
}

/// Builds the report from the rules, the current phase, the test functions, their
/// outcomes and the rules of the issue.
///
/// # Errors
///
/// Fails if a rule of the issue does not exist.
pub(crate) fn evaluate(
    rules: &[Rule],
    phase: Phase,
    tests: &[Test],
    outcomes: &BTreeMap<String, Outcome>,
    issue_rules: &[u16],
) -> Result<Report, Failure> {
    let mut by_rule: BTreeMap<u16, Vec<&Test>> = BTreeMap::new();
    for test in tests {
        by_rule.entry(test.number).or_default().push(test);
    }
    let known: BTreeSet<u16> = rules.iter().map(|rule| rule.number).collect();
    let mut sorted: Vec<&Rule> = rules.iter().collect();
    sorted.sort_by_key(|rule| rule.number);
    let warnings = sorted
        .iter()
        .filter(|rule| rule.needs_test() && rule.phase <= phase)
        .filter(|rule| !by_rule.contains_key(&rule.number))
        .map(|rule| {
            format!(
                "{} needs a test `mg_req_{:04}_...` and has none",
                rule.id(),
                rule.number
            )
        })
        .collect();
    let mut findings: Vec<String> = tests
        .iter()
        .filter(|test| !known.contains(&test.number))
        .map(|test| {
            format!(
                "test `{}` names no rule: MG-REQ-{:04} does not exist",
                test.name, test.number
            )
        })
        .collect();
    findings.extend(failing(&by_rule, &known, outcomes));
    findings.extend(issue_findings(&sorted, &by_rule, outcomes, issue_rules)?);
    Ok(Report { warnings, findings })
}

/// The tests of existing rules that failed, by rule.
fn failing(
    by_rule: &BTreeMap<u16, Vec<&Test>>,
    known: &BTreeSet<u16>,
    outcomes: &BTreeMap<String, Outcome>,
) -> Vec<String> {
    by_rule
        .iter()
        .filter(|(number, _)| known.contains(number))
        .flat_map(|(number, tests)| tests.iter().map(move |test| (number, test)))
        .filter(|(_, test)| outcomes.get(&test.name) == Some(&Outcome::Failed))
        .map(|(number, test)| format!("MG-REQ-{number:04}: test `{}` fails", test.name))
        .collect()
}

/// The rules of the issue that need a test and have no passing one.
///
/// # Errors
///
/// Fails if a rule of the issue does not exist.
fn issue_findings(
    rules: &[&Rule],
    by_rule: &BTreeMap<u16, Vec<&Test>>,
    outcomes: &BTreeMap<String, Outcome>,
    issue_rules: &[u16],
) -> Result<Vec<String>, Failure> {
    let mut findings = Vec::new();
    for number in issue_rules.iter().collect::<BTreeSet<_>>() {
        let rule = rules
            .iter()
            .find(|rule| rule.number == *number)
            .ok_or_else(|| Failure::new(format!("MG-REQ-{number:04} is not a rule")))?;
        let passes = by_rule.get(number).is_some_and(|tests| {
            tests
                .iter()
                .any(|test| outcomes.get(&test.name) == Some(&Outcome::Passed))
        });
        if rule.needs_test() && !passes {
            findings.push(format!(
                "{}: a behavior rule of the issue has no passing test",
                rule.id()
            ));
        }
    }
    Ok(findings)
}

/// Prints one line of the report on standard output.
#[expect(
    clippy::print_stdout,
    reason = "MG-REQ-0165: `cargo xtask traceability` lists what it found"
)]
fn print_line(line: &str) {
    println!("{line}");
}
