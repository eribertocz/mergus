//! Tests of the `deny-rules` and `repository-settings` guards against the fixtures under
//! `xtask/fixtures/guards/<guard>/<case>/`. They never call the network.

use std::path::{Path, PathBuf};

use super::tests::{Fixture, lines};
use super::{Event, Report, deny_rules, prefixed, repository_settings};
use crate::json::Json;
use crate::workspace;

fn case_dir(guard: &str, case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/guards")
        .join(guard)
        .join(case)
}

/// Runs `deny-rules` on a fixture's `base.json` and `head.json`.
fn deny(case: &str) -> Vec<String> {
    let dir = case_dir("deny-rules", case);
    let base = workspace::read(&dir.join("base.json")).unwrap();
    let head = workspace::read(&dir.join("head.json")).unwrap();
    prefixed("deny-rules", deny_rules::judge(&base, Some(&head)).unwrap())
}

/// Runs `repository-settings` on a fixture.
fn settings(case: &str) -> Report {
    let dir = case_dir("repository-settings", case);
    let event = Event::read(&dir.join("event.json")).unwrap();
    let api = Fixture(Json::parse(&workspace::read(&dir.join("api.json")).unwrap()).unwrap());
    repository_settings::judge(&event, &api).unwrap()
}

/// The findings of `repository-settings` on a fixture, prefixed; it must have no warning.
fn settings_findings(case: &str) -> Vec<String> {
    let report = settings(case);
    assert_eq!(report.warnings, Vec::<String>::new(), "{case}");
    prefixed("repository-settings", report.findings)
}

#[test]
fn mg_req_1737_a_head_that_keeps_every_entry_passes() {
    assert_eq!(deny("passes"), Vec::<String>::new());
}

#[test]
fn mg_req_1737_the_head_lacks_entries_of_the_base() {
    let message = |entry: &str| {
        format!("MG-REQ-1737: permissions.deny at the head lacks `{entry}`, present at the base")
    };
    assert_eq!(
        deny("entry-removed"),
        lines(
            "deny-rules",
            &[&message("Bash(gh pr merge *)"), &message("Bash(git tag *)")]
        )
    );
    assert_eq!(
        deny("no-deny-list"),
        lines(
            "deny-rules",
            &[
                &message("Bash(gh pr merge *)"),
                &message("Bash(git tag *)"),
                &message("mcp__*merge*"),
            ]
        )
    );
}

#[test]
fn mg_req_1737_the_head_does_not_parse() {
    let found = deny("head-unparsable");
    assert_eq!(found.len(), 1);
    let first = found.first().unwrap();
    assert!(
        first.starts_with(
            "deny-rules: MG-REQ-1737: .claude/settings.json at the head does not parse: \
             invalid JSON"
        ),
        "{first}"
    );
    let missing = deny_rules::judge("{}", None).unwrap();
    assert_eq!(
        missing,
        ["MG-REQ-1737: .claude/settings.json at the head does not parse: the file is missing"]
    );
}

#[test]
fn mg_req_1737_a_base_that_does_not_parse_is_an_error() {
    assert!(deny_rules::judge("{", Some("{}")).is_err());
}

#[test]
fn mg_req_1738_protected_settings_pass() {
    assert_eq!(settings_findings("passes"), Vec::<String>::new());
}

#[test]
fn mg_req_1738_3_no_required_checks_only_warns() {
    let report = settings("passes-without-checks");
    assert_eq!(report.findings, Vec::<String>::new());
    assert_eq!(
        report.warnings,
        [
            "MG-REQ-1738.3: the branch ruleset has no required_status_checks rule yet; add it \
          once the guards run in CI (MG-REQ-1725)"
        ]
    );
}

#[test]
fn mg_req_1738_1_no_branch_ruleset_protects_the_default_branch() {
    assert_eq!(
        settings("branch-ruleset-missing").findings,
        [
            "MG-REQ-1738.1: no active branch ruleset targets the default branch `main` with the \
          rules deletion, non_fast_forward and pull_request and an empty bypass list"
        ]
    );
}

#[test]
fn mg_req_1738_1_the_branch_ruleset_has_a_bypass() {
    assert_eq!(
        settings_findings("branch-bypass"),
        lines(
            "repository-settings",
            &[
                "MG-REQ-1738.1: no active branch ruleset targets the default branch `main` with \
               the rules deletion, non_fast_forward and pull_request and an empty bypass list"
            ]
        )
    );
}

#[test]
fn mg_req_1738_2_no_tag_ruleset_protects_release_tags() {
    assert_eq!(
        settings_findings("tag-ruleset-missing"),
        lines(
            "repository-settings",
            &[
                "MG-REQ-1738.2: no active tag ruleset targets `refs/tags/v*` with the rules \
               creation, update and deletion and a bypass list that holds only the repository \
               admin role"
            ]
        )
    );
}

#[test]
fn mg_req_1738_2_the_tag_ruleset_bypass_holds_more_than_the_admin_role() {
    assert_eq!(
        settings_findings("tag-bypass"),
        lines(
            "repository-settings",
            &[
                "MG-REQ-1738.2: no active tag ruleset targets `refs/tags/v*` with the rules \
               creation, update and deletion and a bypass list that holds only the repository \
               admin role"
            ]
        )
    );
}

#[test]
fn mg_req_1738_3_the_required_checks_lack_a_guard() {
    assert_eq!(
        settings_findings("checks-missing"),
        lines(
            "repository-settings",
            &[
                "MG-REQ-1738.3: the required status checks lack `independent-review`",
                "MG-REQ-1738.3: the required status checks lack `spec-check`",
            ]
        )
    );
}

#[test]
fn mg_req_1738_3_a_required_check_is_not_tied_to_github_actions() {
    let message = "is required without the GitHub Actions app (integration_id 15368)";
    assert_eq!(
        settings_findings("checks-wrong-app"),
        lines(
            "repository-settings",
            &[
                &format!("MG-REQ-1738.3: `scope` {message}"),
                &format!("MG-REQ-1738.3: `commit-format` {message}"),
            ]
        )
    );
}
