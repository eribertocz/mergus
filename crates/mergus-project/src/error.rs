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
    /// A configuration file is not TOML 1.0 (MG-REQ-0316).
    #[error("`{path}` line {line} is not TOML 1.0: {message}")]
    InvalidToml {
        /// The path of the file.
        path: String,
        /// The line of the problem, from 1.
        line: usize,
        /// What the parser found.
        message: String,
    },
    /// A key is not in the schema of its file (MG-REQ-0316, MG-REQ-0364).
    #[error("`{path}` line {line}: unknown key `{key}`{}", hint(suggestion.as_deref()))]
    UnknownKey {
        /// The path of the file.
        path: String,
        /// The line of the key, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
        /// The closest key allowed at that place (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A value has the wrong type, is invalid, or a required key is missing (MG-REQ-0316).
    #[error("`{path}` line {line}: key `{key}`: {message}{}", hint(suggestion.as_deref()))]
    InvalidValue {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
        /// What the schema expects.
        message: String,
        /// The closest allowed value (MG-REQ-0359).
        suggestion: Option<String>,
    },
    /// A string value contains `${`, `{{` or `{%` (MG-REQ-0364).
    #[error(
        "`{path}` line {line}: key `{key}` holds an expression; configuration values are literals"
    )]
    Expression {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
    /// A float is NaN or infinite, which JSON cannot hold (MG-REQ-0365).
    #[error("`{path}` line {line}: key `{key}` is NaN or infinite")]
    NonFiniteFloat {
        /// The path of the file.
        path: String,
        /// The line of the value, from 1.
        line: usize,
        /// The key, dotted from the root of the file.
        key: String,
    },
}

/// Formats the closest valid name of an error, when there is one.
fn hint(suggestion: Option<&str>) -> String {
    suggestion.map_or_else(String::new, |name| format!("; did you mean `{name}`?"))
}

impl ProjectError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::DuplicateModel { .. } => "MG-0301",
            Self::InvalidName { .. } => "MG-0302",
            Self::InvalidToml { .. } => "MG-0303",
            Self::UnknownKey { .. } => "MG-0304",
            Self::InvalidValue { .. } => "MG-0305",
            Self::Expression { .. } => "MG-0306",
            Self::NonFiniteFloat { .. } => "MG-0307",
        }
    }

    /// Returns the path that caused the error, relative to the project root.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::DuplicateModel { second, .. } => second,
            Self::InvalidName { path, .. }
            | Self::InvalidToml { path, .. }
            | Self::UnknownKey { path, .. }
            | Self::InvalidValue { path, .. }
            | Self::Expression { path, .. }
            | Self::NonFiniteFloat { path, .. } => path,
        }
    }

    /// Returns the line of the file that caused the error, from 1, when there is one.
    #[must_use]
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::DuplicateModel { .. } | Self::InvalidName { .. } => None,
            Self::InvalidToml { line, .. }
            | Self::UnknownKey { line, .. }
            | Self::InvalidValue { line, .. }
            | Self::Expression { line, .. }
            | Self::NonFiniteFloat { line, .. } => Some(*line),
        }
    }

    /// Returns the configuration key that caused the error, dotted from the root of its file.
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        match self {
            Self::DuplicateModel { .. } | Self::InvalidName { .. } | Self::InvalidToml { .. } => {
                None
            }
            Self::UnknownKey { key, .. }
            | Self::InvalidValue { key, .. }
            | Self::Expression { key, .. }
            | Self::NonFiniteFloat { key, .. } => Some(key),
        }
    }
}

/// Why a project layout could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The file system port failed.
    #[error(transparent)]
    Port(#[from] PortError),
    /// A bundled configuration schema cannot be compiled: a defect of Mergus itself.
    #[error("configuration schema `{0}` is invalid")]
    Schema(&'static str),
    /// The project has problems, every one of them, in walking order.
    #[error("the project has {} problem(s)", .0.len())]
    Invalid(Vec<ProjectError>),
}
