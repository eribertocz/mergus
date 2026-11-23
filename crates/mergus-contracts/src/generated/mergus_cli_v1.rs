//! The `mergus_cli` contract, version 1, generated from `schemas/mergus_cli/v1/schema.json` by `cargo xtask contracts-generate`.
//! Do not edit.

use serde::{Deserialize, Serialize};

/// The description of the Mergus command line that `mergus schema` prints: every command with its arguments, their types and allowed values, and its exit codes (MG-REQ-0263, MG-REQ-1348).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergusCli {
    /// The name of the contract.
    pub contract: String,
    /// The major version of the contract.
    pub contract_version: i64,
    /// The version of the Mergus binary that printed the document.
    pub mergus_version: String,
    /// The commands described, sorted by name.
    pub commands: Vec<Command>,
}

/// One command of the command line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    /// The name of the command as typed, its words separated by one space, such as `new model`.
    pub name: String,
    /// What the command does, in one sentence.
    pub summary: String,
    /// Every argument the command accepts.
    pub arguments: Vec<Argument>,
    /// The exit codes the command may end with (`contracts/events.md` section 2).
    pub exit_codes: Vec<i64>,
    /// Whether `--json` makes the command print the events of `mergus_events` on standard output.
    pub json: bool,
    /// Whether the command accepts `--dry-run`.
    pub dry_run: bool,
}

/// One argument of a command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Argument {
    /// The name of the argument: an option or a flag as typed, such as `--gold`; a positional argument without dashes.
    pub name: String,
    /// How the argument is given.
    pub kind: ArgumentKind,
    /// The type of the value of the argument; `boolean` for a flag.
    pub r#type: ArgumentType,
    /// Whether the argument must be given.
    pub required: bool,
    /// Whether the argument may be given more than once.
    pub repeatable: bool,
    /// The value used when the argument is not given, as a string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// The values the argument allows, when it allows only some.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

/// How the argument is given.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgumentKind {
    /// `positional`.
    #[serde(rename = "positional")]
    Positional,
    /// `option`.
    #[serde(rename = "option")]
    Option,
    /// `flag`.
    #[serde(rename = "flag")]
    Flag,
}

/// The type of the value of the argument; `boolean` for a flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgumentType {
    /// `string`.
    #[serde(rename = "string")]
    String,
    /// `integer`.
    #[serde(rename = "integer")]
    Integer,
    /// `boolean`.
    #[serde(rename = "boolean")]
    Boolean,
    /// `path`.
    #[serde(rename = "path")]
    Path,
    /// `selection`.
    #[serde(rename = "selection")]
    Selection,
}

impl crate::Contract for MergusCli {
    const NAME: &'static str = "mergus_cli";
    const VERSION: u32 = 1;
    const SEMANTIC_VERSION: &'static str = "1.0.0";
    const SCHEMA: &'static str = include_str!("../../../../schemas/mergus_cli/v1/schema.json");
}
