//! The failures of writing raw.

use arrow::error::ArrowError;
use mergus_adapter::AdapterError;

/// Why raw could not be opened or written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RawError {
    /// A `local` or `shared` raw catalog lies inside the project root (MG-REQ-0515.2).
    #[error(
        "the raw catalog `{path}` lies inside the project folder; it holds the encryption keys \
         of raw, so place it in a directory outside the project, such as `{suggestion}`"
    )]
    CatalogInsideProject {
        /// The path of the catalog, as written.
        path: String,
        /// A path outside the project the catalog could take.
        suggestion: String,
    },
    /// The raw catalog exists and is not encrypted (MG-REQ-0515.3).
    #[error("the raw catalog `{catalog}` is not encrypted; Mergus never writes raw unencrypted")]
    UnencryptedCatalog {
        /// The catalog: its file, or the secret reference of its connection.
        catalog: String,
    },
    /// A load returned the same key twice (MG-REQ-0518.3).
    #[error(
        "table `{table}` of source `{source_name}` returned the key `{key}` twice; nothing was \
         written to raw"
    )]
    RepeatedKey {
        /// The source.
        source_name: String,
        /// The declared table.
        table: String,
        /// The repeated key, as `column = value`, one pair per key column.
        key: String,
    },
    /// A `cloud` raw catalog was opened without the value of its secret.
    #[error("the raw catalog `{secret}` was opened without the value of its secret")]
    MissingConnection {
        /// The secret reference of the catalog's connection.
        secret: String,
    },
    /// The rows of a load do not fit its schema or its raw tables.
    #[error("the rows of table `{table}` cannot be written to raw: {error}")]
    Rows {
        /// The declared table.
        table: String,
        /// What Arrow reported.
        error: ArrowError,
    },
    /// The storage adapter failed.
    #[error("raw storage failed: {0}")]
    Storage(#[from] AdapterError),
    /// The load registry `ops.loads` could not be written or read (MG-REQ-0514).
    #[error("the load registry `ops.loads` failed: {0}")]
    Registry(AdapterError),
}

impl RawError {
    /// Returns the `MG-####` code of the error (MG-REQ-0008), or `None` for a failure of the
    /// caller or of the storage, which carries its own cause.
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::CatalogInsideProject { .. } => Some("MG-0502"),
            Self::UnencryptedCatalog { .. } => Some("MG-0503"),
            Self::RepeatedKey { .. } => Some("MG-0504"),
            Self::MissingConnection { .. }
            | Self::Rows { .. }
            | Self::Storage(_)
            | Self::Registry(_) => None,
        }
    }
}
