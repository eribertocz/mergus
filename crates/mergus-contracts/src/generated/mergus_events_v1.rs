//! The `mergus_events` contract, version 1, generated from `schemas/mergus_events/v1/schema.json` by `cargo xtask contracts-generate`.
//! Do not edit.

use serde::{Deserialize, Serialize};

/// One event of the `--json` output of a Mergus command (`contracts/events.md`). The fields that an event type requires besides the common ones are optional here and are checked by `mergus-contracts` before the event is emitted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergusEvents {
    /// The name of the contract.
    pub contract: String,
    /// The major version of the contract.
    pub contract_version: i64,
    /// What happened.
    pub r#type: MergusEventsType,
    /// The identifier of the command run: 32 lowercase hexadecimal characters, not all zero (a W3C trace id).
    pub run_id: String,
    /// The position of the event in the run, from 1, growing by 1.
    pub sequence: i64,
    /// When the event was emitted: RFC 3339 in UTC with milliseconds and `Z`.
    pub timestamp: String,
    /// Who started the command, for audit only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Actor>,
    /// The command that runs, such as `validate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The command-line arguments after the program name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<String>>,
    /// The environment the command works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// The steps of the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<Step>>,
    /// The identifier of the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    /// The model of the step or of the test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The engine that runs the step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// How the step, the test or the run ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The number of rows the step wrote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<i64>,
    /// The number of bytes the step wrote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<i64>,
    /// How long the step or the run took, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    /// The `MG-####` code of the diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Whether the diagnostic is an error or a warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<MergusEventsSeverity>,
    /// What the diagnostic found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Where the cause of the diagnostic is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// One sentence that says what to do next.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Command-line invocations that address the diagnostic; never run automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Vec<Vec<String>>>,
    /// The name of the test or check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// The number of rows that fail the test.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failing_rows: Option<i64>,
    /// The manifest the pointer now names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_id: Option<String>,
    /// The exit code of the command (`contracts/events.md` section 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i64>,
    /// Whether running the command again may succeed, as the exit code table states.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergusEventsType {
    /// `run_started`.
    #[serde(rename = "run_started")]
    RunStarted,
    /// `plan_ready`.
    #[serde(rename = "plan_ready")]
    PlanReady,
    /// `step_started`.
    #[serde(rename = "step_started")]
    StepStarted,
    /// `step_finished`.
    #[serde(rename = "step_finished")]
    StepFinished,
    /// `diagnostic`.
    #[serde(rename = "diagnostic")]
    Diagnostic,
    /// `quality_result`.
    #[serde(rename = "quality_result")]
    QualityResult,
    /// `published`.
    #[serde(rename = "published")]
    Published,
    /// `run_finished`.
    #[serde(rename = "run_finished")]
    RunFinished,
}

/// Whether the diagnostic is an error or a warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergusEventsSeverity {
    /// `error`.
    #[serde(rename = "error")]
    Error,
    /// `warning`.
    #[serde(rename = "warning")]
    Warning,
}

/// Who started a command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    /// The identifier of the actor.
    pub id: String,
    /// What the actor is.
    pub kind: ActorKind,
    /// The identifier of the person an agent or a service acts for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<String>,
}

/// What the actor is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorKind {
    /// `person`.
    #[serde(rename = "person")]
    Person,
    /// `agent`.
    #[serde(rename = "agent")]
    Agent,
    /// `service`.
    #[serde(rename = "service")]
    Service,
}

/// One step of a plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// The model of the step.
    pub model: String,
    /// What the step does.
    pub action: String,
    /// Why the step is needed.
    pub reason: String,
}

/// A place in a project file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    /// The path of the file, relative to the project root, with `/`.
    pub file: String,
    /// The line, from 1.
    pub line: i64,
    /// The column, from 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
}

impl crate::Contract for MergusEvents {
    const NAME: &'static str = "mergus_events";
    const VERSION: u32 = 1;
    const SEMANTIC_VERSION: &'static str = "1.0.0";
    const SCHEMA: &'static str = include_str!("../../../../schemas/mergus_events/v1/schema.json");
}
