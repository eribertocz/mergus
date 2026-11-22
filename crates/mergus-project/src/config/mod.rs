//! The configuration files `project.toml` and `sources/*.toml`: TOML 1.0 validated by their
//! JSON Schemas (MG-REQ-0316), literal values only (MG-REQ-0364), canonical JSON (MG-REQ-0318).

mod convert;
mod schema;

use std::path::Path;

use mergus_ports::FileSystem;
use serde_json::Value;

use crate::{LayoutError, ProjectError};

/// The kind of a configuration file, which chooses its schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConfigKind {
    /// `project.toml`.
    Project,
    /// A `sources/<source>.toml` file.
    Source,
}

impl ConfigKind {
    /// Returns the text of the JSON Schema of this kind of file (MG-REQ-0361).
    #[must_use]
    pub fn schema(self) -> &'static str {
        match self {
            Self::Project => include_str!("../../../../schemas/config/project.schema.json"),
            Self::Source => include_str!("../../../../schemas/config/source.schema.json"),
        }
    }

    /// Returns the path of the schema, relative to the repository root.
    #[must_use]
    pub fn schema_path(self) -> &'static str {
        match self {
            Self::Project => "schemas/config/project.schema.json",
            Self::Source => "schemas/config/source.schema.json",
        }
    }

    /// Returns the `$id` of the schema.
    #[must_use]
    pub fn schema_id(self) -> &'static str {
        match self {
            Self::Project => "https://mergus.dev/schemas/project.schema.json",
            Self::Source => "https://mergus.dev/schemas/source.schema.json",
        }
    }
}

/// A valid configuration file.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigFile {
    /// The path of the file, relative to the project root, separated by `/`.
    pub path: String,
    /// The JSON of the file (MG-REQ-0365).
    pub value: Value,
    /// Its canonical JSON (RFC 8785), from which fingerprints are computed (MG-REQ-0318).
    pub canonical: String,
}

/// A source, declared in its own file (MG-REQ-0306).
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    /// The source name: the stem of `sources/<source>.toml`.
    pub name: String,
    /// Its configuration file.
    pub file: ConfigFile,
}

/// The configuration of a project.
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration {
    /// `project.toml`.
    pub project: ConfigFile,
    /// The sources, in the sorted order of their files.
    pub sources: Vec<Source>,
}

/// Reads and validates `project.toml` and every `sources/*.toml` of the project at `root`.
///
/// # Errors
///
/// Fails with [`LayoutError::Port`] when a file cannot be read, [`LayoutError::Schema`] when
/// a bundled schema is broken, and [`LayoutError::Invalid`] with every problem found, file by
/// file, each file's problems ordered by line.
pub fn read_configuration(
    files: &dyn FileSystem,
    root: &Path,
) -> Result<Configuration, LayoutError> {
    let mut problems = Vec::new();
    let project = read_file(
        files,
        root,
        "project.toml",
        ConfigKind::Project,
        &mut problems,
    )?;
    let mut sources = Vec::new();
    let directory = root.join("sources");
    if files.exists(&directory)? {
        for entry in files.list_dir(&directory)? {
            let Some(name) = source_name(&entry) else {
                continue;
            };
            let path = format!("sources/{name}.toml");
            if let Some(file) = read_file(files, root, &path, ConfigKind::Source, &mut problems)? {
                sources.push(Source { name, file });
            }
        }
    }
    match project {
        Some(project) if problems.is_empty() => Ok(Configuration { project, sources }),
        _ => Err(LayoutError::Invalid(problems)),
    }
}

/// The stem of a `.toml` file of `sources/`.
fn source_name(entry: &Path) -> Option<String> {
    let is_toml = entry
        .extension()
        .is_some_and(|extension| extension == "toml");
    let stem = entry.file_stem()?.to_str()?;
    is_toml.then(|| stem.to_owned())
}

fn read_file(
    files: &dyn FileSystem,
    root: &Path,
    path: &str,
    kind: ConfigKind,
    problems: &mut Vec<ProjectError>,
) -> Result<Option<ConfigFile>, LayoutError> {
    let bytes = files.read(&root.join(path))?;
    let mut found = Vec::new();
    let file = parse(kind, path, &bytes, &mut found)?;
    found.sort_by_key(|problem| problem.line());
    let valid = found.is_empty();
    problems.append(&mut found);
    Ok(file.filter(|_| valid))
}

/// Parses the bytes of one configuration file of `kind`.
fn parse(
    kind: ConfigKind,
    path: &str,
    bytes: &[u8],
    problems: &mut Vec<ProjectError>,
) -> Result<Option<ConfigFile>, LayoutError> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        problems.push(ProjectError::InvalidToml {
            path: path.to_owned(),
            line: 1,
            message: "the file is not UTF-8".to_owned(),
        });
        return Ok(None);
    };
    let Some(converted) = convert::convert(path, text, problems) else {
        return Ok(None);
    };
    schema::validate(kind, path, &converted, problems)?;
    let canonical = mergus_contracts::canonical_json(&converted.value);
    Ok(Some(ConfigFile {
        path: path.to_owned(),
        value: converted.value,
        canonical,
    }))
}
