//! The errors of reading a project.

use std::fmt;

use mergus_ports::PortError;

/// The kind of a name checked by MG-REQ-0305.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NameKind {
    /// A model: the stem of a `.sql` or `.py` file under `models/`.
    Model,
    /// A folder: a directory under `models/`, other than a schema.
    Folder,
    /// A schema: a directory directly under `models/gold/`.
    Schema,
    /// A source: the stem of a `sources/*.toml` file.
    Source,
    /// A seed: the stem of a `seeds/*.csv` file.
    Seed,
}

impl fmt::Display for NameKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Model => "model",
            Self::Folder => "folder",
            Self::Schema => "schema",
            Self::Source => "source",
            Self::Seed => "seed",
        })
    }
}

/// A problem of the project. Paths are relative to the project root, separated by `/`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectError {
    /// Two model files have the same stem (MG-REQ-0303).
    #[error("model `{name}` is defined twice: `{first}` and `{second}`")]
    DuplicateModel {
        /// The model name.
        name: String,
        /// The path of the first file, in walking order.
        first: String,
        /// The path of the later file.
        second: String,
    },
    /// A name is not a lowercase identifier (MG-REQ-0305).
    #[error("{kind} name `{name}` of `{path}` is not a lowercase identifier; use `{suggestion}`")]
    InvalidName {
        /// What the name names.
        kind: NameKind,
        /// The name given.
        name: String,
        /// The path of the file or directory that carries the name.
        path: String,
        /// The valid name of MG-REQ-0360.
        suggestion: String,
    },
}

impl ProjectError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::DuplicateModel { .. } => "MG-0301",
            Self::InvalidName { .. } => "MG-0302",
        }
    }

    /// Returns the path that caused the error, relative to the project root.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::DuplicateModel { second, .. } => second,
            Self::InvalidName { path, .. } => path,
        }
    }
}

/// Why a project layout could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The file system port failed.
    #[error(transparent)]
    Port(#[from] PortError),
    /// The project has problems, every one of them, in walking order.
    #[error("the project has {} problem(s)", .0.len())]
    Invalid(Vec<ProjectError>),
}
