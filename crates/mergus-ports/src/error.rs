//! The error every port returns.

/// A failure reported by a port.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    /// The file, object, source, host or program does not exist.
    #[error("`{0}` was not found")]
    NotFound(String),
    /// The operation failed.
    #[error("{0}")]
    Failed(String),
}
