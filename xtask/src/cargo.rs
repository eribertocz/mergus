//! Run Cargo as a child process.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::failure::Failure;
use crate::workspace;

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

/// Runs `cargo <args>` in `dir` and returns its standard output. Standard error is
/// inherited, so the developer sees Cargo's own messages.
///
/// # Errors
///
/// Fails if Cargo cannot start, exits with a non-zero status or prints non-UTF-8 output.
pub(crate) fn output(dir: &Path, args: &[&str]) -> Result<String, Failure> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output = Command::new(&cargo)
        .args(args)
        .current_dir(dir)
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| Failure::new(format!("cannot run cargo: {error}")))?;
    if !output.status.success() {
        return Err(Failure::new(format!(
            "`cargo {}` failed in {}: {}",
            args.join(" "),
            workspace::shown(dir),
            output.status
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| Failure::new(format!("`cargo {}` printed non-UTF-8", args.join(" "))))
}
