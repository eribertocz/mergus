//! Why a plan cannot be made.

use mergus_contracts::EmitError;
use thiserror::Error;

use crate::Gate;

/// Why a plan cannot be made. Every variant but [`PlanError::Refresh`] is an internal error:
/// the caller passed models or a request that the read project and the CLI never produce.
#[derive(Debug, Error)]
pub enum PlanError {
    /// A selected name is not a model of the project.
    #[error("internal error: the selected model `{0}` is not a model of the project")]
    UnknownSelected(String),
    /// The models read one another in a cycle, so no order exists.
    #[error("internal error: the models {} read one another in a cycle", .0.join(", "))]
    Cycle(Vec<String>),
    /// Stage 4 was given no result for a gate other than credentials.
    #[error("internal error: stage 4 was given no result for the gate `{0}`")]
    GateNotGiven(Gate),
    /// Stage 4 was not told whether a required secret reference resolves.
    #[error("internal error: stage 4 was not told whether `{0}` resolves")]
    ExistenceNotGiven(String),
    /// Errors given under one gate name another gate.
    #[error("internal error: an error of the gate `{found}` was given under the gate `{filed}`")]
    GateMismatch {
        /// The gate the errors were given under.
        filed: Gate,
        /// The gate the error names.
        found: Gate,
    },
    /// A gate is said to wait for a phase that is empty, not a version, or not after the
    /// current one.
    #[error(
        "internal error: the gate `{gate}` waits for the phase `{phase}`, which is not a \
         phase after {current}",
        current = crate::gate::CURRENT_PHASE
    )]
    PhaseNotLater {
        /// The gate.
        gate: Gate,
        /// The phase given.
        phase: String,
    },
    /// The execution fields of a step break a condition of MG-REQ-0747.4 that the schema
    /// cannot express.
    #[error("internal error: the step of `{model}` breaks MG-REQ-0747.4: {broken}")]
    ExecutionFields {
        /// The model of the step.
        model: String,
        /// The condition broken.
        broken: &'static str,
    },
    /// Models whose refresh options are invalid (MG-REQ-0654, MG-REQ-0657.2, MG-REQ-0661),
    /// in model name order; each error carries its code.
    #[error("{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))]
    Refresh(Vec<RefreshError>),
    /// An option holds a value the read project never gives.
    #[error("internal error: the option `{option}` of `{model}` holds a value it cannot hold")]
    BadOption {
        /// The model.
        model: String,
        /// The option.
        option: String,
    },
    /// A backfill names a model that is not selected.
    #[error("internal error: the backfilled model `{0}` is not a selected model")]
    UnknownBackfill(String),
    /// A backfill ends before it starts.
    #[error("internal error: the backfill of `{0}` ends before it starts")]
    InvertedBackfill(String),
    /// The plan does not match the `mergus_plan` contract.
    #[error(transparent)]
    Emit(#[from] EmitError),
}

/// Why the refresh of a model cannot be planned. Each error has its `MG-06xx` code
/// (MG-REQ-0008) and exit code 1 (MG-REQ-0654.1).
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum RefreshError {
    /// An `incremental` model has neither `partition_by` nor `key` (MG-REQ-0654).
    #[error(
        "model `{model}` has `refresh = \"incremental\"` but neither `partition_by` nor `key`; \
         set `partition_by = \"<column>:<grain>\"` to recompute periods, or `key` to merge by key"
    )]
    IncrementalWithoutPeriodOrKey {
        /// The model.
        model: String,
    },
    /// A `history` model lacks `key`, `by` or both (MG-REQ-0661).
    #[error(
        "history model `{model}` does not declare {}; declare the business key and the column \
         or columns that detect a change",
        .missing.iter().map(|option| format!("`{option}`")).collect::<Vec<_>>().join(" and ")
    )]
    HistoryWithout {
        /// The model.
        model: String,
        /// The options it lacks, `key` before `by`.
        missing: Vec<&'static str>,
    },
    /// A backfilled model has no `partition_by` (MG-REQ-0657.2).
    #[error(
        "model `{model}` cannot be backfilled: it has no `partition_by`; give it \
         `partition_by = \"<column>:<grain>\"`"
    )]
    BackfillWithoutPartition {
        /// The model.
        model: String,
    },
    /// A backfilled `history` model's rebuild is not confirmed (MG-REQ-0657.4).
    #[error(
        "history model `{model}` cannot be backfilled without confirming the rebuild of its \
         history; a backfill rebuilds every version it holds"
    )]
    BackfillWithoutRebuild {
        /// The model.
        model: String,
    },
}

impl RefreshError {
    /// The `MG-####` code of the error (MG-REQ-0008).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::IncrementalWithoutPeriodOrKey { .. } => "MG-0604",
            Self::HistoryWithout { .. } => "MG-0605",
            Self::BackfillWithoutPartition { .. } | Self::BackfillWithoutRebuild { .. } => {
                "MG-0606"
            }
        }
    }
}
