//! Tests of the traceability report, on the fixture under `xtask/fixtures/traceability/`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::sources::{self, Outcome, Test};
use super::spec::{self, Phase, Rule};
use super::{Report, evaluate};
use crate::workspace;

/// The directory of the traceability fixture.
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/traceability")
}

fn fixture_rules() -> Vec<Rule> {
    spec::rules(&fixture().join("spec")).unwrap()
}

fn fixture_tests() -> Vec<Test> {
    sources::tests_in(&workspace::read(&fixture().join("src/lib.rs")).unwrap())
}

fn fixture_outcomes() -> BTreeMap<String, Outcome> {
    sources::outcomes(&workspace::read(&fixture().join("cargo-test.txt")).unwrap())
}

/// Evaluates the fixture with the listed rules of an issue.
fn fixture_report(issue_rules: &[u16]) -> Report {
    let phase = spec::current_phase(&fixture().join("spec")).unwrap();
    evaluate(
        &fixture_rules(),
        phase,
        &fixture_tests(),
        &fixture_outcomes(),
        issue_rules,
    )
    .unwrap()
}

#[test]
fn mg_req_0165_rules_are_read_like_the_specification_tool() {
    let ids: Vec<_> = fixture_rules().iter().map(Rule::id).collect();
    assert_eq!(
        ids,
        [
            "MG-REQ-9004",
            "MG-REQ-9005",
            "MG-REQ-9006",
            "MG-REQ-9001",
            "MG-REQ-9002",
            "MG-REQ-9003",
        ]
    );
    let phases: Vec<_> = fixture_rules().iter().map(|rule| rule.phase).collect();
    let phase = |text| Phase::parse(text).unwrap();
    assert_eq!(
        phases,
        [
            phase("0.2"),
            phase("0.2"),
            phase("0.2"),
            phase("0.1"),
            phase("0.1"),
            phase("0.3"),
        ]
    );
}

#[test]
fn mg_req_0165_only_rules_md_and_crates_md_are_generated() {
    let spec = workspace::tests::scratch("traceability-generated");
    for (number, name) in [(9101, "RULES.md"), (9102, "crates.md"), (9103, "status.md")] {
        let text = format!(
            "---\nphase: \"0.1\"\n---\n\n### MG-REQ-{number} · Rule in {name}\n\n- Kind: process\n"
        );
        fs::write(spec.join(name), text).unwrap();
    }
    let ids: Vec<_> = spec::rules(&spec).unwrap().iter().map(Rule::id).collect();
    assert_eq!(ids, ["MG-REQ-9103"]);
}

#[test]
fn mg_req_0165_test_functions_are_found_in_sources() {
    let names: Vec<_> = fixture_tests().into_iter().map(|test| test.name).collect();
    assert_eq!(
        names,
        [
            "mg_req_9001_passes",
            "mg_req_9005_fails",
            "mg_req_9005_passes",
            "mg_req_9009_names_no_rule",
        ]
    );
}

#[test]
fn mg_req_0165_test_outcomes_are_read_from_cargo_test() {
    let outcomes = fixture_outcomes();
    assert_eq!(outcomes.get("mg_req_9001_passes"), Some(&Outcome::Passed));
    assert_eq!(outcomes.get("mg_req_9005_fails"), Some(&Outcome::Failed));
    assert_eq!(
        outcomes.get("mg_req_9009_names_no_rule"),
        Some(&Outcome::Ignored)
    );
    assert_eq!(outcomes.len(), 4);
}

#[test]
fn mg_req_0165_the_three_lists_are_reported() {
    let report = fixture_report(&[]);
    assert_eq!(
        report.warnings,
        ["MG-REQ-9002 needs a test `mg_req_9002_...` and has none"]
    );
    assert_eq!(
        report.findings,
        [
            "test `mg_req_9009_names_no_rule` names no rule: MG-REQ-9009 does not exist",
            "MG-REQ-9005: test `mg_req_9005_fails` fails",
        ]
    );
}

#[test]
fn mg_req_0165_issue_rule_without_a_passing_test_fails() {
    let report = fixture_report(&[9001, 9002, 9003, 9004, 9006]);
    assert_eq!(
        &report.findings[2..],
        [
            "MG-REQ-9002: a behavior rule of the issue has no passing test",
            "MG-REQ-9003: a behavior rule of the issue has no passing test",
        ]
    );
}

#[test]
fn mg_req_0165_unknown_issue_rule_is_an_error() {
    let phase = Phase::parse("0.2").unwrap();
    let result = evaluate(&fixture_rules(), phase, &[], &BTreeMap::new(), &[9999]);
    assert!(result.is_err());
}

#[test]
fn mg_req_0165_issue_rules_are_parsed() {
    let args = ["--issue-rules", "MG-REQ-0165,MG-REQ-0166", "MG-REQ-0167"].map(String::from);
    assert_eq!(super::issue_rules(&args).unwrap(), [165, 166, 167]);
    assert!(super::issue_rules(&[]).unwrap().is_empty());
    assert!(super::issue_rules(&["--issue-rules".to_owned(), "0165".to_owned()]).is_err());
    assert!(super::issue_rules(&["--other".to_owned()]).is_err());
}

#[test]
fn mg_req_0165_a_clean_repository_passes() {
    let rules = fixture_rules();
    let tests = fixture_tests();
    let mut outcomes = fixture_outcomes();
    outcomes.insert("mg_req_9005_fails".to_owned(), Outcome::Passed);
    let tests: Vec<_> = tests
        .into_iter()
        .filter(|test| test.number != 9009)
        .collect();
    let phase = Phase::parse("0.1").unwrap();
    let report = evaluate(&rules, phase, &tests, &outcomes, &[9001, 9005]).unwrap();
    assert!(report.findings.is_empty());
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn mg_req_0167_only_behavior_rules_verified_by_a_test_need_one() {
    let needing: Vec<_> = fixture_rules()
        .iter()
        .filter(|rule| rule.needs_test())
        .map(Rule::id)
        .collect();
    assert_eq!(
        needing,
        ["MG-REQ-9005", "MG-REQ-9001", "MG-REQ-9002", "MG-REQ-9003"]
    );
}

#[test]
fn mg_req_0167_current_phase_is_read_from_the_roadmap() {
    let phase = spec::current_phase(&fixture().join("spec")).unwrap();
    assert_eq!(phase, Phase::parse("0.2").unwrap());
    assert!(spec::current_phase(&fixture().join("spec/area")).is_err());
}

#[test]
fn mg_req_0167_rules_of_a_later_phase_are_not_listed() {
    let report = fixture_report(&[]);
    assert!(!report.warnings.iter().any(|line| line.contains("9003")));
    let phase = Phase::parse("0.3").unwrap();
    let report = evaluate(
        &fixture_rules(),
        phase,
        &fixture_tests(),
        &fixture_outcomes(),
        &[],
    )
    .unwrap();
    assert_eq!(report.warnings.len(), 2);
}

#[test]
fn mg_req_0167_phases_compare_by_number() {
    let phase = |text| Phase::parse(text).unwrap();
    assert!(phase("0.9") < phase("0.10"));
    assert!(phase("0.10") < phase("1.0"));
    assert!(Phase::parse("0").is_none());
    assert!(Phase::parse("a.1").is_none());
}

#[test]
fn mg_req_0167_this_repository_has_a_current_phase() {
    let root = workspace::root().unwrap();
    assert!(spec::current_phase(&root.join("spec")).is_ok());
    assert!(!spec::rules(&root.join("spec")).unwrap().is_empty());
}
