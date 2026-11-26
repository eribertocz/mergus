//! The problems of the storage of the zones.

/// A problem of the storage of the zones of `project.toml`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StorageProblem {
    /// raw and gold would sit behind the same credentials: the same or nested locations, or
    /// the same catalog secret (MG-REQ-0501).
    #[error(
        "`{path}` line {line}: raw and gold are behind the same {key} `{value}`; raw and gold \
         must be separate stores, each with its own credentials"
    )]
    SharedStore {
        /// The path of `project.toml`, relative to the project root.
        path: String,
        /// The line of the key of gold, from 1.
        line: usize,
        /// The key both zones share: `location` or `catalog`.
        key: &'static str,
        /// The shared value, as written: the location that holds the other, or the secret
        /// reference.
        value: String,
    },
}

impl StorageProblem {
    /// Returns the `MG-####` code of the problem (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::SharedStore { .. } => "MG-0501",
        }
    }

    /// Returns the path of the file, relative to the project root.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::SharedStore { path, .. } => path,
        }
    }

    /// Returns the line of the problem, from 1.
    #[must_use]
    pub fn line(&self) -> usize {
        match self {
            Self::SharedStore { line, .. } => *line,
        }
    }

    /// Returns the dotted key at fault, such as `gold.location`.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::SharedStore { key, .. } => format!("gold.{key}"),
        }
    }
}
