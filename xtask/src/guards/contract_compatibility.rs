//! The `contract-compatibility` guard: every contract version a pull request changes keeps
//! its `VERSION` file in step with the kind of the change (MG-REQ-0204, MG-REQ-0205).
//!
//! A contract version is the directory `schemas/<contract>/v<major>/`, which holds
//! `schema.json` and `VERSION`. A schema the pull request modifies is compared with its text
//! at the base commit, which is `main` for a pull request into `main`:
//!
//! - a major change fails: it needs a new directory `v<major + 1>` instead;
//! - a minor or patch change needs `VERSION` raised in that part, with the lower parts reset
//!   to 0;
//! - a `VERSION` that changes without a change to the schema fails.
//!
//! A schema the pull request adds starts a new contract or a new major version, so its
//! `VERSION` must be `<major>.0.0`. A `VERSION` that is missing or is not one line
//! `MAJOR.MINOR.PATCH` with the major version of its directory fails.

use super::Event;
use super::github::{self, Api};
use crate::contracts::{Bump, Semver, classify, read_version};
use crate::failure::Failure;
use crate::json::Json;

/// How a pull request changes one file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Added,
    Modified,
    Removed,
}

/// The files of one contract version that a pull request changes.
struct Touched {
    /// The directory `schemas/<contract>/v<major>`.
    dir: String,
    major: u64,
    schema: Option<Status>,
    version: Option<Status>,
}

/// Returns the findings of the `contract-compatibility` guard.
///
/// # Errors
///
/// Fails if the API cannot be read, or the text of a schema at the base is not JSON.
pub(crate) fn judge(event: &Event, api: &dyn Api) -> Result<Vec<String>, Failure> {
    let mut touched: Vec<Touched> = Vec::new();
    for file in github::files(api, &event.repository, event.number)? {
        for (path, status) in changes(&file.paths, &file.status) {
            let Some((dir, major, name)) = contract_file(path) else {
                continue;
            };
            let index = match touched.iter().position(|seen| seen.dir == dir) {
                Some(index) => index,
                None => {
                    touched.push(Touched {
                        dir: dir.to_owned(),
                        major,
                        schema: None,
                        version: None,
                    });
                    touched.len() - 1
                }
            };
            let entry = &mut touched[index];
            if name == "schema.json" {
                entry.schema = Some(status);
            } else {
                entry.version = Some(status);
            }
        }
    }
    let mut findings = Vec::new();
    for entry in &touched {
        let found = Reader { event, api }.judge(entry)?;
        findings.extend(
            found
                .into_iter()
                .map(|finding| format!("MG-REQ-0205: {finding}")),
        );
    }
    Ok(findings)
}

/// Each path a changed file adds, modifies or removes. A renamed file removes its old path
/// and adds its new one; a copied file adds its new path.
fn changes<'a>(paths: &'a [String], status: &str) -> Vec<(&'a str, Status)> {
    let path = |index: usize| paths.get(index).map(String::as_str);
    let found = match status {
        "added" => vec![(path(0), Status::Added)],
        "modified" | "changed" => vec![(path(0), Status::Modified)],
        "removed" => vec![(path(0), Status::Removed)],
        "renamed" => vec![(path(0), Status::Added), (path(1), Status::Removed)],
        "copied" => vec![(path(0), Status::Added)],
        _ => Vec::new(),
    };
    found
        .into_iter()
        .filter_map(|(path, status)| Some((path?, status)))
        .collect()
}

/// Splits a repository path, as GitHub writes it, that is
/// `schemas/<contract>/v<major>/schema.json` or `schemas/<contract>/v<major>/VERSION` into
/// its directory, its major version and its file name.
fn contract_file(path: &str) -> Option<(&str, u64, &str)> {
    let parts: Vec<&str> = path.split('/').collect();
    let ["schemas", contract, version, name] = parts.as_slice() else {
        return None;
    };
    let major = version
        .strip_prefix('v')
        .filter(|digits| !digits.is_empty() && !digits.starts_with('0'))
        .filter(|digits| digits.chars().all(|c| c.is_ascii_digit()))
        .and_then(|digits| digits.parse::<u64>().ok())?;
    let file = ["schema.json", "VERSION"].contains(name);
    (!contract.is_empty() && file).then(|| (&path[..path.len() - name.len() - 1], major, *name))
}

/// Reads the files of the pull request at its base and at its head.
struct Reader<'a> {
    event: &'a Event,
    api: &'a dyn Api,
}

