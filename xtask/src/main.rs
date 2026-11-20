//! Repository automation for Mergus, run as `cargo xtask <command>`.
//!
//! Each command runs one check of the specification and exits with a non-zero status when
//! the check fails:
//!
//! - `clippy`: every member inherits the workspace lints, and Clippy passes with
//!   `-D warnings` (MG-REQ-0009, MG-REQ-0171, MG-REQ-0172).
//! - `unsafe`: `unsafe` appears only in FFI crates, behind `// SAFETY:` comments
//!   (MG-REQ-0170).
//! - `missing-docs`: every member inherits the workspace lints, and the documentation builds
//!   with warnings denied (MG-REQ-0175).
//! - `reproducible-build`: two release builds produce identical binaries (MG-REQ-0113).
//! - `check-boundaries`: the crates respect the boundaries that `cargo metadata` shows
//!   (MG-REQ-0126).

#![forbid(unsafe_code)]

mod boundaries;
mod cargo;
mod failure;
mod json;
mod lints;
mod metadata;
mod reproducible;
mod unsafe_code;
mod workspace;

use std::ffi::OsString;
use std::path::Path;

use failure::Failure;

const USAGE: &str =
    "usage: cargo xtask <clippy|unsafe|missing-docs|reproducible-build|check-boundaries>";

fn main() -> Result<(), Failure> {
    let root = workspace::root()?;
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        "clippy" => clippy(&root),
        "unsafe" => unsafe_code::check(&workspace::members(&root)?),
        "missing-docs" => missing_docs(&root),
        "reproducible-build" => reproducible::check(&root),
        "check-boundaries" => boundaries::check(&root),
        _ => Err(Failure::new(USAGE)),
    }
}

fn clippy(root: &Path) -> Result<(), Failure> {
    lints::check("clippy", &workspace::members(root)?)?;
    let args = [
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ];
    cargo::run(root, &args, &[])
}

fn missing_docs(root: &Path) -> Result<(), Failure> {
    lints::check("missing-docs", &workspace::members(root)?)?;
    let flags = OsString::from("-D warnings");
    let args = ["doc", "--workspace", "--no-deps", "--locked"];
    cargo::run(root, &args, &[("RUSTDOCFLAGS", &flags)])
}
