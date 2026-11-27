//! The warnings of reading a project: they never fail the read (MG-REQ-0646.2).

/// A warning about the project. Paths are relative to the project root, separated by `/`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectWarning {
    /// A seed has more than 10,000,000 bytes (MG-REQ-0646).
    #[error(
        "seed `{path}` has {bytes} bytes, more than 10,000,000; load data of this size from a \
         source instead"
    )]
    LargeSeed {
        /// The path of the CSV file.
        path: String,
        /// Its size in bytes.
        bytes: u64,
    },
}

impl ProjectWarning {
    /// Returns the `MG-####` code of the warning (MG-REQ-0646.2).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::LargeSeed { .. } => "MG-0617",
        }
    }

    /// Returns the path of the file the warning is about, relative to the project root.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::LargeSeed { path, .. } => path,
        }
    }
}
