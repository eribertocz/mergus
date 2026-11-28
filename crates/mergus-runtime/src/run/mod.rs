//! The run cycle: stage 1 reads, stages 2 to 4 plan and gate, stage 5 executes, all through
//! ports, with the work zone and the candidates kept as MG-REQ-0503 and MG-REQ-0705 say.

mod cancel;
mod candidate;
mod copy;
mod cycle;
mod error;
mod execute;
mod plan;
mod read;
mod read_errors;
mod read_python;
mod sql_work;
mod work;
mod work_zone;

pub use cancel::{CANCELLED_EXIT_CODE, CancelToken};
pub use candidate::{CANDIDATES, COMPLETE, RESULT};
pub use cycle::Run;
pub use error::{RunError, StepFailure};
pub use execute::Executed;
pub use plan::{plan_models, plan_project};
pub use read::{ReadError, ReadModel, ReadPorts, ReadProject, read_project};
pub use read_errors::FileErrors;
pub use sql_work::{SqlInput, SqlWork};
pub use work::{Extraction, ModelWork, StepContext, Work};
pub use work_zone::RUNS;
