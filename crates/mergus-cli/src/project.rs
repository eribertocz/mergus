//! Finding the project: the nearest directory, from the current one upwards, that holds
//! `project.toml` (MG-REQ-1344).

use std::path::PathBuf;

use mergus_ports::{Environment, FileSystem};

use crate::error::CliError;

/// Returns the root of the project the command works on.
///
/// # Errors
///
/// Fails with [`CliError::ProjectNotFound`] when no directory holds `project.toml`, and with
/// [`CliError::Unreadable`] when the current directory or a directory cannot be read.
pub(crate) fn find(
    environment: &dyn Environment,
    files: &dyn FileSystem,
) -> Result<PathBuf, CliError> {
    let start = environment
        .current_dir()
        .map_err(|error| CliError::Unreadable(error.to_string()))?;
    for directory in start.ancestors() {
        let found = files
            .exists(&directory.join("project.toml"))
            .map_err(|error| CliError::Unreadable(error.to_string()))?;
        if found {
            return Ok(directory.to_path_buf());
        }
    }
    Err(CliError::ProjectNotFound {
        start: start.display().to_string(),
    })
}
