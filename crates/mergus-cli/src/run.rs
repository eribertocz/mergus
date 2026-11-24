//! One run of the binary: arguments in, output and exit code out.

use std::cell::RefCell;
use std::io::{BufRead, Write};

use mergus_ports::{Clock, Environment, FileSystem, Random, SqlParser};

use crate::args::{self, Invocation};
use crate::cli_schema;
use crate::commands::{Command, DRY_RUN, EXCLUDE, SELECTION};
use crate::diagnostic::{Diagnostic, group};
use crate::error::CliError;
use crate::exit;
use crate::explain::explain;
use crate::init::{init, next_steps};
use crate::mode::{colors, detect};
use crate::models;
use crate::output::Output;
use crate::planning::plan;
use crate::scaffold::{add_source, new_model};
use crate::selection::{listing, select};
use crate::validate::validate;

/// What the user types on a terminal, one line at a time.
pub(crate) trait Answers {
    /// The next line typed, without its line ending; `None` at the end of the input or when
    /// it cannot be read.
    fn line(&self) -> Option<String>;
}

impl<R: BufRead> Answers for RefCell<R> {
    fn line(&self) -> Option<String> {
        let mut line = String::new();
        match self.borrow_mut().read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\n', '\r']).to_owned()),
        }
    }
}

/// The ports a run uses, created by `main` or by a test.
pub(crate) struct Ports<'a> {
    pub(crate) environment: &'a dyn Environment,
    pub(crate) files: &'a dyn FileSystem,
    pub(crate) clock: &'a dyn Clock,
    pub(crate) random: &'a dyn Random,
    /// DuckDB's SQL parser, which `validate` checks `.sql` models with (MG-REQ-0193).
    pub(crate) sql_parser: &'a dyn SqlParser,
    /// Whether standard output is a terminal (MG-REQ-1321).
    pub(crate) terminal: bool,
    /// Standard input when it is a terminal, which `init` asks its questions on
    /// (MG-REQ-1349.1).
    pub(crate) input: Option<&'a dyn Answers>,
}

/// Runs the command of the arguments of `ports.environment` and returns its exit code
/// (MG-REQ-0262).
pub(crate) fn run(ports: &Ports<'_>, stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
    let arguments = ports.environment.args();
    let mode = detect(
        args::wants_json(&arguments),
        ports.terminal,
        ports.environment,
    );
    let color = colors(mode, ports.environment);
    let mut output = Output::new(mode, color, ports.clock, ports.random, stdout, stderr);
    match drive(ports, &arguments, &mut output) {
        Ok(code) => code,
        Err(error) => {
            output.internal(&error);
            exit::INTERNAL
        }
    }
}

/// Parses `arguments`, runs the command and reports every diagnostic, grouped by file
/// (MG-REQ-1323). An invalid command line reports its diagnostic without `run_started`
/// (MG-REQ-0297). `schema` prints its document alone (MG-REQ-1348).
fn drive(ports: &Ports<'_>, arguments: &[String], output: &mut Output<'_>) -> Result<u8, CliError> {
    let diagnostics = match args::parse(arguments) {
        Err(error) => vec![Diagnostic::of_cli(&error)],
        Ok(invocation) if invocation.command == Command::Schema => {
            output.document(&cli_schema::document(&invocation)?)?;
            return Ok(exit::SUCCESS);
        }
        Ok(invocation) => {
            output.set_verbosity(invocation.verbosity);
            output.started(invocation.command.name(), arguments)?;
            execute(ports, &invocation, output)
                .unwrap_or_else(|error| vec![Diagnostic::of_cli(&error)])
        }
    };
    let mut code = exit::SUCCESS;
    for diagnostic in &group(diagnostics) {
        code = code.max(output.diagnostic(diagnostic)?);
    }
    output.finished(code)?;
    Ok(code)
}

/// Runs the command of `invocation` and returns the diagnostics it found.
fn execute(
    ports: &Ports<'_>,
    invocation: &Invocation,
    output: &mut Output<'_>,
) -> Result<Vec<Diagnostic>, CliError> {
    let plan = match invocation.command {
        Command::Validate => {
            let begin = ports.clock.monotonic().elapsed();
            let diagnostics =
                validate(ports).unwrap_or_else(|error| vec![Diagnostic::of_cli(&error)]);
            let end = ports.clock.monotonic().elapsed();
            output.stage(1, end.saturating_sub(begin))?;
            return Ok(diagnostics);
        }
        Command::Ls => return ls(ports, invocation, output),
        Command::Plan => return plan(ports, invocation, output),
        Command::Explain => {
            let argument = invocation.value("argument").unwrap_or_default();
            return match explain(ports, argument)? {
                Ok(text) => output.report(&text).map(|()| Vec::new()),
                Err(diagnostics) => Ok(diagnostics),
            };
        }
        Command::Init => init(ports, invocation, output)?,
        Command::NewModel => new_model(ports, invocation)?,
        Command::AddSource => add_source(ports, invocation)?,
        Command::Schema => {
            return Err(CliError::Internal(
                "`schema` prints its document alone".to_owned(),
            ));
        }
    };
    if invocation.flag(DRY_RUN) {
        output.report(&plan.listing(ports.files)?)?;
        return Ok(Vec::new());
    }
    plan.apply(ports.files)?;
    if invocation.command == Command::Init {
        output.report(&next_steps())?;
    }
    Ok(Vec::new())
}

/// `mergus ls [<selection>]`: every selected model and why (MG-REQ-1305).
fn ls(
    ports: &Ports<'_>,
    invocation: &Invocation,
    output: &mut Output<'_>,
) -> Result<Vec<Diagnostic>, CliError> {
    let project = match models::read(ports)? {
        Ok(project) => project,
        Err(diagnostics) => return Ok(diagnostics),
    };
    let selected = select(
        &project,
        &invocation.values(SELECTION),
        invocation.value(EXCLUDE),
    )?;
    output.report(&listing(&selected))?;
    Ok(Vec::new())
}
