//! Run Cargo as a child process.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use crate::failure::Failure;

/// Runs `cargo <args>` in `root` with extra environment variables, inheriting standard
/// output and error so the developer sees Cargo's own messages.
///
/// # Errors
///
/// Fails if Cargo cannot start or exits with a non-zero status.
pub(crate) fn run(root: &Path, args: &[&str], envs: &[(&str, &OsString)]) -> Result<(), Failure> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let status = Command::new(&cargo)
        .args(args)
        .envs(envs.iter().copied())
        .current_dir(root)
        .status()
        .map_err(|error| Failure::new(format!("cannot run cargo: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Failure::new(format!(
            "`cargo {}` failed: {status}",
            args.join(" ")
        )))
    }
}
