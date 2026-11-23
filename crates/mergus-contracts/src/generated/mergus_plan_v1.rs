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

impl crate::Contract for MergusPlan {
    const NAME: &'static str = "mergus_plan";
    const VERSION: u32 = 1;
    const SEMANTIC_VERSION: &'static str = "1.0.0";
    const SCHEMA: &'static str = include_str!("../../../../schemas/mergus_plan/v1/schema.json");
}
