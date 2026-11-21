//! The `FileSystem` port: local files and directories.

use std::path::{Path, PathBuf};

use crate::PortError;

/// Local files and directories.
pub trait FileSystem: Send + Sync {
    /// Returns the contents of the file at `path`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the file does not exist, and with
    /// [`PortError::Failed`] when it cannot be read.
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError>;

    /// Creates or replaces the file at `path` with `contents`. Its parent must exist.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the parent directory does not exist, and with
    /// [`PortError::Failed`] when the file cannot be written.
    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError>;

    /// Removes the file at `path`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the file does not exist, and with
    /// [`PortError::Failed`] when it cannot be removed.
    fn remove_file(&self, path: &Path) -> Result<(), PortError>;

    /// Creates the directory at `path` and every missing parent.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when a directory cannot be created.
    fn create_dir_all(&self, path: &Path) -> Result<(), PortError>;

    /// Returns the paths of the entries of the directory at `path`, sorted by component.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the directory does not exist, and with
    /// [`PortError::Failed`] when it cannot be read.
    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, PortError>;

    /// Returns whether a file or directory exists at `path`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::Failed`] when the answer cannot be known.
    fn exists(&self, path: &Path) -> Result<bool, PortError>;
}
