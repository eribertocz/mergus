//! Tests of the guards against event files and API responses kept under
//! `xtask/fixtures/guards/<guard>/<case>/`. They never call the network.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::github::{self, Api};
use super::{
    COMMIT_FORMAT, Event, Guard, SCOPE, SESSION_IDENTITY, event_path, exit_code, findings,
};
use crate::failure::Failure;
use crate::json::Json;
use crate::workspace;

/// API responses read from a fixture's `api.json`: an object from request path, without its
/// query, to response. A request for any other path fails, so a guard cannot reach an
/// endpoint the fixture does not expect.
pub(super) struct Fixture(pub(super) Json);

impl Api for Fixture {
    fn get(&self, path: &str) -> Result<Json, Failure> {
        let key = path.split('?').next().unwrap_or(path);
        self.0
            .get(key)
            .cloned()
            .ok_or_else(|| Failure::new(format!("no fixture for {key}")))
    }
}

pub(super) fn case_dir(guard: Guard, case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/guards")
        .join(guard.name)
        .join(case)
}

/// Runs a guard on a fixture and returns its printed lines.
pub(super) fn run(guard: Guard, case: &str) -> Vec<String> {
    let dir = case_dir(guard, case);
    let event = Event::read(&dir.join("event.json")).unwrap();
    let api = Fixture(Json::parse(&workspace::read(&dir.join("api.json")).unwrap()).unwrap());
    findings(guard, &event, &api).unwrap()
}

pub(super) fn lines(prefix: &str, messages: &[&str]) -> Vec<String> {
    messages
        .iter()
        .map(|message| format!("{prefix}: {message}"))
        .collect()
}

#[test]
fn mg_req_1730_the_event_gives_the_pull_request() {
    let event = Event::read(&case_dir(SCOPE, "issue-passes").join("event.json")).unwrap();
    assert_eq!(event.repository, "owner/repo");
    assert_eq!(event.number, 5);
    assert_eq!(event.branch, "issue-7-guards");
    let missing = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    assert!(Event::read(&missing).is_err());
}

#[test]
fn mg_req_1730_the_event_path_comes_from_the_flag() {
    let args = ["--event".to_owned(), "a.json".to_owned()];
    assert_eq!(event_path(&args).unwrap(), PathBuf::from("a.json"));
    assert!(event_path(&["--other".to_owned()]).is_err());
}

#[test]
fn mg_req_1730_findings_are_prefixed_and_set_the_status() {
    let found = run(SCOPE, "branch-name");
    assert!(found.iter().all(|line| line.starts_with("scope: ")));
    assert_eq!(exit_code(&found), ExitCode::from(1));
    assert_eq!(exit_code(&[]), ExitCode::SUCCESS);
}

#[test]
fn mg_req_1730_lists_are_read_page_by_page() {
    let page: Vec<String> = (0..100).map(|n| n.to_string()).collect();
    let full = Json::parse(&format!("[{}]", page.join(","))).unwrap();
    struct Pages(Json);
    impl Api for Pages {
        fn get(&self, path: &str) -> Result<Json, Failure> {
            assert!(path.starts_with("/x?state=all&per_page=100&page="));
            Ok(if path.ends_with("page=1") || path.ends_with("page=2") {
                self.0.clone()
            } else {
                Json::parse("[1]").unwrap()
            })
        }
    }
    assert_eq!(
        github::list(&Pages(full), "/x?state=all").unwrap().len(),
        201
    );
}

#[test]
fn mg_req_1731_valid_pull_requests_pass() {
    for case in [
        "issue-passes",
        "issue-owner-authorized",
        "spec-passes",
        "release-passes",
    ] {
        assert_eq!(run(SCOPE, case), Vec::<String>::new(), "{case}");
    }
}

#[test]
fn mg_req_1731_1_branch_name() {
    assert_eq!(
        run(SCOPE, "branch-name"),
        lines(
            "scope",
            &[
                "MG-REQ-1731.1: branch `feature/guards` is not named issue-<n>-<words>, \
               spec/<words> or release/<major>.<minor>.<patch>"
            ]
        )
    );
}

#[test]
fn mg_req_1731_2_file_not_in_the_issue() {
    assert_eq!(
        run(SCOPE, "issue-file-not-listed"),
        lines(
            "scope",
            &[
                "MG-REQ-1731.2: `crates/x/src/lib.rs` is not in the Files section of issue #7",
                "MG-REQ-1731.2: `justfile.bak` is not in the Files section of issue #7",
            ]
        )
    );
}

#[test]
fn mg_req_1731_2_specification_scope_on_an_issue_branch() {
    assert_eq!(
        run(SCOPE, "issue-spec-scope"),
        lines(
            "scope",
            &[
                "MG-REQ-1731.2: `spec/kernel.md` is in the specification scope",
                "MG-REQ-1731.2: `AGENTS.md` is in the specification scope",
                "MG-REQ-1731.2: `.claude/skills/x.md` is in the specification scope",
                "MG-REQ-1731.2: `scripts/spec_tool.py` is in the specification scope",
            ]
        )
    );
}

