//! The `Process` port: child processes.

use std::path::PathBuf;

use crate::PortError;

/// A child process to run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessCommand {
    /// The program, found on the search path when it is not a path.
    pub program: String,
    /// The arguments.
    pub args: Vec<String>,
    /// The working directory, or `None` for the current one.
    pub current_dir: Option<PathBuf>,
    /// Environment variables set for the child, in order; a later one replaces an earlier one
    /// of the same name. With `clear_env`, they are the child's whole environment; without
    /// it, they are set over the variables the child inherits from this process.
    pub env: Vec<(String, String)>,
    /// When `true`, the child inherits no variable from this process and starts with only
    /// `env`; when `false`, it inherits them all, then `env` is set over them.
    pub clear_env: bool,
}

/// What a finished child process returned.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessOutput {
    /// The exit code, or `None` when a signal ended the process.
    pub code: Option<i32>,
    /// What it wrote to standard output.
    pub stdout: Vec<u8>,
    /// What it wrote to standard error.
    pub stderr: Vec<u8>,
}

/// Child processes.
pub trait Process: Send + Sync {
    /// Runs `command` to completion and returns its output.
    ///
    /// # Errors
    ///
    /// Fails with [`PortError::NotFound`] when the program does not exist, and with
    /// [`PortError::Failed`] when it cannot be started.
    fn run(&self, command: &ProcessCommand) -> Result<ProcessOutput, PortError>;
}
