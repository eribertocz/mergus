//! Read the members of a workspace and their declared dependencies from `cargo metadata`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cargo;
use crate::failure::Failure;
use crate::json::Json;
use crate::workspace;

/// A workspace as `cargo metadata --no-deps` describes it.
pub(crate) struct Workspace {
    /// The directory of the root `Cargo.toml`.
    pub(crate) root: PathBuf,
    /// The members, sorted by name.
    pub(crate) members: Vec<Package>,
}

/// One workspace member.
pub(crate) struct Package {
    /// The package name.
    pub(crate) name: String,
    /// The directory that holds the member's `Cargo.toml`.
    pub(crate) dir: PathBuf,
    /// The dependencies, in the order `cargo metadata` lists them.
    pub(crate) dependencies: Vec<Dependency>,
    /// The `[features]` table: each feature and what it enables.
    pub(crate) features: BTreeMap<String, Vec<String>>,
}

/// The table a dependency is declared in.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `[dependencies]`.
    Normal,
    /// `[build-dependencies]`.
    Build,
    /// `[dev-dependencies]`.
    Dev,
}

/// One declared dependency of a member.
pub(crate) struct Dependency {
    /// The name of the package depended on.
    pub(crate) name: String,
    /// The name the member uses for it: the rename, or the package name.
    pub(crate) key: String,
    /// The table it is declared in.
    pub(crate) kind: Kind,
    /// The registry or git source, or `None` for a path dependency.
    pub(crate) source: Option<String>,
    /// The directory of a path dependency.
    pub(crate) path: Option<PathBuf>,
    /// The features the member enables on it.
    pub(crate) features: Vec<String>,
    /// Whether it is optional: linked only when a feature enables it.
    pub(crate) optional: bool,
}

impl Workspace {
    /// Returns the member a dependency resolves to, or `None` if it is not a member.
    pub(crate) fn resolve(&self, dependency: &Dependency) -> Option<&Package> {
        if dependency.source.is_some() {
            return None;
        }
        let path = dependency.path.as_deref()?;
        self.members.iter().find(|member| member.dir == path)
    }

    /// Returns the member named `name`, or `None`.
    pub(crate) fn member(&self, name: &str) -> Option<&Package> {
        self.members.iter().find(|member| member.name == name)
    }

    /// Shows `path` relative to the workspace root when it is inside it.
    pub(crate) fn show(&self, path: &Path) -> String {
        workspace::show(&self.root, path)
    }
}

/// Runs `cargo metadata` for the workspace that contains `dir`.
///
/// # Errors
///
/// Fails if Cargo fails or its output is not the expected JSON.
pub(crate) fn read(dir: &Path) -> Result<Workspace, Failure> {
    let args = [
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--offline",
    ];
    parse(&cargo::output(dir, &args)?)
}

/// Parses the JSON printed by `cargo metadata --format-version 1 --no-deps`.
///
/// # Errors
///
/// Fails if the text is not JSON or lacks a field the checks need.
pub(crate) fn parse(text: &str) -> Result<Workspace, Failure> {
    let json = Json::parse(text)?;
    let root = PathBuf::from(string(&json, "workspace_root")?);
    let mut members = field(&json, "packages")?
        .items()
        .iter()
        .map(package)
        .collect::<Result<Vec<_>, _>>()?;
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Workspace { root, members })
}

fn package(json: &Json) -> Result<Package, Failure> {
    let manifest = PathBuf::from(string(json, "manifest_path")?);
    let dir = manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| Failure::new(format!("{} has no parent", workspace::shown(&manifest))))?;
    let dependencies = field(json, "dependencies")?
        .items()
        .iter()
        .map(dependency)
        .collect::<Result<Vec<_>, _>>()?;
    let features = field(json, "features")?
        .members()
        .iter()
        .map(|(name, enables)| (name.clone(), strings(enables)))
        .collect();
    Ok(Package {
        name: string(json, "name")?.to_owned(),
        dir,
        dependencies,
        features,
    })
}

fn dependency(json: &Json) -> Result<Dependency, Failure> {
    let name = string(json, "name")?.to_owned();
    let kind = match json.get("kind").and_then(Json::as_str) {
        None => Kind::Normal,
        Some("build") => Kind::Build,
        Some("dev") => Kind::Dev,
        Some(other) => {
            return Err(Failure::new(format!(
                "dependency `{name}` has an unknown kind `{other}`"
            )));
        }
    };
    let key = json
        .get("rename")
        .and_then(Json::as_str)
        .unwrap_or(&name)
        .to_owned();
    Ok(Dependency {
        key,
        kind,
        source: json.get("source").and_then(Json::as_str).map(str::to_owned),
        path: json.get("path").and_then(Json::as_str).map(PathBuf::from),
        features: json.get("features").map(strings).unwrap_or_default(),
        optional: matches!(json.get("optional"), Some(Json::Bool(true))),
        name,
    })
}

fn field<'a>(json: &'a Json, key: &str) -> Result<&'a Json, Failure> {
    json.get(key)
        .ok_or_else(|| Failure::new(format!("`cargo metadata` output has no `{key}`")))
}

fn string<'a>(json: &'a Json, key: &str) -> Result<&'a str, Failure> {
    field(json, key)?
        .as_str()
        .ok_or_else(|| Failure::new(format!("`cargo metadata` field `{key}` is not a string")))
}

fn strings(json: &Json) -> Vec<String> {
    json.items()
        .iter()
        .filter_map(Json::as_str)
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Kind, parse};

    const METADATA: &str = r#"{
        "workspace_root": "/w",
        "packages": [
            {
                "name": "b",
                "manifest_path": "/w/crates/b/Cargo.toml",
                "dependencies": [],
                "features": {"default": ["real"], "real": []}
            },
            {
                "name": "a",
                "manifest_path": "/w/crates/a/Cargo.toml",
                "dependencies": [
                    {"name": "b", "kind": null, "rename": null, "source": null,
                     "path": "/w/crates/b", "features": ["real"]},
                    {"name": "b", "kind": "dev", "rename": "c", "source": null,
                     "path": "/w/crates/b", "features": []},
                    {"name": "keyring", "kind": "build", "rename": null,
                     "source": "registry+https://github.com/rust-lang/crates.io-index",
                     "features": [], "optional": true}
                ],
                "features": {}
            }
        ]
    }"#;

    #[test]
    fn members_are_sorted_and_dependencies_resolved() {
        let workspace = parse(METADATA).unwrap();
        let names: Vec<_> = workspace.members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        let a = workspace.member("a").unwrap();
        assert_eq!(a.dir, Path::new("/w/crates/a"));
        let [normal, dev, build] = &a.dependencies[..] else {
            panic!("expected three dependencies");
        };
        assert!(normal.kind == Kind::Normal && normal.features == ["real"]);
        assert!(dev.kind == Kind::Dev && dev.key == "c" && dev.name == "b");
        assert!(build.kind == Kind::Build && build.source.is_some());
        assert!(build.optional && !normal.optional && !dev.optional);
        assert_eq!(
            workspace.resolve(normal).map(|m| m.name.as_str()),
            Some("b")
        );
        assert!(workspace.resolve(build).is_none());
        let b = workspace.member("b").unwrap();
        assert_eq!(b.features["default"], ["real"]);
        assert_eq!(workspace.show(&b.dir), "crates/b");
    }

    #[test]
    fn missing_fields_fail() {
        assert!(parse(r#"{"packages": []}"#).is_err());
        assert!(parse(r#"{"workspace_root": "/w"}"#).is_err());
    }
}
