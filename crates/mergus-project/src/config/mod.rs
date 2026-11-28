//! The configuration files `project.toml`, `sources/*.toml` and folder `mergus.toml`: TOML 1.0
//! validated by their JSON Schemas (MG-REQ-0316), literal values only (MG-REQ-0364), secrets
//! by reference only (MG-REQ-0320), canonical
//! JSON (MG-REQ-0318), and the effective options of each model (MG-REQ-0332).

mod convert;
mod folder;
mod limits;
mod options;
mod schema;
mod secret;

pub use limits::{ProjectLimits, parse_positive, parse_wall_time};
pub use options::{Effective, ModelOptions, Origin, Setting, read_model_options};

pub(crate) use convert::key_order;

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
    /// A folder `mergus.toml` under `models/`.
    Folder,
}

impl ConfigKind {
    /// Returns the text of the JSON Schema of this kind of file (MG-REQ-0361).
    #[must_use]
    pub fn schema(self) -> &'static str {
        match self {
            Self::Project => include_str!("../../schemas/project.schema.json"),
            Self::Source => include_str!("../../schemas/source.schema.json"),
            Self::Folder => include_str!("../../schemas/folder.schema.json"),
        }
    }

    /// Returns the path of the schema, relative to the repository root.
    #[must_use]
    pub fn schema_path(self) -> &'static str {
        match self {
            Self::Project => "crates/mergus-project/schemas/project.schema.json",
            Self::Source => "crates/mergus-project/schemas/source.schema.json",
            Self::Folder => "crates/mergus-project/schemas/folder.schema.json",
        }
    }

    /// Returns the `$id` of the schema.
    #[must_use]
    pub fn schema_id(self) -> &'static str {
        match self {
            Self::Project => "https://mergus.dev/schemas/project.schema.json",
            Self::Source => "https://mergus.dev/schemas/source.schema.json",
            Self::Folder => "https://mergus.dev/schemas/folder.schema.json",
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
/// file, each file's problems ordered by line, code and key.
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
    sort_problems(&mut found);
    let valid = found.is_empty();
    problems.append(&mut found);
    Ok(file.filter(|_| valid))
}

/// Sorts the problems of one file by a total order: line, then code, then key path, so that
/// their order does not depend on the order they were found in (MG-REQ-0005).
pub(crate) fn sort_problems(problems: &mut [ProjectError]) {
    problems.sort_by(|left, right| {
        (left.line(), left.code(), left.key()).cmp(&(right.line(), right.code(), right.key()))
    });
}

/// Reads `text`, the TOML document of the file `path`, as JSON with the mapping of
/// MG-REQ-0365, without checking it against any schema.
///
/// # Errors
///
/// Fails with every problem found when `text` is not TOML 1.0 or a value cannot be mapped.
pub fn toml_to_json(path: &str, text: &str) -> Result<Value, Vec<ProjectError>> {
    let mut problems = Vec::new();
    match convert::convert(path, text, &mut problems) {
        Some(converted) if problems.is_empty() => Ok(converted.value),
        _ => Err(problems),
    }
}

/// Returns the line, from 1, of the key at `keys`, a path of table keys from the root of
/// `text`, the TOML document of a configuration file: the line the key is written on, in a
/// table header, a dotted key or an inline table. `None` when `keys` is empty, the file does
/// not hold that key, or `text` is not TOML 1.0. Other crates locate their problems with it
/// rather than parsing TOML themselves (MG-REQ-0186).
#[must_use]
pub fn key_line(text: &str, keys: &[&str]) -> Option<usize> {
    let converted = convert::convert("", text, &mut Vec::new())?;
    let path: Vec<convert::Segment> = keys
        .iter()
        .map(|key| convert::Segment::Key((*key).to_owned()))
        .collect();
    converted.lines.get(&path).map(|lines| lines.key)
}

/// Parses the bytes of one configuration file of `kind`.
fn parse(
    kind: ConfigKind,
    path: &str,
    bytes: &[u8],
    problems: &mut Vec<ProjectError>,
) -> Result<Option<ConfigFile>, LayoutError> {
    let Some(text) = utf8(path, bytes, problems) else {
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

/// The text of a configuration file, or `None` with a problem when it is not UTF-8.
fn utf8<'a>(path: &str, bytes: &'a [u8], problems: &mut Vec<ProjectError>) -> Option<&'a str> {
    let text = std::str::from_utf8(bytes).ok();
    if text.is_none() {
        problems.push(ProjectError::InvalidToml {
            path: path.to_owned(),
            line: 1,
            message: "the file is not UTF-8".to_owned(),
        });
    }
    text
}
