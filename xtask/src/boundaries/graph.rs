//! Graph walks over the members: cycles, what `mergus-plan` reaches, and feature closures.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::crates::UNREACHABLE_FROM_PLAN;
use super::linked;
use crate::metadata::{Kind, Package, Workspace};

/// MG-REQ-0126.2: no cycle of normal or build dependencies between members.
pub(super) fn cycles(workspace: &Workspace, problems: &mut Vec<String>) {
    let mut found = BTreeSet::new();
    let mut done = BTreeSet::new();
    for member in &workspace.members {
        let mut path = Vec::new();
        visit(workspace, member, &mut path, &mut done, &mut found);
    }
    for cycle in found {
        let names: Vec<_> = cycle.iter().map(|name| format!("`{name}`")).collect();
        problems.push(format!(
            "MG-REQ-0126.2: dependency cycle {} -> `{}`",
            names.join(" -> "),
            cycle.first().map_or("", String::as_str)
        ));
    }
}

/// Depth-first search that records every cycle it closes, rotated to start at its smallest
/// name so that each cycle is reported once.
fn visit<'a>(
    workspace: &'a Workspace,
    member: &'a Package,
    path: &mut Vec<&'a str>,
    done: &mut BTreeSet<&'a str>,
    found: &mut BTreeSet<Vec<String>>,
) {
    if let Some(start) = path.iter().position(|name| *name == member.name) {
        let mut cycle: Vec<String> = path[start..]
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        let smallest = (0..cycle.len()).min_by_key(|&at| &cycle[at]).unwrap_or(0);
        cycle.rotate_left(smallest);
        found.insert(cycle);
        return;
    }
    if done.contains(member.name.as_str()) {
        return;
    }
    path.push(&member.name);
    for dependency in member.dependencies.iter().filter(|d| linked(d)) {
        if let Some(target) = workspace.resolve(dependency) {
            visit(workspace, target, path, done, found);
        }
    }
    path.pop();
    done.insert(&member.name);
}

/// MG-REQ-0126.6: `mergus-plan` reaches no engine, source or adapter crate and not
/// `mergus-runtime`, through a direct normal or build dependency followed by normal
/// dependencies.
pub(super) fn planning(workspace: &Workspace, problems: &mut Vec<String>) {
    let Some(plan) = workspace.member("mergus-plan") else {
        return;
    };
    let mut parents: BTreeMap<&str, &str> = BTreeMap::new();
    let mut queue = VecDeque::from([plan]);
    while let Some(member) = queue.pop_front() {
        let first_hop = member.name == plan.name;
        for dependency in &member.dependencies {
            let followed = if first_hop {
                linked(dependency)
            } else {
                dependency.kind == Kind::Normal
            };
            let Some(target) = workspace.resolve(dependency).filter(|_| followed) else {
                continue;
            };
            if target.name != plan.name && !parents.contains_key(target.name.as_str()) {
                parents.insert(&target.name, &member.name);
                queue.push_back(target);
            }
        }
    }
    for (&reached, _) in parents
        .iter()
        .filter(|(name, _)| UNREACHABLE_FROM_PLAN.contains(name))
    {
        let mut chain = vec![reached];
        while let Some(&parent) = chain.last().and_then(|name| parents.get(name)) {
            chain.push(parent);
        }
        chain.reverse();
        let shown: Vec<_> = chain.iter().map(|name| format!("`{name}`")).collect();
        problems.push(format!(
            "MG-REQ-0126.6: `mergus-plan` depends on `{reached}` ({})",
            shown.join(" -> ")
        ));
    }
}

/// Whether enabling `features` of `package` enables `wanted`, following the package's own
/// `[features]` table. Without the package, only the listed names count.
pub(super) fn enables(package: Option<&Package>, features: &[String], wanted: &str) -> bool {
    let mut seen = BTreeSet::new();
    let mut pending: Vec<&str> = features.iter().map(String::as_str).collect();
    while let Some(feature) = pending.pop() {
        if feature == wanted {
            return true;
        }
        if !seen.insert(feature) {
            continue;
        }
        if let Some(enabled) = package.and_then(|package| package.features.get(feature)) {
            pending.extend(enabled.iter().map(String::as_str));
        }
    }
    false
}

/// The features of the dependency `key` that the member's own features enable, written
/// `key/feature` or `key?/feature`.
pub(super) fn features_through(member: &Package, key: &str) -> Vec<String> {
    let prefixes = [format!("{key}/"), format!("{key}?/")];
    member
        .features
        .values()
        .flatten()
        .filter_map(|value| {
            prefixes
                .iter()
                .find_map(|prefix| value.strip_prefix(prefix.as_str()))
        })
        .map(str::to_owned)
        .collect()
}
