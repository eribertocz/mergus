//! The `mergus_plan` contract, version 1, generated from `schemas/mergus_plan/v1/schema.json` by `cargo xtask contracts-generate`.
//! Do not edit.

use serde::{Deserialize, Serialize};

/// The plan: what a run would compute or skip for the selected models, each decision with its reason, identified by its hash (MG-REQ-0714, MG-REQ-0742). Every list is sorted by byte order unless stated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergusPlan {
    /// The name of the contract.
    pub contract: String,
    /// The major version of the contract.
    pub contract_version: i64,
    /// The lowercase hexadecimal SHA-256 of the canonical JSON of the plan without `plan_id` (MG-REQ-0743).
    pub plan_id: String,
    /// The version of Mergus that made the plan.
    pub mergus_version: String,
    /// The environment the plan is for (MG-REQ-0296.5).
    pub environment: String,
    /// The most steps of one parallel group (MG-REQ-0744.1).
    pub max_parallel: i64,
    /// The names of the selected models.
    pub selected: Vec<String>,
    /// One step per selected model, sorted by parallel group and then by model name (MG-REQ-0744.3).
    pub steps: Vec<Step>,
    /// The models left out of the selection, upstream of a selected model, whose result is stale (MG-REQ-0745), sorted by model.
    pub stale_upstream: Vec<Stale>,
    /// The gates of stage 4, in the order of the gate table of `run/gates.md`, each with its status (MG-REQ-0747.3, MG-REQ-0720, MG-REQ-0722). Absent before phase 0.4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gates: Option<Vec<Gate>>,
}

/// The decision for one selected model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// The model name.
    pub model: String,
    /// The stable identifier of the model (MG-REQ-0368).
    pub model_id: String,
    /// The lowercase hexadecimal SHA-256 of the canonical JSON of what the model's result depends on (MG-REQ-0711, MG-REQ-0743).
    pub fingerprint: String,
    /// The tables the model reads: lowercase, a model bare, a raw table as `raw.<table>`.
    pub inputs: Vec<String>,
    /// Whether the model is computed or its available result reused (MG-REQ-0744.2).
    pub action: StepAction,
    /// Why: `result_available` for `skip`, `no_result` for `compute` (MG-REQ-0744.2).
    pub reason: StepReason,
    /// What is computed: the whole result.
    pub refresh: StepRefresh,
    /// The parallel group of the step, from 1 (MG-REQ-0744.3).
    pub parallel_group: i64,
    /// How an incremental step recomputes its result; when present, `refresh` describes nothing and readers use it (MG-REQ-0747.4, MG-REQ-0652, MG-REQ-0653).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incremental: Option<Incremental>,
    /// The segments of the model in execution order (MG-REQ-0747.1, MG-REQ-0631).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<Segment>>,
    /// The boundaries between segments, sorted by `from` and then by `to` (MG-REQ-0747.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundaries: Option<Vec<Boundary>>,
    /// The periods the model recomputes, sorted by `start`; present only when `incremental.mode` is `period`, and empty when nothing is recomputed (MG-REQ-0747.2, MG-REQ-0652).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub periods: Option<Vec<Period>>,
}

/// Whether the model is computed or its available result reused (MG-REQ-0744.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepAction {
    /// `compute`.
    #[serde(rename = "compute")]
    Compute,
    /// `skip`.
    #[serde(rename = "skip")]
    Skip,
}

/// Why: `result_available` for `skip`, `no_result` for `compute` (MG-REQ-0744.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepReason {
    /// `no_result`.
    #[serde(rename = "no_result")]
    NoResult,
    /// `result_available`.
    #[serde(rename = "result_available")]
    ResultAvailable,
}

/// What is computed: the whole result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepRefresh {
    /// `full`.
    #[serde(rename = "full")]
    Full,
}

/// A model left out of the selection whose result is stale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stale {
    /// The stale model.
    pub model: String,
    /// The selector that includes it: `+<model>` of the first selected model downstream of it, in name order.
    pub include: String,
}

/// The incremental refresh of a step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incremental {
    /// What is recomputed: `period` the periods of `periods` (MG-REQ-0652), `key` the rows of changed keys (MG-REQ-0653).
    pub mode: IncrementalMode,
}

