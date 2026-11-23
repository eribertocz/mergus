//! One run of the binary: arguments in, output and exit code out.

use std::io::Write;

use mergus_ports::{Clock, Environment, FileSystem, Random};

use crate::args::{self, Command};
use crate::diagnostic::Diagnostic;
use crate::error::CliError;
use crate::exit;
use crate::output::Output;
use crate::validate::validate;

/// The ports a run uses, created by `main` or by a test.
pub(crate) struct Ports<'a> {
    pub(crate) environment: &'a dyn Environment,
    pub(crate) files: &'a dyn FileSystem,
    pub(crate) clock: &'a dyn Clock,
    pub(crate) random: &'a dyn Random,
}

/// Runs the command of the arguments of `ports.environment` and returns its exit code
/// (MG-REQ-0262).
pub(crate) fn run(ports: &Ports<'_>, stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
    let arguments = ports.environment.args();
    let json = args::wants_json(&arguments);
    let mut output = Output::new(json, ports.clock, ports.random, stdout, stderr);
    match drive(ports, &arguments, &mut output) {
        Ok(code) => code,
        Err(error) => {
            output.internal(&error);
            exit::INTERNAL
        }
    }
}

/// Parses `arguments`, runs the command and reports every diagnostic. An invalid command
/// line reports its diagnostic without `run_started` (MG-REQ-0297).
fn drive(ports: &Ports<'_>, arguments: &[String], output: &mut Output<'_>) -> Result<u8, CliError> {
    let diagnostics = match args::parse(arguments) {
        Err(error) => vec![Diagnostic::of_cli(&error)],
        Ok(command) => {
            output.started(command.name(), arguments)?;
            match command {
                Command::Validate => {
                    validate(ports).unwrap_or_else(|error| vec![Diagnostic::of_cli(&error)])
                }
            }
        }
    };
    let mut code = exit::SUCCESS;
    for diagnostic in &diagnostics {
        code = code.max(output.diagnostic(diagnostic)?);
    }
    output.finished(code)?;
    Ok(code)
}
