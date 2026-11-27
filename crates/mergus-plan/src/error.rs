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
    /// The plan does not match the `mergus_plan` contract.
    #[error(transparent)]
    Emit(#[from] EmitError),
}
