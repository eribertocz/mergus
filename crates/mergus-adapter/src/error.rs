//! The errors of the adapter contract.

use std::fmt;

use arrow::error::ArrowError;

use crate::{Requirement, VersionRef};

/// Why an adapter operation failed. Later versions may add variants.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AdapterError {
    /// The Arrow stream failed.
    #[error("arrow stream failed: {0}")]
    Arrow(#[from] ArrowError),
    /// The storage failed.
    #[error("storage failed: {0}")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The table cannot be read at the version: the version has expired or does not exist
    /// (MG-REQ-0252).
    #[error("table `{table}` has no readable version `{}`", version.as_str())]
    VersionUnavailable {
        /// The table that was read.
        table: String,
        /// The version it was read at.
        version: VersionRef,
    },
}

/// A run needs write modes or features its adapter does not declare (MG-REQ-0287).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub struct CapabilityError {
    /// The requirements not declared, in the order they were needed.
    pub missing: Vec<Requirement>,
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the storage adapter does not declare ")?;
        for (index, requirement) in self.missing.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{requirement}")?;
        }
        Ok(())
    }
}
