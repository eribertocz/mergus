//! The `mergus` binary; translates arguments and output only.
//!
//! The product name is defined once, as [`mergus_contracts::names::BINARY`] (MG-REQ-0207).
//! This crate derives everything named after it, such as the prefix of its environment
//! variables, in [`name`] (MG-REQ-1325).
//!
//! `main` wires the real ports and hands them to [`run::run`], which parses the command line
//! ([`args`]), runs the command and reports what it found: events on standard output with
//! `--json` ([`output`]), errors in the layout of [`layout`] on standard error otherwise, with
//! colors only in the interactive mode ([`mode`]). Every error carries an `MG-####` code whose
//! exit code, hint and cause come from the catalog of `errors/` ([`catalog`]).

#![forbid(unsafe_code)]

mod args;
mod catalog;
mod diagnostic;
mod error;
mod exit;
mod layout;
mod mode;
mod name;
mod output;
mod project;
mod run;
mod time;
mod validate;

use std::io::IsTerminal;
use std::process::ExitCode;

use mergus_ports::real::{RealClock, RealEnvironment, RealFileSystem, RealRandom};

fn main() -> ExitCode {
    let random = RealRandom::new();
    let ports = run::Ports {
        environment: &RealEnvironment,
        files: &RealFileSystem,
        clock: &RealClock::new(),
        random: &random,
        terminal: std::io::stdout().is_terminal(),
    };
    let code = run::run(&ports, &mut std::io::stdout(), &mut std::io::stderr());
    ExitCode::from(code)
}

#[cfg(test)]
mod tests;
