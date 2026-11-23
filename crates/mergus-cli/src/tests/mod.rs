//! Tests of the binary, run on the simulated ports of `mergus-sim`.

mod contract;
mod diagnostics;
mod formats;
mod human;
mod layout;
mod project;

use std::path::Path;

use mergus_ports::{FileSystem, Random, Timestamp};
use mergus_sim::{ClockFaults, SimClock, SimEnvironment, SimFileSystem, SimRandom};
use serde_json::Value;

use crate::run::{Ports, run};

/// The root of the simulated project.
pub(crate) const ROOT: &str = "/work/sales";

/// The wall-clock time the simulated clock starts at: 2026-11-22T23:15:14.123456Z, rounded
/// down.
pub(crate) const START: i128 = 1_795_389_314_123_000_456;

/// A valid `project.toml`.
pub(crate) const PROJECT: &str = "name = \"sales\"\n";

/// What one run of the binary printed and returned.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit: u8,
}

impl Outcome {
    /// Every line of standard output, parsed as JSON.
    pub(crate) fn events(&self) -> Vec<Value> {
        self.stdout
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    /// The events of type `kind`.
    pub(crate) fn of(&self, kind: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|event| event["type"] == kind)
            .collect()
    }
}

/// Returns a simulated file system holding `entries`, given relative to [`ROOT`].
pub(crate) fn project(entries: &[(&str, &str)]) -> SimFileSystem {
    let files = SimFileSystem::new();
    files.create_dir_all(Path::new(ROOT)).unwrap();
    for (path, contents) in entries {
        let path = Path::new(ROOT).join(path);
        files.create_dir_all(path.parent().unwrap()).unwrap();
        files.write(&path, contents.as_bytes()).unwrap();
    }
    files
}

/// How to run the binary.
pub(crate) struct Setup<'a> {
    pub(crate) files: &'a dyn FileSystem,
    pub(crate) cwd: &'a str,
    pub(crate) args: &'a [&'a str],
    pub(crate) vars: &'a [(&'a str, &'a str)],
    pub(crate) random: &'a dyn Random,
    pub(crate) seed: u64,
    /// Whether standard output is a terminal.
    pub(crate) terminal: bool,
}

impl Setup<'_> {
    /// Runs the binary on simulated ports.
    pub(crate) fn run(&self) -> Outcome {
        let environment = SimEnvironment::new(self.vars.iter().copied(), None)
            .with_args(self.args.iter().copied())
            .with_current_dir(self.cwd);
        let clock = SimClock::new(
            self.seed,
            Timestamp::from_unix_nanos(START),
            ClockFaults::default(),
        );
        let ports = Ports {
            environment: &environment,
            files: self.files,
            clock: &clock,
            random: self.random,
            terminal: self.terminal,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit = run(&ports, &mut stdout, &mut stderr);
        Outcome {
            stdout: String::from_utf8(stdout).unwrap(),
            stderr: String::from_utf8(stderr).unwrap(),
            exit,
        }
    }
}

/// Runs `mergus <args>` in [`ROOT`] of `files`, with seed 7.
pub(crate) fn mergus(files: &dyn FileSystem, args: &[&str]) -> Outcome {
    let random = SimRandom::new(7);
    Setup {
        files,
        cwd: ROOT,
        args,
        vars: &[],
        random: &random,
        seed: 7,
        terminal: false,
    }
    .run()
}

/// A project with two problems of `mergus-project`: an invalid model name and an unknown
/// key of `project.toml`.
pub(crate) fn broken() -> SimFileSystem {
    project(&[
        ("project.toml", "name = \"sales\"\nnmae = \"x\"\n"),
        ("models/Orders.sql", "select 1"),
    ])
}
