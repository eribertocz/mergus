//! The failures of the DuckLake adapter, carried by [`AdapterError::Storage`].

use arrow::datatypes::DataType;
use mergus_adapter::{AdapterError, VersionRef, WriteMode};
use mergus_ports::PortError;

/// Why the DuckLake storage failed. Returned inside [`AdapterError::Storage`].
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum DuckLakeError {
    /// The `Database` port failed.
    #[error("DuckLake storage failed: {0}")]
    Port(#[from] PortError),
    /// The write mode is not one the adapter declares.
    #[error("the DuckLake adapter does not implement write mode `{0}`")]
    UnsupportedMode(WriteMode),
    /// A column has an Arrow type the adapter does not store.
    #[error("the DuckLake adapter does not store column `{column}` of type `{data_type}`")]
    UnsupportedType {
        /// The column.
        column: String,
        /// Its Arrow type.
        data_type: DataType,
    },
    /// The version is not a snapshot of the catalog.
    #[error("the DuckLake catalog has no snapshot `{}`", .0.as_str())]
    UnknownVersion(VersionRef),
    /// DuckDB answered a query with rows of an unexpected shape.
    #[error("DuckDB answered `{0}` with rows of an unexpected shape")]
    UnexpectedRows(String),
}

impl From<DuckLakeError> for AdapterError {
    fn from(error: DuckLakeError) -> Self {
        Self::Storage(Box::new(error))
    }
}

/// Converts a port failure into an [`AdapterError`].
pub(crate) fn port(error: PortError) -> AdapterError {
    DuckLakeError::Port(error).into()
}
