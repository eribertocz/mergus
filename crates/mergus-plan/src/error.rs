//! Why a plan cannot be made.

use mergus_contracts::EmitError;
use thiserror::Error;

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
    /// The plan does not match the `mergus_plan` contract.
    #[error(transparent)]
    Emit(#[from] EmitError),
}
