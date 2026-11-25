//! MG-REQ-0123: only `mergus-security` resolves secret references into values.
//!
//! No other member depends on `secrecy`, the crate that holds secret values, or names the
//! prefix of the environment variables that carry them. `keyring` belongs to MG-REQ-0126.7.

use std::path::{Path, PathBuf};

use crate::failure::Failure;
use crate::metadata::Workspace;
use crate::workspace;

/// The only member that resolves secrets.
const DOOR: &str = "mergus-security";

/// The prefix of the environment variables that carry secret values, built in two parts so
/// that this file does not name it.
const VARIABLE_PREFIX: &str = concat!("MERGUS_", "SECRET_");

/// Lists every member other than [`DOOR`] that depends on `secrecy` or names
/// [`VARIABLE_PREFIX`] in a Rust file.
///
/// # Errors
///
/// Fails if a directory or a Rust file of a member cannot be read.
pub(crate) fn problems(workspace: &Workspace) -> Result<Vec<String>, Failure> {
    let mut problems = Vec::new();
    for member in workspace
        .members
        .iter()
        .filter(|member| member.name != DOOR)
    {
        if member
            .dependencies
            .iter()
            .any(|dependency| dependency.name == "secrecy")
        {
            problems.push(format!(
                "MG-REQ-0123: `{}` depends on `secrecy`",
                member.name
            ));
        }
        for file in own_rust_files(&member.dir)? {
            if workspace::read(&file)?.contains(VARIABLE_PREFIX) {
                problems.push(format!(
                    "MG-REQ-0123: `{}` names the secret variables",
                    workspace.show(&file)
                ));
            }
        }
    }
    problems.sort();
    Ok(problems)
}

/// Lists the `.rs` files of the package in `dir`, sorted by path. It skips `target`
/// directories and the directories of other packages, such as the fixture workspaces of
/// `xtask`.
///
/// # Errors
///
/// Fails if a directory cannot be read.
fn own_rust_files(dir: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut files = Vec::new();
    for path in workspace::sorted_entries(dir)? {
        if path.is_dir() {
            let target = path.file_name().is_some_and(|name| name == "target");
            if !target && !path.join("Cargo.toml").is_file() {
                files.extend(own_rust_files(&path)?);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::problems;
    use crate::metadata;
    use crate::workspace;

    #[test]
    fn mg_req_0123_only_security_resolves_secrets() {
        let root = workspace::root().unwrap();
        let workspace = metadata::read(&root).unwrap();
        assert_eq!(problems(&workspace).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn mg_req_0123_secrets_outside_security_fail() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/secrets-door");
        let workspace = metadata::read(&dir).unwrap();
        assert_eq!(
            problems(&workspace).unwrap(),
            [
                "MG-REQ-0123: `crates/mergus-cli/tests/env.rs` names the secret variables",
                "MG-REQ-0123: `mergus-catalog` depends on `secrecy`",
                "MG-REQ-0123: `xtask` depends on `secrecy`",
            ]
        );
    }
}
