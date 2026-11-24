//! The command line (MG-REQ-0297), read against the table of [`crate::commands`].

use mergus_project::closest_name;

use crate::commands::{self, Argument, Command, JSON, Kind, NAMES, QUIET, VERBOSE};
use crate::error::CliError;
use crate::mode::Verbosity;

/// The commands in the order they are suggested when nothing is closer.
const SUGGESTED: [&str; 8] = [
    "validate",
    "init",
    "new model",
    "add source",
    "schema",
    "plan",
    "ls",
    "explain",
];

/// The options every command accepts.
const GLOBAL: [&str; 3] = [JSON, QUIET, VERBOSE];

/// A parsed command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Invocation {
    /// The command to run.
    pub(crate) command: Command,
    /// How much human output to show (MG-REQ-1347.3).
    pub(crate) verbosity: Verbosity,
    /// The arguments given, by name; a flag has an empty value.
    given: Vec<(&'static str, String)>,
}

impl Invocation {
    /// Returns whether the flag `name` is given.
    pub(crate) fn flag(&self, name: &str) -> bool {
        self.value(name).is_some()
    }

    /// Returns every value given to the argument `name`, in order.
    pub(crate) fn values(&self, name: &str) -> Vec<&str> {
        self.given
            .iter()
            .filter(|(given, _)| *given == name)
            .map(|(_, value)| value.as_str())
            .collect()
    }

