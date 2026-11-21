//! The `repository-settings` guard: the rulesets protect the default branch and the release
//! tags, and require the checks of MG-REQ-1727 from GitHub Actions (MG-REQ-1738).

use super::github::{self, Api};
use super::{Event, Report};
use crate::failure::Failure;
use crate::json::Json;

/// The guards of MG-REQ-1724 that have a job in the guard workflow. `contract-compatibility`
/// joins them with its job (MG-REQ-1727). Tests check it against the guard workflow and
/// against the start of [`REQUIRED_CHECKS`].
#[cfg(test)]
pub(crate) const GUARD_JOBS: [&str; 8] = [
    "scope",
    "session-identity",
    "independent-review",
    "issue-format",
    "pr-format",
    "commit-format",
    "deny-rules",
    "repository-settings",
];

/// The checks the ruleset on the default branch requires from GitHub Actions (MG-REQ-1727):
/// every guard of MG-REQ-1724 with a job in the guard workflow, `ci` on each runner,
/// `traceability` and `benchmarks`.
pub(crate) const REQUIRED_CHECKS: [&str; 13] = [
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
];

/// The app id of GitHub Actions.
const GITHUB_ACTIONS: u64 = 15368;

/// The repository role id of the admin role.
const ADMIN_ROLE: u64 = 5;

/// The release tags.
const RELEASE_TAGS: &str = "refs/tags/v*";

/// A ref condition that targets every branch or every tag.
const ALL: &str = "~ALL";

/// A ref condition that targets the default branch.
const DEFAULT_BRANCH: &str = "~DEFAULT_BRANCH";

/// One ruleset, as `GET /repos/{repo}/rulesets/{id}` returns it.
struct Ruleset(Json);

impl Ruleset {
    fn text(&self, key: &str) -> &str {
        self.0.get(key).and_then(Json::as_str).unwrap_or_default()
    }

    /// Whether it is active and its target is `target` (`branch` or `tag`).
    fn is_active(&self, target: &str) -> bool {
        self.text("enforcement") == "active" && self.text("target") == target
    }

    /// The patterns of `conditions.ref_name.<key>` (`include` or `exclude`).
    fn refs(&self, key: &str) -> Vec<&str> {
        self.0
            .get("conditions")
            .and_then(|conditions| conditions.get("ref_name"))
            .and_then(|ref_name| ref_name.get(key))
            .map(Json::items)
            .unwrap_or_default()
            .iter()
            .filter_map(Json::as_str)
            .collect()
    }

    /// Whether its conditions include one of `names` and exclude none of them.
    fn targets(&self, names: &[&str]) -> bool {
        let hits = |key: &str| self.refs(key).iter().any(|name| names.contains(name));
        hits("include") && !hits("exclude")
    }

    fn rules(&self) -> &[Json] {
        self.0.get("rules").map(Json::items).unwrap_or_default()
    }

    /// Whether it has a rule of each type of `types`.
    fn has_rules(&self, types: &[&str]) -> bool {
        types.iter().all(|kind| {
            self.rules()
                .iter()
                .any(|rule| rule.get("type").and_then(Json::as_str) == Some(kind))
        })
    }

    fn bypass(&self) -> &[Json] {
        self.0
            .get("bypass_actors")
            .map(Json::items)
            .unwrap_or_default()
    }
}

/// Returns the findings and warnings of the `repository-settings` guard.
///
/// # Errors
///
/// Fails if the API cannot be read.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Report, Failure> {
    let repo = &event.repository;
    let default = github::text(api.get(&format!("/repos/{repo}"))?.get("default_branch"));
    let mut rulesets = Vec::new();
    for summary in github::list(api, &format!("/repos/{repo}/rulesets"))? {
        let id = summary
            .get("id")
            .and_then(Json::as_u64)
            .ok_or_else(|| Failure::new(format!("a ruleset of {repo} has no id")))?;
        rulesets.push(Ruleset(api.get(&format!("/repos/{repo}/rulesets/{id}"))?));
    }
    let head = format!("refs/heads/{default}");
    let branch: Vec<&Ruleset> = rulesets
        .iter()
        .filter(|ruleset| ruleset.is_active("branch"))
        .filter(|ruleset| ruleset.targets(&[&head, DEFAULT_BRANCH, ALL]))
        .collect();
    let mut report = Report::default();
    if !branch.iter().any(|ruleset| {
        ruleset.has_rules(&["deletion", "non_fast_forward", "pull_request"])
            && ruleset.bypass().is_empty()
    }) {
        report.findings.push(format!(
            "MG-REQ-1738.1: no active branch ruleset targets the default branch `{default}` \
             with the rules deletion, non_fast_forward and pull_request and an empty bypass \
             list"
        ));
    }
    if !rulesets.iter().any(protects_tags) {
        report.findings.push(format!(
            "MG-REQ-1738.2: no active tag ruleset targets `{RELEASE_TAGS}` with the rules \
             creation, update and deletion and a bypass list that holds only the repository \
             admin role"
        ));
    }
    required_checks(&branch, &mut report);
    Ok(report)
}

/// Whether a ruleset protects the release tags as MG-REQ-1738.2 asks.
fn protects_tags(ruleset: &Ruleset) -> bool {
    ruleset.is_active("tag")
        && ruleset.targets(&[RELEASE_TAGS, ALL])
        && ruleset.has_rules(&["creation", "update", "deletion"])
        && ruleset.bypass().iter().all(|actor| {
            actor.get("actor_type").and_then(Json::as_str) == Some("RepositoryRole")
                && actor.get("actor_id").and_then(Json::as_u64) == Some(ADMIN_ROLE)
        })
}

/// Adds the findings of MG-REQ-1738.3 on the `required_status_checks` rules of the branch
/// rulesets, or a warning when they have none.
fn required_checks(branch: &[&Ruleset], report: &mut Report) {
    let rules: Vec<&Json> = branch
        .iter()
        .flat_map(|ruleset| ruleset.rules())
        .filter(|rule| rule.get("type").and_then(Json::as_str) == Some("required_status_checks"))
        .collect();
    if rules.is_empty() {
        report.warnings.push(
            "MG-REQ-1738.3: the branch ruleset has no required_status_checks rule yet; add it \
             once the guards run in CI (MG-REQ-1725)"
                .to_owned(),
        );
        return;
    }
    let checks: Vec<&Json> = rules
        .iter()
        .flat_map(|rule| {
            rule.get("parameters")
                .and_then(|parameters| parameters.get("required_status_checks"))
                .map(Json::items)
                .unwrap_or_default()
        })
        .collect();
    for guard in REQUIRED_CHECKS {
        let listed: Vec<&&Json> = checks
            .iter()
            .filter(|check| check.get("context").and_then(Json::as_str) == Some(guard))
            .collect();
        if listed.is_empty() {
            report.findings.push(format!(
                "MG-REQ-1738.3: the required status checks lack `{guard}`"
            ));
        } else if listed
            .iter()
            .any(|check| check.get("integration_id").and_then(Json::as_u64) != Some(GITHUB_ACTIONS))
        {
            report.findings.push(format!(
                "MG-REQ-1738.3: `{guard}` is required without the GitHub Actions app \
                 (integration_id {GITHUB_ACTIONS})"
            ));
        }
    }
}
