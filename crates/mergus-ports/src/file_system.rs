//! The `FileSystem` port: local files and directories.

use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use crate::PortError;

/// A file opened for streaming reads: [`Read`] and [`Seek`], sendable to another thread.
pub trait SeekRead: Read + Seek + Send {}

impl<T: Read + Seek + Send> SeekRead for T {}

/// Local files and directories.
pub trait FileSystem: Send + Sync {
    /// Returns the contents of the file at `path`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the file does not exist, and with
    /// [`PortError::Failed`] when it cannot be read.
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError>;

    /// Opens the file at `path` for reading as a stream, so that it can be read piece by piece
    /// without holding it whole in memory.
    ///
    /// The default reads the whole file with [`FileSystem::read`]; the real and simulated
    /// file systems stream it.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the file does not exist, and with
    /// [`PortError::Failed`] when it cannot be opened.
    fn open(&self, path: &Path) -> Result<Box<dyn SeekRead>, PortError> {
        Ok(Box::new(Cursor::new(self.read(path)?)))
    }

    /// Creates or replaces the file at `path` and returns a stream that writes it piece by
    /// piece. Its parent must exist. The caller flushes the stream once it has written
    /// everything; an error of the last write may be lost when it is only dropped.
    ///
    /// The default fails: a file system that does not implement it cannot write streams.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the parent directory does not exist, and with
    /// [`PortError::Failed`] when the file cannot be created.
    fn create(&self, path: &Path) -> Result<Box<dyn Write + Send>, PortError> {
        Err(PortError::Failed(format!(
            "this file system cannot create `{}` as a stream",
            path.display()
        )))
    }

    /// Creates or replaces the file at `path` with `contents`. Its parent must exist.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the parent directory does not exist, and with
    /// [`PortError::Failed`] when the file cannot be written.
    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError>;

    /// Creates the file at `path` with `contents` only if nothing exists there, in one step,
    /// so that of two callers that race for the same path one alone creates it. Its parent
    /// must exist. Returns `false`, and changes nothing, when something already exists at
    /// `path`.
    ///
    /// The default checks [`FileSystem::exists`], then writes: it is exclusive only for a
    /// file system that no other caller changes at the same time. The real file system
    /// creates the file exclusively.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the parent directory does not exist, and with
    /// [`PortError::Failed`] when the file cannot be created.
    fn create_new(&self, path: &Path, contents: &[u8]) -> Result<bool, PortError> {
        if self.exists(path)? {
            return Ok(false);
        }
        self.write(path, contents)?;
        Ok(true)
    }

    /// Removes the file at `path`.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the file does not exist, and with
    /// [`PortError::Failed`] when it cannot be removed.
    fn remove_file(&self, path: &Path) -> Result<(), PortError>;

    /// Removes the directory at `path` with everything in it.
    ///
    /// The default fails: a file system that does not implement it cannot remove directories.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the directory does not exist, and with
    /// [`PortError::Failed`] when it cannot be removed.
    fn remove_dir_all(&self, path: &Path) -> Result<(), PortError> {
        Err(PortError::Failed(format!(
            "this file system cannot remove the directory `{}`",
            path.display()
        )))
    }

    /// Moves the file at `from` to `to` in one step, replacing the file at `to` if there is
    /// one, so a reader sees either the old entry or the new one, never a part of it. Both
    /// paths must be on the same file system.
    ///
    /// The default fails: a file system that does not implement it cannot rename files.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when `from` or the parent of `to` does not exist,
    /// and with [`PortError::Failed`] when the file cannot be moved.
    fn rename(&self, from: &Path, to: &Path) -> Result<(), PortError> {
        Err(PortError::Failed(format!(
            "this file system cannot rename `{}` to `{}`",
            from.display(),
            to.display()
        )))
    }

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