/// What is recomputed: `period` the periods of `periods` (MG-REQ-0652), `key` the rows of changed keys (MG-REQ-0653).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncrementalMode {
    /// `period`.
    #[serde(rename = "period")]
    Period,
    /// `key`.
    #[serde(rename = "key")]
    Key,
}

/// A run of the model's steps on one engine (MG-REQ-0631).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    /// The position of the segment in execution order, from 0.
    pub index: i64,
    /// The engine that runs the segment.
    pub engine: SegmentEngine,
    /// The names of the model's steps the segment runs, in order.
    pub steps: Vec<String>,
    /// How Polars runs the segment; present only when `engine` is `polars` (MG-REQ-0635).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polars_level: Option<SegmentPolarsLevel>,
    /// Why that level was chosen; present only when `engine` is `polars` (MG-REQ-0635).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polars_reason: Option<SegmentPolarsReason>,
}

/// The engine that runs the segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentEngine {
    /// `duckdb`.
    #[serde(rename = "duckdb")]
    Duckdb,
    /// `polars`.
    #[serde(rename = "polars")]
    Polars,
}

/// How Polars runs the segment; present only when `engine` is `polars` (MG-REQ-0635).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentPolarsLevel {
    /// `native`.
    #[serde(rename = "native")]
    Native,
    /// `in_process`.
    #[serde(rename = "in_process")]
    InProcess,
    /// `subprocess`.
    #[serde(rename = "subprocess")]
    Subprocess,
}

/// Why that level was chosen; present only when `engine` is `polars` (MG-REQ-0635).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentPolarsReason {
    /// `qualified`.
    #[serde(rename = "qualified")]
    Qualified,
    /// `version_mismatch`.
    #[serde(rename = "version_mismatch")]
    VersionMismatch,
    /// `not_available`.
    #[serde(rename = "not_available")]
    NotAvailable,
}

/// Where data crosses from one segment to another (MG-REQ-0633).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    /// The index of the segment the data leaves.
    pub from: i64,
    /// The index of the segment the data enters.
    pub to: i64,
    /// The names of the columns crossing, in their output order.
    pub columns: Vec<String>,
    /// How the data crosses (MG-REQ-0633).
    pub transport: BoundaryTransport,
}

/// How the data crosses (MG-REQ-0633).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BoundaryTransport {
    /// `arrow`.
    #[serde(rename = "arrow")]
    Arrow,
    /// `parquet`.
    #[serde(rename = "parquet")]
    Parquet,
}

/// One period the model recomputes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Period {
    /// The start of the period, an RFC 3339 timestamp in UTC.
    pub start: String,
    /// The end of the period, an RFC 3339 timestamp in UTC.
    pub end: String,
}

/// One gate of stage 4 and its status (MG-REQ-0720).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    /// The name of the gate in the gate table of `run/gates.md`.
    pub gate: String,
    /// Whether the gate passes, blocks, or is not applicable yet.
    pub status: GateStatus,
    /// The phase the gate is due in, such as `0.5`; present only when `status` is `not_applicable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// Why the gate blocks; present only when `status` is `block` (MG-REQ-0722).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<GateError>>,
}

/// Whether the gate passes, blocks, or is not applicable yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    /// `pass`.
    #[serde(rename = "pass")]
    Pass,
    /// `block`.
    #[serde(rename = "block")]
    Block,
    /// `not_applicable`.
    #[serde(rename = "not_applicable")]
    NotApplicable,
}

/// One error of a blocking gate (MG-REQ-0008).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateError {
    /// The `MG-####` code of the error.
    pub code: String,
    /// What is wrong.
    pub message: String,
    /// The model that caused it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The step of the model that caused it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    /// How to fix it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl crate::Contract for MergusPlan {
    const NAME: &'static str = "mergus_plan";
    const VERSION: u32 = 1;
    const SEMANTIC_VERSION: &'static str = "1.1.0";
    const SCHEMA: &'static str = include_str!("../../../../schemas/mergus_plan/v1/schema.json");
}
