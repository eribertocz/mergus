//! The `ObjectStore` port: S3-compatible and local object storage.

use crate::PortError;

/// S3-compatible and local object storage, addressed by key.
pub trait ObjectStore: Send + Sync {
    /// Returns the bytes of the object `key`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the object does not exist, and with
    /// [`PortError::Failed`] when the store fails.
    fn get(&self, key: &str) -> Result<Vec<u8>, PortError>;

    /// Creates or replaces the object `key` with `bytes`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the store fails.
    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), PortError>;

    /// Deletes the object `key`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the object does not exist, and with
    /// [`PortError::Failed`] when the store fails.
    fn delete(&self, key: &str) -> Result<(), PortError>;

    /// Returns the keys that start with `prefix`, sorted.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the store fails.
    fn list(&self, prefix: &str) -> Result<Vec<String>, PortError>;
}
