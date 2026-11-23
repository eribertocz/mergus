//! `mergus schema`: the `mergus_cli` document of one command or of every command
//! (MG-REQ-0263, MG-REQ-1348).

use mergus_contracts::generated::mergus_cli_v1::{
    Argument, ArgumentKind, ArgumentType, Command as Described, MergusCli,
};
use mergus_contracts::{Contract, ValidDocument, validate};

use crate::args::Invocation;
use crate::commands::{self, ALL, Command, Kind, Type};
use crate::error::CliError;

/// The document `mergus schema` prints: the command it names, or every command with
/// `--all`, sorted by name.
///
/// # Errors
///
/// Fails with [`CliError::Internal`] when the document does not match its contract.
pub(crate) fn document(invocation: &Invocation) -> Result<ValidDocument, CliError> {
    let chosen: Vec<Command> = match invocation.value("command").and_then(commands::lookup) {
        Some(command) => vec![command],
        None => ALL.to_vec(),
    };
    let document = MergusCli {
        contract: MergusCli::NAME.to_owned(),
        contract_version: i64::from(MergusCli::VERSION),
        mergus_version: env!("CARGO_PKG_VERSION").to_owned(),
        commands: chosen.into_iter().map(describe).collect(),
    };
    validate(&document).map_err(|error| CliError::Internal(error.to_string()))
}

fn describe(command: Command) -> Described {
    Described {
        name: command.name().to_owned(),
        summary: command.summary().to_owned(),
        arguments: command
            .arguments()
            .iter()
            .map(|argument| Argument {
                name: argument.name.to_owned(),
                kind: match argument.kind {
                    Kind::Positional => ArgumentKind::Positional,
                    Kind::Option => ArgumentKind::Option,
                    Kind::Flag => ArgumentKind::Flag,
                },
                r#type: match argument.r#type {
                    Type::String => ArgumentType::String,
                    Type::Boolean => ArgumentType::Boolean,
                },
                required: argument.required,
                repeatable: false,
                default: argument.default.map(str::to_owned),
                values: (!argument.values.is_empty()).then(|| {
                    argument
                        .values
                        .iter()
                        .map(|value| (*value).to_owned())
                        .collect()
                }),
            })
            .collect(),
        exit_codes: command
            .exit_codes()
            .iter()
            .copied()
            .map(i64::from)
            .collect(),
        json: command.json(),
        dry_run: command.dry_run(),
    }
}
