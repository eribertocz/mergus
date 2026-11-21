//! The layout of a schema directory, as `schemas/README.md` describes it:
//! `<contract>/v<version>/schema.json` and `<contract>/v<version>/VERSION`.

use std::path::{Path, PathBuf};

use super::model::pascal;
use super::version::{self, Semver};
use crate::failure::Failure;
use crate::workspace;

/// One version of one contract.
pub(crate) struct Version {
    /// The contract name: its directory name, in snake_case.
    pub(crate) contract: String,
    /// The number of its `v<version>` directory.
    pub(crate) version: u32,
    /// Its `schema.json`.
    pub(crate) schema: PathBuf,
    /// Its full version, read from its `VERSION` file (MG-REQ-0204).
    pub(crate) semver: Semver,
}

/// Lists every contract version under `schemas`, sorted by contract name, then by version
/// number. Files directly under `schemas`, such as `README.md`, are not contracts.
///
/// # Errors
///
/// Fails if a directory cannot be read, or if a directory breaks the layout: a contract
/// version without `schema.json`, or without a well-formed `VERSION` file.
pub(crate) fn versions(schemas: &Path) -> Result<Vec<Version>, Failure> {
    let mut versions = Vec::new();
    for contract_dir in workspace::sorted_entries(schemas)? {
        if !contract_dir.is_dir() {
            continue;
        }
        let contract = file_name(&contract_dir);
        if contract.contains('-') || pascal(&contract).is_none() {
            return Err(broken(&contract_dir, "a contract name must be snake_case"));
        }
        let mut found = Vec::new();
        for version_dir in workspace::sorted_entries(&contract_dir)? {
            let version = file_name(&version_dir)
                .strip_prefix('v')
                .filter(|digits| !digits.starts_with('0'))
                .and_then(|digits| digits.parse::<u32>().ok())
                .filter(|_| version_dir.is_dir())
                .ok_or_else(|| broken(&version_dir, "expected a directory `v<version>`"))?;
            let schema = version_dir.join("schema.json");
            if !schema.is_file() {
                return Err(broken(&version_dir, "`schema.json` is missing"));
            }
            let semver = semver(&version_dir, version)?;
            found.push(Version {
                contract: contract.clone(),
                version,
                schema,
                semver,
            });
        }
        found.sort_by_key(|version| version.version);
        versions.extend(found);
    }
    Ok(versions)
}

/// Reads the `VERSION` file of the directory `v<version>` (MG-REQ-0204).
fn semver(version_dir: &Path, version: u32) -> Result<Semver, Failure> {
    let path = version_dir.join("VERSION");
    if !path.is_file() {
        return Err(broken(version_dir, "`VERSION` is missing"));
    }
    version::read(&workspace::read(&path)?, u64::from(version))
        .map_err(|problem| broken(&path, &problem))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn broken(path: &Path, message: &str) -> Failure {
    Failure::new(format!("{}: {message}", workspace::shown(path)))
}
