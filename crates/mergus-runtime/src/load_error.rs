//! The failures of an incremental load of one table.

use mergus_source::ExtractError;

use crate::RawError;

/// Why an incremental load of a table failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// The source could not be read.
    #[error(transparent)]
    Source(#[from] ExtractError),
    /// Raw or the load registry could not be written or read.
    #[error(transparent)]
    Raw(#[from] RawError),
}

impl LoadError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008); `None` for a port failure of
    /// the source or a failure of the load registry.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Source(error) => error.code(),
            Self::Raw(error) => error.code(),
        }
    }
}
