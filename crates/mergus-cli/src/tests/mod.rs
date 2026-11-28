//! Tests of the binary, run on the simulated ports of `mergus-sim`.

mod care;
mod contract;
mod diagnostics;
mod doctor;
mod explain;
mod formats;
mod human;
mod idempotency;
mod init;
mod layout;
mod plan;
mod project;
mod run;
mod scaffold;
mod schema;
mod secret;
mod selection;
mod settings;
mod sources;
mod storage;

use std::cell::RefCell;
use std::path::Path;
use std::sync::LazyLock;

use mergus_ports::real::RealSqlParser;
use mergus_ports::{Database, Keychain};
use mergus_ports::{FileSystem, Random, Timestamp};
use mergus_sim::{ClockFaults, SimClock, SimEnvironment, SimFileSystem, SimKeychain, SimRandom};
use serde_json::Value;

use crate::run::{Answers, Interpreters, Ports, SecretInput, run};

/// The root of the simulated project.
pub(crate) const ROOT: &str = "/work/sales";

/// The wall-clock time the simulated clock starts at: 2026-11-22T23:15:14.123456Z, rounded
/// down.
pub(crate) const START: i128 = 1_795_389_314_123_000_456;

/// The real SQL parser, shared by every test: `validate` checks `.sql` models with DuckDB's
/// parser itself (MG-REQ-0193).
static SQL_PARSER: LazyLock<RealSqlParser> = LazyLock::new(RealSqlParser::new);

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
    /// What the user types, when standard input is a terminal.
    pub(crate) input: Option<&'a str>,
}

impl Setup<'_> {
    /// Runs the binary on simulated ports.
    pub(crate) fn run(&self) -> Outcome {
        self.run_with_config_dir(None)
    }

    /// Runs the binary on simulated ports whose user configuration directory is
    /// `config_dir`.
    pub(crate) fn run_with_config_dir(&self, config_dir: Option<&str>) -> Outcome {
        self.run_all(config_dir, &SimKeychain::new(), &NoInput, None)
    }

    /// Runs the binary on simulated ports, `secret set` reading `stdin` and writing into
    /// `keychain`.
    pub(crate) fn run_on(&self, keychain: &dyn Keychain, stdin: &dyn SecretInput) -> Outcome {
        self.run_all(None, keychain, stdin, None)
    }

    /// Runs the binary on simulated ports, attaching database sources to `database`.
    pub(crate) fn run_with_database(&self, database: &dyn Database) -> Outcome {
        self.run_all(None, &SimKeychain::new(), &NoInput, Some(database))
    }

    fn run_all(
        &self,
        config_dir: Option<&str>,
        keychain: &dyn Keychain,
        stdin: &dyn SecretInput,
        database: Option<&dyn Database>,
    ) -> Outcome {
        let environment =
            SimEnvironment::new(self.vars.iter().copied(), config_dir.map(Into::into))
                .with_args(self.args.iter().copied())
                .with_current_dir(self.cwd);
        let clock = SimClock::new(
            self.seed,
            Timestamp::from_unix_nanos(START),
            ClockFaults::default(),
        );
        let input = self.input.map(|typed| RefCell::new(typed.as_bytes()));
        let ports = Ports {
            environment: &environment,
            files: self.files,
            clock: &clock,
            random: self.random,
            sql_parser: &*SQL_PARSER,
            terminal: self.terminal,
            input: input.as_ref().map(|typed| typed as &dyn Answers),
            secret_input: stdin,
            keychain,
            database,
            cancel: mergus_runtime::CancelToken::new(),
            python: &NoPython,
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

/// A machine without a Python interpreter: the simulated ports run no child.
pub(crate) struct NoPython;

impl Interpreters for NoPython {
    fn locate(
        &self,
        root: &Path,
        _env: Vec<(String, String)>,
    ) -> Result<Box<dyn mergus_ports::Python>, mergus_ports::PortError> {
        Err(mergus_ports::PortError::PythonNotFound {
            searched: vec![root.join(".venv/bin/python").display().to_string()],
        })
    }
}

/// Standard input that is not a terminal and holds nothing.
pub(crate) struct NoInput;

impl SecretInput for NoInput {
    fn is_terminal(&self) -> bool {
        false
    }

    fn hidden_line(&self) -> Option<String> {
        None
    }

    fn all(&self) -> Option<String> {
        Some(String::new())
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
        input: None,
        terminal: false,
    }
    .run()
}

/// Runs `mergus <args>` in `cwd` of `files`, with seed 7, standard input a terminal where the
/// user types `input` when it is given.
pub(crate) fn mergus_in(
    files: &dyn FileSystem,
    cwd: &str,
    args: &[&str],
    input: Option<&str>,
) -> Outcome {
    let random = SimRandom::new(7);
    Setup {
        files,
        cwd,
        args,
        vars: &[],
        random: &random,
        seed: 7,
        terminal: input.is_some(),
        input,
    }
    .run()
}

/// The text of the file at `path`, relative to [`ROOT`], or `None` when it does not exist.
pub(crate) fn text(files: &dyn FileSystem, path: &str) -> Option<String> {
    files
        .read(&Path::new(ROOT).join(path))
        .ok()
        .map(|bytes| String::from_utf8(bytes).unwrap())
}

/// Whether `path`, relative to [`ROOT`], exists.
pub(crate) fn exists(files: &dyn FileSystem, path: &str) -> bool {
    files.exists(&Path::new(ROOT).join(path)).unwrap()
}

/// A project with two problems of `mergus-project`: an invalid model name and an unknown
/// key of `project.toml`.
pub(crate) fn broken() -> SimFileSystem {
    project(&[
        ("project.toml", "name = \"sales\"\nnmae = \"x\"\n"),
        ("models/Orders.sql", "select 1"),
    ])
}

/// A valid project of four `.sql` models reading two sources:
/// `stg_orders` and `stg_customers` (tag `staging`) read `erp` and `crm`; `fact_sales` (tag
/// `finance`) reads both; `dim_customer` reads `stg_customers`.
pub(crate) fn shop() -> SimFileSystem {
    project(&[
        ("project.toml", PROJECT),
        (
            "sources/erp.toml",
            "kind = \"database\"\nengine = \"postgresql\"\nconnection = \"secret:erp\"\n[tables.orders]\n",
        ),
        (
            "sources/crm.toml",
            "kind = \"database\"\nengine = \"postgresql\"\nconnection = \"secret:crm\"\n[tables.customers]\n",
        ),
        (
            "models/staging/mergus.toml",
            "[all]\ntags = [\"staging\"]\n",
        ),
        (
            "models/staging/stg_orders.sql",
            "select * from raw.erp__orders\n",
        ),
        (
            "models/staging/stg_customers.sql",
            "select * from raw.crm__customers\n",
        ),
        (
            "models/gold/sales/mergus.toml",
            "[fact_sales]\ntags = [\"finance\"]\n",
        ),
        (
            "models/gold/sales/fact_sales.sql",
            "select o.id from stg_orders o join stg_customers c on o.id = c.id\n",
        ),
        (
            "models/gold/sales/dim_customer.sql",
            "select * from stg_customers\n",
        ),
    ])
}