#[test]
fn mg_req_1731_2_owner_file_without_authorization() {
    let message = "is an owner file and issue #7 lacks the label owner-authorized";
    assert_eq!(
        run(SCOPE, "issue-owner-file"),
        lines(
            "scope",
            &[
                &format!("MG-REQ-1731.2: `deny.toml` {message}"),
                &format!("MG-REQ-1731.2: `.github/workflows/ci.yml` {message}"),
                &format!("MG-REQ-1731.2: `.github/CODEOWNERS` {message}"),
            ]
        )
    );
}

#[test]
fn mg_req_1731_3_spec_branch_outside_the_specification_scope() {
    assert_eq!(
        run(SCOPE, "spec-outside"),
        lines(
            "scope",
            &[
                "MG-REQ-1731.3: `xtask/src/main.rs` is outside the specification scope",
                "MG-REQ-1731.3: `scripts/other.py` is outside the specification scope",
            ]
        )
    );
}

#[test]
fn mg_req_1731_4_release_branch_outside_its_files() {
    let message = "is not a Cargo.toml, a pyproject.toml or CHANGELOG.md";
    assert_eq!(
        run(SCOPE, "release-outside"),
        lines(
            "scope",
            &[
                &format!("MG-REQ-1731.4: `Cargo.lock` {message}"),
                &format!("MG-REQ-1731.4: `docs/CHANGELOG.md` {message}"),
            ]
        )
    );
}

#[test]
fn mg_req_1731_5_lint_attribute_without_a_reason() {
    let message = "adds an allow or expect attribute without a `// reason: <text>` comment";
    assert_eq!(
        run(SCOPE, "allow-without-reason"),
        lines(
            "scope",
            &[
                &format!("MG-REQ-1731.5: spec/a.rs:2 {message}"),
                &format!("MG-REQ-1731.5: spec/a.rs:4 {message}"),
                &format!("MG-REQ-1731.5: spec/a.rs:5 {message}"),
                &format!("MG-REQ-1731.5: spec/a.rs:26 {message}"),
            ]
        )
    );
}

#[test]
fn mg_req_1731_5_ignored_test() {
    assert_eq!(
        run(SCOPE, "ignore-added"),
        lines(
            "scope",
            &[
                "MG-REQ-1731.5: spec/a.rs:12 adds an ignored test",
                "MG-REQ-1731.5: spec/a.rs:13 adds an ignored test",
            ]
        )
    );
}

#[test]
fn mg_req_1732_valid_commits_pass() {
    for case in ["passes", "spec-branch-passes"] {
        assert_eq!(run(COMMIT_FORMAT, case), Vec::<String>::new(), "{case}");
    }
}

#[test]
fn mg_req_1732_1_subject_format() {
    assert_eq!(
        run(COMMIT_FORMAT, "bad-subject"),
        lines(
            "commit-format",
            &["MG-REQ-1732.1: commit aaaaaaa: the subject is not <type>(<scope>)!: <text>"]
        )
    );
}

#[test]
fn mg_req_1732_2_subject_ends_with_the_issue() {
    let message = "the subject does not end with (#7)";
    assert_eq!(
        run(COMMIT_FORMAT, "missing-issue"),
        lines(
            "commit-format",
            &[
                &format!("MG-REQ-1732.2: commit aaaaaaa: {message}"),
                &format!("MG-REQ-1732.2: commit bbbbbbb: {message}"),
            ]
        )
    );
}

#[test]
fn mg_req_1732_3_trailers() {
    let co_author = "lacks a Co-Authored-By: trailer";
    let session = "lacks a Claude-Session: trailer with a valid value";
    assert_eq!(
        run(COMMIT_FORMAT, "missing-trailers"),
        lines(
            "commit-format",
            &[
                &format!("MG-REQ-1732.3: commit aaaaaaa: {co_author}"),
                &format!("MG-REQ-1732.3: commit bbbbbbb: {session}"),
                &format!("MG-REQ-1732.3: commit ccccccc: {co_author}"),
                &format!("MG-REQ-1732.3: commit ccccccc: {session}"),
            ]
        )
    );
}

#[test]
fn mg_req_1733_one_new_session_passes() {
    for case in ["passes", "spec-branch-passes"] {
        assert_eq!(run(SESSION_IDENTITY, case), Vec::<String>::new(), "{case}");
    }
}

#[test]
fn mg_req_1733_more_than_one_session() {
    assert_eq!(
        run(SESSION_IDENTITY, "two-sessions"),
        lines(
            "session-identity",
            &["MG-REQ-1733: the commits carry 2 Claude-Session values: \
               https://claude.ai/code/session_01Aaaa, https://claude.ai/code/session_01Bbbb"]
        )
    );
}

#[test]
fn mg_req_1733_session_of_another_implementation_pull_request() {
    assert_eq!(
        run(SESSION_IDENTITY, "reused-session"),
        lines(
            "session-identity",
            &[
                "MG-REQ-1733: Claude-Session https://claude.ai/code/session_01Aaaa also appears \
               in implementation pull request #3"
            ]
        )
    );
}