impl Reader<'_> {
    fn text(&self, path: &str, sha: &str) -> Result<String, Failure> {
        github::file(self.api, &self.event.repository, path, sha)
    }

    /// The findings on one contract version, without their rule prefix.
    fn judge(&self, entry: &Touched) -> Result<Vec<String>, Failure> {
        let dir = &entry.dir;
        let missing = || vec![format!("`{dir}/VERSION` is missing")];
        match (entry.schema, entry.version) {
            (Some(Status::Removed), _) => Ok(Vec::new()),
            (_, Some(Status::Removed)) | (Some(Status::Added), None) => Ok(missing()),
            (Some(Status::Added), Some(_)) => self.new_version(entry),
            (Some(Status::Modified), version) => match self.schema_change(dir)? {
                Ok(bump) => self.bump(entry, bump, version),
                Err(findings) => Ok(findings),
            },
            (None, version) => self.bump(entry, Bump::None, version),
        }
    }

    /// A new contract or a new major version starts at `<major>.0.0`.
    fn new_version(&self, entry: &Touched) -> Result<Vec<String>, Failure> {
        let first = Semver::first(entry.major);
        let path = format!("{}/VERSION", entry.dir);
        Ok(match self.version(&path, &self.event.head, entry.major)? {
            Err(finding) => vec![finding],
            Ok(version) if version == first => Vec::new(),
            Ok(version) => vec![format!(
                "`{path}` is `{version}`, but a new contract or major version starts at \
                     `{first}`"
            )],
        })
    }

    /// Classifies the change of `<dir>/schema.json`, or lists why it needs a new major
    /// version.
    fn schema_change(&self, dir: &str) -> Result<Result<Bump, Vec<String>>, Failure> {
        let path = format!("{dir}/schema.json");
        let base = Json::parse(&self.text(&path, &self.event.base)?)
            .map_err(|failure| Failure::new(format!("{path} at the base: {failure:?}")))?;
        let head = match Json::parse(&self.text(&path, &self.event.head)?) {
            Ok(head) => head,
            Err(failure) => return Ok(Err(vec![format!("`{path}` is not JSON: {failure:?}")])),
        };
        Ok(classify(&base, &head).map_err(|problems| {
            problems
                .into_iter()
                .map(|problem| {
                    format!("`{path}` changes incompatibly without a new major version: {problem}")
                })
                .collect()
        }))
    }

    /// Checks that `VERSION` moves from its base by exactly `bump`.
    fn bump(
        &self,
        entry: &Touched,
        bump: Bump,
        version: Option<Status>,
    ) -> Result<Vec<String>, Failure> {
        let path = format!("{}/VERSION", entry.dir);
        let change = || format!("{bump} change to `schema.json`");
        let (base, head) = match version {
            None if bump == Bump::None => return Ok(Vec::new()),
            None => {
                return Ok(vec![format!(
                    "`{path}` is unchanged, but {} needs a new version",
                    change()
                )]);
            }
            Some(Status::Added) => return Ok(vec![format!("`{path}` is missing at the base")]),
            Some(_) => (
                self.version(&path, &self.event.base, entry.major)?,
                self.version(&path, &self.event.head, entry.major)?,
            ),
        };
        Ok(match (base, head) {
            (Ok(base), Ok(head)) => {
                let expected = base.bumped(bump);
                if head == expected {
                    Vec::new()
                } else {
                    vec![format!(
                        "`{path}` goes from `{base}` to `{head}`, but {} needs `{expected}`",
                        change()
                    )]
                }
            }
            (base, head) => base.err().into_iter().chain(head.err()).collect(),
        })
    }

    /// Reads the `VERSION` file at `path` in the commit `sha`: its version, or the finding
    /// that it is malformed.
    fn version(
        &self,
        path: &str,
        sha: &str,
        major: u64,
    ) -> Result<Result<Semver, String>, Failure> {
        let at = if sha == self.event.base {
            " at the base"
        } else {
            ""
        };
        Ok(read_version(&self.text(path, sha)?, major)
            .map_err(|problem| format!("`{path}`{at} is malformed: {problem}")))
    }
}

#[cfg(test)]
mod tests {
    use super::contract_file;

    #[test]
    fn mg_req_0205_only_contract_version_files_are_compared() {
        assert_eq!(
            contract_file("schemas/catalog_state/v12/schema.json"),
            Some(("schemas/catalog_state/v12", 12, "schema.json"))
        );
        assert_eq!(
            contract_file("schemas/sample/v1/VERSION"),
            Some(("schemas/sample/v1", 1, "VERSION"))
        );
        for path in [
            "schemas/README.md",
            "schemas/sample/v1/valid/full.json",
            "schemas/sample/v0/schema.json",
            "schemas/sample/v01/VERSION",
            "schemas/sample/latest/schema.json",
            "schemas/sample/v1/version",
            "crates/x/schemas/sample/v1/schema.json",
        ] {
            assert_eq!(contract_file(path), None, "{path}");
        }
    }
}
