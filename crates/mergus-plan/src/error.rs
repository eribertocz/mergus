//! Why a plan cannot be made.

use mergus_contracts::EmitError;
use thiserror::Error;

use crate::Gate;

/// Why a plan cannot be made. Every variant is an internal error: the caller passed models or
/// a request that the read project and the CLI never produce.
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
    /// The plan does not match the `mergus_plan` contract.
    #[error(transparent)]
    Emit(#[from] EmitError),
}
