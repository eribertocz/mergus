//! Every member inherits `[workspace.lints]`, so the lints of the specification apply to it.

use crate::failure::Failure;
use crate::workspace::{self, Member};

/// Checks that every member's `Cargo.toml` has `[lints]` with `workspace = true`.
///
/// # Errors
///
/// Fails with the list of members that do not inherit the workspace lints.
pub(crate) fn check(check: &str, members: &[Member]) -> Result<(), Failure> {
    let mut problems = Vec::new();
    for member in members {
        let manifest = workspace::read(&member.dir.join("Cargo.toml"))?;
        if !inherits_workspace_lints(&manifest) {
            problems.push(format!(
                "{}: Cargo.toml must contain `[lints]` with `workspace = true`",
                member.name
            ));
        }
    }
    Failure::from_problems(check, &problems)
}

/// Returns whether a manifest has a `[lints]` table whose only key is `workspace = true`.
fn inherits_workspace_lints(manifest: &str) -> bool {
    let mut in_lints = false;
    let mut inherits = false;
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') {
            in_lints = line == "[lints]";
        } else if in_lints && !line.is_empty() {
            let compact: String = line.split_whitespace().collect();
            if compact != "workspace=true" {
                return false;
            }
            inherits = true;
        }
    }
    inherits
}

#[cfg(test)]
mod tests {
    use super::inherits_workspace_lints;

    #[test]
    fn mg_req_0009_manifest_inheriting_workspace_lints_passes() {
        let manifest = "[package]\nname = \"a\"\n\n[lints]\nworkspace = true # shared\n";
        assert!(inherits_workspace_lints(manifest));
    }

    #[test]
    fn mg_req_0009_manifest_without_lints_is_reported() {
        assert!(!inherits_workspace_lints("[package]\nname = \"a\"\n"));
    }

    #[test]
    fn mg_req_0009_manifest_with_local_lints_is_reported() {
        let manifest = "[lints]\nworkspace = false\n";
        assert!(!inherits_workspace_lints(manifest));
        let manifest = "[lints.clippy]\nunwrap_used = \"allow\"\n";
        assert!(!inherits_workspace_lints(manifest));
    }
}
