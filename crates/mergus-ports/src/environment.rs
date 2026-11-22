//! The `Environment` port: command-line arguments, current directory, environment variables
//! and the user configuration directory.

use std::path::PathBuf;

use crate::PortError;

/// Command-line arguments, current directory, environment variables and the user
/// configuration directory.
pub trait Environment: Send + Sync {
    /// Returns the command-line arguments after the program name, in order. An argument that
    /// is not valid Unicode has each invalid sequence replaced by `U+FFFD`.
    fn args(&self) -> Vec<String>;

    /// Returns the current directory.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the current directory does not exist, and with
    /// [`PortError::Failed`] when it cannot be known.
    fn current_dir(&self) -> Result<PathBuf, PortError>;

    /// Returns the value of the environment variable `name`, or `None` when it is not set or
    /// is not valid Unicode.
    fn var(&self, name: &str) -> Option<String>;

    /// Returns the user configuration directory, or `None` when there is none.
    fn config_dir(&self) -> Option<PathBuf>;
}
