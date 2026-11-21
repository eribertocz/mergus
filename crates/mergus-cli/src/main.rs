//! The `mergus` binary; translates arguments and output only.
//!
//! The product name is defined once, as [`mergus_contracts::names::BINARY`] (MG-REQ-0207).
//! This crate derives everything named after it, such as the prefix of its environment
//! variables, in [`name`] (MG-REQ-1325). Commands, `--json`, output modes and error rendering
//! come with later issues.

#![forbid(unsafe_code)]

mod name;

fn main() {}
