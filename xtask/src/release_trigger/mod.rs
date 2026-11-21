//! `release-trigger`: no workflow under `.github/workflows/` grants `contents: write`,
//! `packages: write` or `id-token: write` unless only a push of tags matching `v*` triggers
//! it (MG-REQ-1715, MG-REQ-1712).
//!
//! A workflow grants a permission when its top-level `permissions` or the `permissions` of
//! one of its jobs sets it to `write`, or is `write-all`. It is triggered only by a push of
//! such tags when `on` is a mapping with the single event `push`, whose filters are `tags`
//! and optionally `paths` or `paths-ignore`, and whose every tag pattern that is not a
//! negation (`!`) starts with `v`, so that it matches only tags that `v*` matches.

mod yaml;

#[cfg(test)]
mod tests;

use std::path::Path;

use crate::failure::Failure;
use crate::workspace;
use yaml::Node;

/// The permissions that only the release workflow may grant.
const WRITES: [&str; 3] = ["contents", "packages", "id-token"];

/// The filters a release `push` may have besides `tags`.
const NARROWING_FILTERS: [&str; 2] = ["paths", "paths-ignore"];

/// Runs the check on the workflows of the repository at `root`.
///
/// # Errors
///
/// Fails, listing every finding, when a workflow grants a write permission of MG-REQ-1715 and
/// has another trigger, or when a workflow cannot be read or parsed.
pub(crate) fn check(root: &Path) -> Result<(), Failure> {
    let dir = root.join(".github").join("workflows");
    let mut problems = Vec::new();
    for path in workspace::sorted_entries(&dir)? {
        let workflow = path
            .extension()
            .is_some_and(|extension| extension == "yml" || extension == "yaml");
        if !workflow {
            continue;
        }
        let shown = workspace::show(root, &path);
        match judge(&workspace::read(&path)?) {
            Ok(None) => {}
            Ok(Some(finding)) => problems.push(format!("{shown}: {finding}")),
            Err(error) => problems.push(format!("{shown}: cannot be read as YAML: {error}")),
        }
    }
    Failure::from_problems("release-trigger", &problems)
}

/// The finding on one workflow, if any.
///
/// # Errors
///
/// Fails when the workflow is outside the YAML subset that `yaml` reads.
fn judge(workflow: &str) -> Result<Option<String>, String> {
    let root = yaml::parse(workflow)?;
    let granted = grants(&root);
    if granted.is_empty() || only_release_tags(root.get("on")) {
        return Ok(None);
    }
    let granted: Vec<String> = granted
        .iter()
        .map(|permission| format!("`{permission}: write`"))
        .collect();
    Ok(Some(format!(
        "MG-REQ-1715: grants {} and is triggered by something other than a push of tags \
         matching `v*`",
        granted.join(", ")
    )))
}

/// The permissions of [`WRITES`] that the workflow or one of its jobs grants, in that order.
fn grants(root: &Node) -> Vec<&'static str> {
    let mut blocks: Vec<&Node> = root.get("permissions").into_iter().collect();
    if let Some(Node::Map(jobs)) = root.get("jobs") {
        blocks.extend(jobs.iter().filter_map(|(_, job)| job.get("permissions")));
    }
    WRITES
        .into_iter()
        .filter(|permission| {
            blocks.iter().any(|block| match block {
                Node::Scalar(all) => all == "write-all",
                map => map.get(permission).and_then(Node::as_str) == Some("write"),
            })
        })
        .collect()
}

/// Whether `on` is only a push of tags that match `v*`.
fn only_release_tags(on: Option<&Node>) -> bool {
    let Some(Node::Map(events)) = on else {
        return false;
    };
    let [(event, Node::Map(filters))] = events.as_slice() else {
        return false;
    };
    let known = filters
        .iter()
        .all(|(filter, _)| filter == "tags" || NARROWING_FILTERS.contains(&filter.as_str()));
    let patterns: Vec<&str> = match filters.iter().find(|(filter, _)| filter == "tags") {
        Some((_, Node::Scalar(pattern))) => vec![pattern.as_str()],
        Some((_, Node::List(patterns))) => patterns.iter().filter_map(Node::as_str).collect(),
        _ => Vec::new(),
    };
    let positive: Vec<&&str> = patterns.iter().filter(|p| !p.starts_with('!')).collect();
    event == "push"
        && known
        && !positive.is_empty()
        && positive.iter().all(|pattern| pattern.starts_with('v'))
}