    /// Returns the value given to the option or positional argument `name`.
    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.given
            .iter()
            .find(|(given, _)| *given == name)
            .map(|(_, value)| value.as_str())
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
/// Fails with [`CliError`] code `MG-1301` when no command or an unknown one is given, for the
/// first argument the command does not accept, for a missing or disallowed value, for a
/// missing required argument, and for options that exclude each other (MG-REQ-1347.4).
pub(crate) fn parse(args: &[String]) -> Result<Invocation, CliError> {
    let (command, words) = find_command(args)?;
    let mut given: Vec<(&'static str, String)> = Vec::new();
    let mut rest = args
        .iter()
        .enumerate()
        .filter(|(index, _)| !words.contains(index))
        .map(|(_, argument)| argument);
    let mut positionals = Vec::new();
    while let Some(argument) = rest.next() {
        if argument.starts_with('-') {
            let (spec, value) = dashed(command, argument, &mut rest)?;
            if given.iter().any(|(name, _)| *name == spec.name) {
                if spec.kind == Kind::Flag {
                    continue;
                }
                return Err(CliError::RepeatedOption { option: spec.name });
            }
            given.push((spec.name, value));
        } else {
            positionals.push(argument.clone());
        }
    }
    for (spec, value) in assign(command, positionals)? {
        given.push((spec.name, value));
    }
    for (name, value) in &given {
        check_value(command, name, value)?;
    }
    if let Some(missing) = command
        .arguments()
        .iter()
        .find(|spec| spec.required && !given.iter().any(|(name, _)| *name == spec.name))
    {
        return Err(CliError::MissingArgument {
            argument: missing.name,
            command: command.name(),
        });
    }
    let invocation = Invocation {
        command,
        verbosity: Verbosity::Normal,
        given,
    };
    exclusive(&invocation)?;
    Ok(Invocation {
        verbosity: verbosity(invocation.flag(QUIET), invocation.flag(VERBOSE))?,
        ..invocation
    })
}

/// Finds the command: the first word, or the first two for `new model` and `add source`.
/// Returns it with the positions of its words.
fn find_command(args: &[String]) -> Result<(Command, Vec<usize>), CliError> {
    let Some(first) = args.iter().position(|argument| !argument.starts_with('-')) else {
        if let Some(unknown) = args
            .iter()
            .find(|argument| !GLOBAL.contains(&argument.as_str()))
        {
            return Err(CliError::UnknownOption {
                given: unknown.clone(),
                closest: closest(unknown, &GLOBAL),
            });
        }
        return Err(CliError::MissingCommand {
            closest: SUGGESTED[0],
        });
    };
    let word = args[first].as_str();
    if let Some(command) = commands::lookup(word) {
        return Ok((command, vec![first]));
    }
    let second = args
        .get(first + 1)
        .filter(|argument| !argument.starts_with('-'));
    if let Some(command) = second.and_then(|second| commands::lookup(&format!("{word} {second}"))) {
        return Ok((command, vec![first, first + 1]));
    }
    let closest = NAMES
        .iter()
        .find(|name| name.split(' ').next() == Some(word))
        .copied()
        .unwrap_or_else(|| closest(word, &SUGGESTED));
    Err(CliError::UnknownCommand {
        given: word.to_owned(),
        closest,
    })
}

/// Reads the option or flag `argument` of `command`, taking the value of an option from
/// `rest` when it is not written after `=`.
fn dashed<'a>(
    command: Command,
    argument: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(&'static Argument, String), CliError> {
    let (name, inline) = match argument.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (argument, None),
    };
    let dashed: Vec<&'static str> = command
        .arguments()
        .iter()
        .filter(|spec| spec.kind != Kind::Positional)
        .map(|spec| spec.name)
        .collect();
    let unknown = || CliError::UnknownOption {
        given: argument.to_owned(),
        closest: closest(argument, &dashed),
    };
    let spec = command
        .arguments()
        .iter()
        .find(|spec| spec.kind != Kind::Positional && spec.name == name)
        .ok_or_else(unknown)?;
    match (spec.kind, inline) {
        (Kind::Flag, None) => Ok((spec, String::new())),
        (Kind::Flag, Some(_)) => Err(unknown()),
        (_, Some(value)) => Ok((spec, value.to_owned())),
        (_, None) => rest
            .next()
            .map(|value| (spec, value.clone()))
            .ok_or(CliError::MissingValue { option: spec.name }),
    }
}

/// Gives each positional word to the positional arguments of `command`, in order. The words
/// after `schema` form one command name, such as `new model`.
fn assign(
    command: Command,
    mut words: Vec<String>,
) -> Result<Vec<(&'static Argument, String)>, CliError> {
    if command == Command::Schema && !words.is_empty() {
        words = vec![words.join(" ")];
    }
    let mut specs = command
        .arguments()
        .iter()
        .filter(|spec| spec.kind == Kind::Positional);
    let mut repeated: Option<&'static Argument> = None;
    words
        .into_iter()
        .map(|word| match repeated.or_else(|| specs.next()) {
            Some(spec) => {
                if spec.repeatable {
                    repeated = Some(spec);
                }
                Ok((spec, word))
            }
            None => Err(CliError::UnexpectedArgument {
                given: word,
                command: command.name(),
            }),
        })
        .collect()
}

/// Fails when `value` is not one of the values the argument `name` allows.
fn check_value(command: Command, name: &str, value: &str) -> Result<(), CliError> {
    let Some(spec) = command.arguments().iter().find(|spec| spec.name == name) else {
        return Ok(());
    };
    if spec.values.is_empty() || spec.values.contains(&value) {
        return Ok(());
    }
    Err(CliError::InvalidValue {
        argument: spec.name,
        given: value.to_owned(),
        closest: closest(value, spec.values),
    })
}

/// Fails when two arguments that exclude each other are given, or `schema` has neither a
/// command name nor `--all`.
fn exclusive(invocation: &Invocation) -> Result<(), CliError> {
    let pairs: &[(&'static str, &'static str)] = match invocation.command {
        Command::NewModel => &[("--sql", "--py")],
        Command::Schema => &[("--all", "command")],
        _ => &[],
    };
    for (first, second) in pairs {
        if invocation.flag(first) && invocation.flag(second) {
            return Err(CliError::ConflictingOptions { first, second });
        }
    }
    if invocation.command == Command::Schema
        && !invocation.flag("--all")
        && !invocation.flag("command")
    {
        return Err(CliError::MissingArgument {
            argument: "command",
            command: invocation.command.name(),
        });
    }
    Ok(())
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

/// The candidate closest to `given`, or the first one when none is close (MG-REQ-0359).
fn closest(given: &str, candidates: &[&'static str]) -> &'static str {
    let given = given.split('=').next().unwrap_or(given);
    closest_name(given, candidates.iter().copied())
        .or_else(|| candidates.first().copied())
        .unwrap_or_default()
}
