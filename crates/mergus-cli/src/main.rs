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
//!
//! The commands and their arguments are defined once, in [`commands`]: the parser reads
//! them and `mergus schema` prints them ([`cli_schema`]). `init` ([`init`]), `new model` and
//! `add source` ([`scaffold`]) build a plan of files ([`files`]) that is checked before
//! anything is written, and that `--dry-run` lists instead.
//!
//! `ls` and `plan` read the project ([`models`]) and select models ([`selection`]); `plan`
//! resolves its settings ([`settings`]) and writes the plan of `mergus-plan` ([`planning`]).
//! `explain` prints an error code's catalog file or a model's options ([`explain`]).
//! `run` and `backfill` drive the run cycle of `mergus-runtime` into candidates in work
//! ([`execution`], [`work`]). A command that changes state, given `--idempotency-key`, runs
//! once per key and arguments ([`idempotency`]).
//! `doctor` shows which link of the resolution chain answers for each secret reference of the
//! sources, never its value ([`doctor`]).

#![forbid(unsafe_code)]

mod args;
mod arrow_types;
mod care;
mod catalog;
mod cli_schema;
mod commands;
mod diagnostic;
mod doctor;
mod error;
mod execution;
mod exit;
mod explain;
mod files;
mod idempotency;
mod init;
mod layout;
mod limits;
mod mode;
mod models;
mod name;
mod output;
mod planning;
mod project;
mod run;
mod scaffold;
mod secret;
mod selection;
mod settings;
mod step_events;
mod time;
mod validate;
mod work;

use std::cell::RefCell;
use std::io::{IsTerminal, Read, StdinLock};
use std::process::ExitCode;

use mergus_ports::real::{
    RealClock, RealDatabase, RealEnvironment, RealFileSystem, RealKeychain, RealRandom,
    RealSqlParser,
};
use mergus_ports::{Database, Environment};
use mergus_runtime::CancelToken;

/// Standard input, read through the lock `main` holds (MG-REQ-1007).
struct Stdin<'a>(&'a RefCell<StdinLock<'static>>);

impl run::SecretInput for Stdin<'_> {
    fn is_terminal(&self) -> bool {
        std::io::stdin().is_terminal()
    }

    /// Reads the line from the terminal with echo turned off.
    fn hidden_line(&self) -> Option<String> {
        rpassword::read_password().ok()
    }

    fn all(&self) -> Option<String> {
        let mut value = String::new();
        self.0.borrow_mut().read_to_string(&mut value).ok()?;
        Some(value)
    }
}

fn main() -> ExitCode {
    let random = RealRandom::new();
    // MG-REQ-0710.4: Ctrl-C sets the cancel token; a second Ctrl-C sets it again, so the run
    // still ends through its finalizer. Without a handler, Ctrl-C keeps its default.
    let cancel = CancelToken::new();
    let token = cancel.clone();
    ctrlc::set_handler(move || token.cancel()).ok();
    let stdin = RefCell::new(std::io::stdin().lock());
    let database = RealEnvironment
        .cache_dir()
        .map(|cache_dir| RealDatabase::new(&cache_dir));
    let ports = run::Ports {
        environment: &RealEnvironment,
        files: &RealFileSystem,
        clock: &RealClock::new(),
        random: &random,
        sql_parser: &RealSqlParser::new(),
        terminal: std::io::stdout().is_terminal(),
        input: std::io::stdin()
            .is_terminal()
            .then_some(&stdin as &dyn run::Answers),
        secret_input: &Stdin(&stdin),
        keychain: &RealKeychain,
        database: database.as_ref().map(|database| database as &dyn Database),
        cancel,
    };
    let code = run::run(&ports, &mut std::io::stdout(), &mut std::io::stderr());
    ExitCode::from(code)
}

#[cfg(test)]
mod tests;
