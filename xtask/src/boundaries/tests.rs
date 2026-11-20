//! Every item of MG-REQ-0126 has a fixture workspace under `xtask/fixtures/boundaries/` that
//! breaks it, and the check must report exactly the expected problems for that item.

use std::path::Path;

use super::problems;
use crate::metadata;
use crate::workspace;

/// Runs the check on a fixture workspace and keeps the problems of one item.
fn problems_of(fixture: &str, item: &str) -> Vec<String> {
    let prefix = format!("{item}: ");
    all_problems(fixture)
        .into_iter()
        .filter(|problem| problem.starts_with(&prefix))
        .collect()
}

/// Runs the check on a fixture workspace.
fn all_problems(fixture: &str) -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/boundaries")
        .join(fixture);
    problems(&metadata::read(&dir).unwrap())
}

#[test]
fn mg_req_0126_valid_workspace_passes() {
    assert_eq!(all_problems("valid"), Vec::<String>::new());
}

#[test]
fn mg_req_0126_this_workspace_passes() {
    let root = workspace::root().unwrap();
    assert_eq!(
        problems(&metadata::read(&root).unwrap()),
        Vec::<String>::new()
    );
}

#[test]
fn mg_req_0110_misplaced_nested_and_unknown_members_fail() {
    assert_eq!(
        problems_of("placement", "MG-REQ-0126.1"),
        [
            "MG-REQ-0126.1: `mergus-catalog` is in `other/mergus-catalog`, not in `crates/mergus-catalog`",
            "MG-REQ-0126.1: `mergus-extra` is not in the crate table of `architecture/crates.md`",
            "MG-REQ-0126.1: `mergus-plan` is in `crates/group/mergus-plan`, not in `crates/mergus-plan`",
            "MG-REQ-0126.1: `xtask` is in `tools/xtask`, not in `xtask`",
        ]
    );
}

#[test]
fn mg_req_0120_upward_same_layer_and_cyclic_dependencies_fail() {
    assert_eq!(
        problems_of("layers", "MG-REQ-0126.2"),
        [
            "MG-REQ-0126.2: `mergus-ports` (layer 0) depends on `mergus-contracts` (layer 0)",
            "MG-REQ-0126.2: `mergus-project` (layer 1) depends on `mergus-plan` (layer 2)",
            "MG-REQ-0126.2: `mergus-quality` (layer 3) depends on `mergus-catalog` (layer 3)",
            "MG-REQ-0126.2: `mergus-sim` (layer test) depends on `mergus-cli` (layer 5)",
            "MG-REQ-0126.2: dependency cycle `mergus-ops` -> `mergus-security` -> `mergus-ops`",
        ]
    );
}

#[test]
fn mg_req_0003_mergus_dependency_outside_the_workspace_fails() {
    assert_eq!(
        problems_of("resolution", "MG-REQ-0126.3"),
        [
            "MG-REQ-0126.3: `mergus-catalog` depends on `mergus-ops`, which does not resolve to a workspace member",
            "MG-REQ-0126.3: `mergus-catalog` depends on `mergus-security`, which does not resolve to a workspace member",
        ]
    );
}

#[test]
fn mg_req_0004_core_crate_depending_on_a_non_core_member_fails() {
    assert_eq!(
        problems_of("core", "MG-REQ-0126.4"),
        [
            "MG-REQ-0126.4: core crate `mergus-plan` depends on `mergus-ops`, which is not core",
            "MG-REQ-0126.4: core crate `mergus-project` depends on `mergus-catalog`, which is not core",
        ]
    );
}

#[test]
fn mg_req_0121_real_ports_outside_the_wiring_crates_fail() {
    assert_eq!(
        problems_of("real", "MG-REQ-0126.5"),
        [
            "MG-REQ-0126.5: `mergus-ports` enables feature `real` in its default features",
            "MG-REQ-0126.5: `mergus-catalog` enables feature `real` of `mergus-ports`",
            "MG-REQ-0126.5: `mergus-quality` enables feature `real` of `mergus-ports`",
        ]
    );
}

#[test]
fn mg_req_0121_plan_reaching_an_engine_or_the_runtime_fails() {
    assert_eq!(
        problems_of("planning", "MG-REQ-0126.6"),
        [
            "MG-REQ-0126.6: `mergus-plan` depends on `mergus-engine-duckdb` (`mergus-plan` -> `mergus-engine-duckdb`)",
            "MG-REQ-0126.6: `mergus-plan` depends on `mergus-runtime` (`mergus-plan` -> `mergus-project` -> `mergus-runtime`)",
        ]
    );
}

#[test]
fn mg_req_0126_keyring_outside_security_fails() {
    assert_eq!(
        problems_of("keyring", "MG-REQ-0126.7"),
        [
            "MG-REQ-0126.7: `mergus-catalog` depends on `keyring`",
            "MG-REQ-0126.7: `xtask` depends on `keyring`",
        ]
    );
}

#[test]
fn mg_req_0124_test_crates_outside_dev_dependencies_fail() {
    assert_eq!(
        problems_of("test-crates", "MG-REQ-0126.8"),
        [
            "MG-REQ-0126.8: `mergus-cli` depends on `mergus-sim` outside `[dev-dependencies]`",
            "MG-REQ-0126.8: `mergus-runtime` depends on `mergus-testkit` outside `[dev-dependencies]`",
        ]
    );
}
