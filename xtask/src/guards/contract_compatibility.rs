//! The `contract-compatibility` guard: a pull request that changes a schema in place keeps
//! every document of its major version valid, or the change needs a new major version
//! (MG-REQ-0204, MG-REQ-0205).
//!
//! A schema lives at `schemas/<contract>/v<major>/schema.json`. A new major version is a new
//! `v<major>` directory, so a schema the pull request adds has no older text to compare with.
//! A schema it modifies is compared with its text at the base commit, which is `main` for a
//! pull request into `main`.

use super::Event;
use super::github::{self, Api};
use crate::contracts::incompatibilities;
use crate::failure::Failure;
use crate::json::Json;

/// The statuses GitHub gives a file whose path also exists at the base commit.
const MODIFIED: [&str; 2] = ["modified", "changed"];

/// Returns the findings of the `contract-compatibility` guard.
///
/// # Errors
///
/// Fails if the API cannot be read, or the text of a schema at the base is not JSON.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    let repo = &event.repository;
    let mut findings = Vec::new();
    for file in github::files(api, repo, event.number)? {
        let Some(path) = file.paths.first() else {
            continue;
        };
        if !MODIFIED.contains(&file.status.as_str()) || !is_schema(path) {
            continue;
        }
        let base = Json::parse(&github::file(api, repo, path, &event.base)?)
            .map_err(|failure| Failure::new(format!("{path} at the base: {failure:?}")))?;
        let head = match Json::parse(&github::file(api, repo, path, &event.head)?) {
            Ok(head) => head,
            Err(failure) => {
                findings.push(format!("MG-REQ-0205: `{path}` is not JSON: {failure:?}"));
                continue;
            }
        };
        findings.extend(incompatibilities(&base, &head).into_iter().map(|problem| {
            format!(
                "MG-REQ-0205: `{path}` changes incompatibly without a new major version: \
                 {problem}"
            )
        }));
    }
    Ok(findings)
}

/// Whether a repository path, as GitHub writes it, is `schemas/<contract>/v<major>/schema.json`.
fn is_schema(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let major = |part: &str| {
        part.strip_prefix('v').is_some_and(|digits| {
            !digits.is_empty()
                && !digits.starts_with('0')
                && digits.chars().all(|c| c.is_ascii_digit())
        })
    };
    matches!(
        parts.as_slice(),
        ["schemas", contract, version, "schema.json"] if !contract.is_empty() && major(version)
    )
}

#[cfg(test)]
mod tests {
    use super::is_schema;

    #[test]
    fn mg_req_0205_only_schema_files_are_compared() {
        assert!(is_schema("schemas/catalog_state/v12/schema.json"));
        for path in [
            "schemas/README.md",
            "schemas/sample/v1/valid/full.json",
            "schemas/sample/v0/schema.json",
            "schemas/sample/v01/schema.json",
            "schemas/sample/latest/schema.json",
            "crates/x/schemas/sample/v1/schema.json",
        ] {
            assert!(!is_schema(path), "{path}");
        }
    }
}
