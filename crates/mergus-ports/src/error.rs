//! The error every port returns.

use std::path::PathBuf;

/// A failure reported by a port.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    /// The file, object, source, host or program does not exist.
    #[error("`{0}` was not found")]
    NotFound(String),
    /// The operation failed.
    #[error("{0}")]
    Failed(String),
    /// A DuckDB extension is not in its cache directory and cannot be downloaded from its
    /// host (MG-REQ-0194.3).
    #[error(
        "the DuckDB extension `{extension}` is not in `{}` and cannot be downloaded from `{host}`",
        cache_dir.display()
    )]
    ExtensionUnavailable {
        /// The name of the extension.
        extension: String,
        /// The host it is downloaded from.
        host: String,
        /// The directory it is cached in.
        cache_dir: PathBuf,
    },
    /// A DuckLake catalog attached with the `ENCRYPTED` option exists and is not encrypted
    /// (MG-REQ-0515.3).
    #[error("the DuckLake catalog `{catalog}` is not encrypted")]
    UnencryptedCatalog {
        /// The metadata database of the catalog, as attached.
        catalog: String,
    },
    /// The connection value of a PostgreSQL source already sets a libpq parameter that
    /// Mergus sets for a protected session (MG-REQ-0429.3). The value is never shown.
    #[error(
        "the connection value of source `{source_name}` already sets `{parameter}`, which Mergus \
         sets for every source session"
    )]
    SessionParameterSet {
        /// The name the source is attached as.
        source_name: String,
        /// The parameter, `application_name` or `options`.
        parameter: String,
    },
}

impl PortError {
    /// Returns the `MG-####` code of an error the user can fix in the project
    /// (MG-REQ-0008); `None` for the others, which the caller reports with its own code.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::SessionParameterSet { .. } => Some("MG-0405"),
            Self::NotFound(_)
            | Self::Failed(_)
            | Self::ExtensionUnavailable { .. }
            | Self::UnencryptedCatalog { .. } => None,
        }
    }
}
