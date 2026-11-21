//! The `Environment` port: environment variables and the user configuration directory.

use std::path::PathBuf;

/// Environment variables and the user configuration directory.
pub trait Environment: Send + Sync {
    /// Returns the value of the environment variable `name`, or `None` when it is not set or
    /// is not valid Unicode.
    fn var(&self, name: &str) -> Option<String>;

    /// Returns the user configuration directory, or `None` when there is none.
    fn config_dir(&self) -> Option<PathBuf>;
}
