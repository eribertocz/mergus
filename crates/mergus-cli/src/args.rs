//! The command line (MG-REQ-0297).

use mergus_project::closest_name;

use crate::error::CliError;

/// The option that asks for events on standard output.
pub(crate) const JSON: &str = "--json";

/// Every command, in the order they are suggested.
const COMMANDS: [&str; 1] = ["validate"];

/// Every option.
const OPTIONS: [&str; 1] = [JSON];

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

/// Returns whether `--json` is among `args`.
pub(crate) fn wants_json(args: &[String]) -> bool {
    args.iter().any(|argument| argument == JSON)
}

/// Parses the arguments after the program name.
///
/// # Errors
///
/// Fails with the first unknown option, unknown command or unexpected argument, and when no
/// command is given.
pub(crate) fn parse(args: &[String]) -> Result<Command, CliError> {
    let mut command = None;
    for argument in args {
        if argument == JSON {
            continue;
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
    command.ok_or(CliError::MissingCommand {
        closest: COMMANDS[0],
    })
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
