//! The command line (MG-REQ-0297).

use mergus_project::closest_name;

use crate::error::CliError;
use crate::mode::Verbosity;

/// The option that asks for events on standard output.
pub(crate) const JSON: &str = "--json";

/// The option that shows only errors.
const QUIET: &str = "--quiet";

/// The option that adds a line per stage.
const VERBOSE: &str = "--verbose";

/// Every command, in the order they are suggested.
const COMMANDS: [&str; 1] = ["validate"];

/// Every option.
const OPTIONS: [&str; 3] = [JSON, QUIET, VERBOSE];

/// A command the binary runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    /// `mergus validate` (MG-REQ-1345).
    Validate,
}

impl Command {
    /// The name of the command, as typed.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Validate => "validate",
        }
    }
}

/// A parsed command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Invocation {
    /// The command to run.
    pub(crate) command: Command,
    /// How much human output to show (MG-REQ-1347.3).
    pub(crate) verbosity: Verbosity,
}

/// Returns whether `--json` is among `args`.
pub(crate) fn wants_json(args: &[String]) -> bool {
    args.iter().any(|argument| argument == JSON)
}

/// Parses the arguments after the program name.
///
/// # Errors
///
/// Fails with the first unknown option, unknown command or unexpected argument, when no
/// command is given, and when `--quiet` and `--verbose` are both given (MG-REQ-1347.4).
pub(crate) fn parse(args: &[String]) -> Result<Invocation, CliError> {
    let mut command = None;
    let (mut quiet, mut verbose) = (false, false);
    for argument in args {
        match argument.as_str() {
            JSON => continue,
            QUIET => {
                quiet = true;
                continue;
            }
            VERBOSE => {
                verbose = true;
                continue;
            }
            _ => {}
        }
        if argument.starts_with('-') {
            return Err(CliError::UnknownOption {
                given: argument.clone(),
                closest: closest(argument, &OPTIONS),
            });
        }
        match command {
            Some(command) => {
                return Err(CliError::UnexpectedArgument {
                    given: argument.clone(),
                    command: Command::name(command),
                });
            }
            None => command = Some(lookup(argument)?),
        }
    }
    let command = command.ok_or(CliError::MissingCommand {
        closest: COMMANDS[0],
    })?;
    Ok(Invocation {
        command,
        verbosity: verbosity(quiet, verbose)?,
    })
}

/// The verbosity of `--quiet` and `--verbose`, which exclude each other (MG-REQ-1347.4).
fn verbosity(quiet: bool, verbose: bool) -> Result<Verbosity, CliError> {
    match (quiet, verbose) {
        (true, true) => Err(CliError::ConflictingOptions {
            first: QUIET,
            second: VERBOSE,
        }),
        (true, false) => Ok(Verbosity::Quiet),
        (false, true) => Ok(Verbosity::Verbose),
        (false, false) => Ok(Verbosity::Normal),
    }
}

fn lookup(word: &str) -> Result<Command, CliError> {
    match word {
        "validate" => Ok(Command::Validate),
        _ => Err(CliError::UnknownCommand {
            given: word.to_owned(),
            closest: closest(word, &COMMANDS),
        }),
    }
}

/// The candidate closest to `given`, or the first one when none is close (MG-REQ-0359).
fn closest(given: &str, candidates: &[&'static str]) -> &'static str {
    let given = given.split('=').next().unwrap_or(given);
    closest_name(given, candidates.iter().copied())
        .or_else(|| candidates.first().copied())
        .unwrap_or_default()
}
