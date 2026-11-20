//! Tests of the `independent-review`, `issue-format` and `pr-format` guards against the
//! fixtures under `xtask/fixtures/guards/<guard>/<case>/`. They never call the network.

use super::tests::{lines, run};
use super::{INDEPENDENT_REVIEW, ISSUE_FORMAT, PR_FORMAT};

#[test]
fn mg_req_1734_a_later_ready_verdict_from_another_session_passes() {
    for case in ["passes", "spec-branch-passes"] {
        assert_eq!(
            run(INDEPENDENT_REVIEW, case),
            Vec::<String>::new(),
            "{case}"
        );
    }
}

#[test]
fn mg_req_1734_no_verdict() {
    assert_eq!(
        run(INDEPENDENT_REVIEW, "no-verdict"),
        lines(
            "independent-review",
            &["MG-REQ-1734: no pull request comment has a `Verdict:` line"]
        )
    );
}

#[test]
fn mg_req_1734_the_latest_verdict_is_not_ready() {
    assert_eq!(
        run(INDEPENDENT_REVIEW, "not-ready"),
        lines(
            "independent-review",
            &["MG-REQ-1734: the latest verdict is `changes-needed`, not `ready`"]
        )
    );
}

#[test]
fn mg_req_1734_the_verdict_predates_the_head_commit() {
    assert_eq!(
        run(INDEPENDENT_REVIEW, "before-head"),
        lines(
            "independent-review",
            &[
                "MG-REQ-1734: the latest verdict (2026-01-02T12:00:00+02:00) is not after the \
               head commit 1111111 (2026-01-02T10:00:00Z)"
            ]
        )
    );
}

#[test]
fn mg_req_1734_the_verdict_comes_from_an_author_session() {
    assert_eq!(
        run(INDEPENDENT_REVIEW, "same-session"),
        lines(
            "independent-review",
            &[
                "MG-REQ-1734: the latest verdict carries no Claude-Session value that differs \
               from those of the commits: https://claude.ai/code/session_01Aaaa"
            ]
        )
    );
}

#[test]
fn mg_req_1734_the_verdict_has_no_session() {
    assert_eq!(
        run(INDEPENDENT_REVIEW, "no-session"),
        lines(
            "independent-review",
            &["MG-REQ-1734: the latest verdict has no `Claude-Session:` line"]
        )
    );
}

#[test]
fn mg_req_1735_a_complete_issue_passes() {
    for case in ["passes", "spec-branch-passes"] {
        assert_eq!(run(ISSUE_FORMAT, case), Vec::<String>::new(), "{case}");
    }
}

#[test]
fn mg_req_1735_the_issue_lacks_the_label() {
    assert_eq!(
        run(ISSUE_FORMAT, "missing-label"),
        lines(
            "issue-format",
            &["MG-REQ-1735: issue #7 lacks the label `impl`"]
        )
    );
}

#[test]
fn mg_req_1735_the_issue_lacks_a_heading() {
    assert_eq!(
        run(ISSUE_FORMAT, "missing-heading"),
        lines(
            "issue-format",
            &[
                "MG-REQ-1735: issue #7 lacks the heading `## Out of scope`",
                "MG-REQ-1735: issue #7 lacks the heading `## Parallel with`",
            ]
        )
    );
}

#[test]
fn mg_req_1735_the_issue_has_an_empty_section() {
    assert_eq!(
        run(ISSUE_FORMAT, "empty-section"),
        lines(
            "issue-format",
            &[
                "MG-REQ-1735: issue #7 has no text under `## Crate`",
                "MG-REQ-1735: issue #7 has no text under `## Depends on`",
            ]
        )
    );
}

#[test]
fn mg_req_1735_the_issue_names_no_rule() {
    assert_eq!(
        run(ISSUE_FORMAT, "no-rule-id"),
        lines(
            "issue-format",
            &["MG-REQ-1735: issue #7 names no MG-REQ- ID under `## Rules`"]
        )
    );
}

#[test]
fn mg_req_1736_a_filled_template_passes() {
    for case in ["passes", "spec-branch-passes"] {
        assert_eq!(run(PR_FORMAT, case), Vec::<String>::new(), "{case}");
    }
}

#[test]
fn mg_req_1736_the_body_lacks_a_heading() {
    assert_eq!(
        run(PR_FORMAT, "missing-heading"),
        lines(
            "pr-format",
            &[
                "MG-REQ-1736: the body lacks the heading `## Issue`",
                "MG-REQ-1736: the body lacks the heading `## Self-check`",
            ]
        )
    );
}

#[test]
fn mg_req_1736_the_body_has_an_empty_section() {
    assert_eq!(
        run(PR_FORMAT, "empty-section"),
        lines(
            "pr-format",
            &["MG-REQ-1736: the body has no text under `## Deviations raised as issues`"]
        )
    );
}

#[test]
fn mg_req_1736_the_issue_section_lacks_closes() {
    assert_eq!(
        run(PR_FORMAT, "missing-closes"),
        lines("pr-format", &["MG-REQ-1736: `## Issue` lacks `Closes #7`"])
    );
}

#[test]
fn mg_req_1736_rules_covered_lacks_a_rule_of_the_issue() {
    assert_eq!(
        run(PR_FORMAT, "missing-rule"),
        lines(
            "pr-format",
            &[
                "MG-REQ-1736: `## Rules covered` does not name MG-REQ-1738, listed under \
               `## Rules` of issue #7"
            ]
        )
    );
}
