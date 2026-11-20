//! MG-REQ-0126: the crate boundaries, read from `cargo metadata` only.
//!
//! Each item of MG-REQ-0126 has one function. Every problem starts with the item it breaks,
//! such as `MG-REQ-0126.2: `. A rule that names a crate that is not a member yet is skipped
//! for that crate.

pub(crate) mod crates;
mod graph;

use std::collections::BTreeSet;
use std::path::Path;

use crate::failure::Failure;
use crate::metadata::{self, Dependency, Kind, Package, Workspace};
use crates::{Layer, REAL_PORTS_USERS, SHARED_LAYER_3, TEST_CRATES};

/// Checks the boundaries of the workspace that contains `dir`.
///
/// # Errors
///
/// Fails if `cargo metadata` fails, or with every violation of MG-REQ-0126.
pub(crate) fn check(dir: &Path) -> Result<(), Failure> {
    Failure::from_problems("boundaries", &problems(&metadata::read(dir)?))
}

/// Lists every violation of MG-REQ-0126, item by item.
pub(crate) fn problems(workspace: &Workspace) -> Vec<String> {
    let mut problems = Vec::new();
    placement(workspace, &mut problems);
    layers(workspace, &mut problems);
    graph::cycles(workspace, &mut problems);
    resolution(workspace, &mut problems);
    core(workspace, &mut problems);
    real_ports(workspace, &mut problems);
    graph::planning(workspace, &mut problems);
    keyring(workspace, &mut problems);
    test_crates(workspace, &mut problems);
    problems
}

/// Whether a dependency is linked into the crate: a normal or build dependency.
fn linked(dependency: &Dependency) -> bool {
    dependency.kind != Kind::Dev
}

/// The members that a member links through normal or build dependencies.
fn linked_members<'a>(
    workspace: &'a Workspace,
    member: &'a Package,
) -> impl Iterator<Item = &'a Package> + 'a {
    member
        .dependencies
        .iter()
        .filter(|dependency| linked(dependency))
        .filter_map(|dependency| workspace.resolve(dependency))
}

/// MG-REQ-0126.1: every member is in `crates/<its name>/` and in the crate table, except
/// `xtask`, which is in `xtask/`.
fn placement(workspace: &Workspace, problems: &mut Vec<String>) {
    for member in &workspace.members {
        let expected = if member.name == "xtask" {
            workspace.root.join("xtask")
        } else {
            workspace.root.join("crates").join(&member.name)
        };
        if member.dir != expected {
            problems.push(format!(
                "MG-REQ-0126.1: `{}` is in `{}`, not in `{}`",
                member.name,
                workspace.show(&member.dir),
                workspace.show(&expected)
            ));
        }
        if member.name != "xtask" && crates::find(&member.name).is_none() {
            problems.push(format!(
                "MG-REQ-0126.1: `{}` is not in the crate table of `architecture/crates.md`",
                member.name
            ));
        }
    }
}

/// MG-REQ-0126.2: linked members are of a strictly lower layer, except the shared layer-3
/// crates; the test crates may depend on layers 0 to 4. Dependencies on the test crates
/// belong to MG-REQ-0126.8.
fn layers(workspace: &Workspace, problems: &mut Vec<String>) {
    for member in &workspace.members {
        let Some(from) = crates::find(&member.name) else {
            continue;
        };
        for target in linked_members(workspace, member) {
            let Some(to) = crates::find(&target.name) else {
                continue;
            };
            if to.layer != Layer::Test && !layer_allows(from, to) {
                problems.push(format!(
                    "MG-REQ-0126.2: `{}` (layer {}) depends on `{}` (layer {})",
                    from.name, from.layer, to.name, to.layer
                ));
            }
        }
    }
}

fn layer_allows(from: &crates::Crate, to: &crates::Crate) -> bool {
    let Layer::Number(to_layer) = to.layer else {
        return false;
    };
    match from.layer {
        Layer::Test => to_layer <= 4,
        Layer::Number(from_layer) => {
            to_layer < from_layer
                || (from_layer == 3
                    && to_layer == 3
                    && from.name != to.name
                    && SHARED_LAYER_3.contains(&to.name))
        }
    }
}

/// MG-REQ-0126.3: every dependency on a crate named `mergus-*` resolves to a member.
fn resolution(workspace: &Workspace, problems: &mut Vec<String>) {
    for member in &workspace.members {
        let mut reported = BTreeSet::new();
        for dependency in &member.dependencies {
            if dependency.name.starts_with("mergus-")
                && workspace.resolve(dependency).is_none()
                && reported.insert(&dependency.name)
            {
                problems.push(format!(
                    "MG-REQ-0126.3: `{}` depends on `{}`, which does not resolve to a workspace member",
                    member.name, dependency.name
                ));
            }
        }
    }
}

/// MG-REQ-0126.4: core crates link only core members and `mergus-ports`.
fn core(workspace: &Workspace, problems: &mut Vec<String>) {
    for member in &workspace.members {
        if !crates::find(&member.name).is_some_and(|row| row.core) {
            continue;
        }
        for target in linked_members(workspace, member) {
            let target_is_core = crates::find(&target.name).is_some_and(|row| row.core);
            if !target_is_core && target.name != "mergus-ports" {
                problems.push(format!(
                    "MG-REQ-0126.4: core crate `{}` depends on `{}`, which is not core",
                    member.name, target.name
                ));
            }
        }
    }
}

/// MG-REQ-0126.5: only the wiring crates enable the feature `real` of `mergus-ports`, and
/// `mergus-ports` does not enable it by default. A feature that enables another feature
/// counts as enabling it too.
fn real_ports(workspace: &Workspace, problems: &mut Vec<String>) {
    let ports = workspace.member("mergus-ports");
    let enables_real = |features: &[String]| graph::enables(ports, features, "real");
    if ports.is_some() && enables_real(&["default".to_owned()]) {
        problems.push(
            "MG-REQ-0126.5: `mergus-ports` enables feature `real` in its default features"
                .to_owned(),
        );
    }
    for member in &workspace.members {
        if REAL_PORTS_USERS.contains(&member.name.as_str()) {
            continue;
        }
        let enabled = member
            .dependencies
            .iter()
            .filter(|dependency| linked(dependency) && dependency.name == "mergus-ports")
            .any(|dependency| {
                enables_real(&dependency.features)
                    || enables_real(&graph::features_through(member, &dependency.key))
            });
        if enabled {
            problems.push(format!(
                "MG-REQ-0126.5: `{}` enables feature `real` of `mergus-ports`",
                member.name
            ));
        }
    }
}

/// MG-REQ-0126.7: only `mergus-security` depends on `keyring`.
fn keyring(workspace: &Workspace, problems: &mut Vec<String>) {
    for member in &workspace.members {
        let depends = member
            .dependencies
            .iter()
            .any(|dependency| dependency.name == "keyring");
        if depends && member.name != "mergus-security" {
            problems.push(format!(
                "MG-REQ-0126.7: `{}` depends on `keyring`",
                member.name
            ));
        }
    }
}

/// MG-REQ-0126.8: the test crates are only dev-dependencies.
fn test_crates(workspace: &Workspace, problems: &mut Vec<String>) {
    for member in &workspace.members {
        let mut reported = BTreeSet::new();
        for dependency in &member.dependencies {
            if linked(dependency)
                && TEST_CRATES.contains(&dependency.name.as_str())
                && reported.insert(&dependency.name)
            {
                problems.push(format!(
                    "MG-REQ-0126.8: `{}` depends on `{}` outside `[dev-dependencies]`",
                    member.name, dependency.name
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests;
