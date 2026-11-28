//! The run cycle: stage 1 reads, stages 2 to 4 plan and gate, stage 5 executes within the
//! run's limits, all through ports, with the work zone and the candidates kept as MG-REQ-0503
//! and MG-REQ-0705 say, and the finalizer at the end (MG-REQ-0709, MG-REQ-0718).

mod cancel;
mod candidate;
mod copy;
mod cycle;
mod error;
mod execute;
mod limits;
mod plan;
mod read;
mod read_cycles;
mod read_errors;
mod read_python;
mod report;
mod sql_work;
mod work;
mod work_zone;

pub use cancel::{CANCELLED_EXIT_CODE, CancelToken};
pub use candidate::{CANDIDATES, COMPLETE, RESULT};
pub use cycle::{Adopted, Run};
pub use error::{RunError, StepFailure};
pub use execute::Executed;
pub use limits::{Limit, LimitStop, Limits};
pub use plan::{plan_models, plan_project};
pub use read::{ReadError, ReadModel, ReadPorts, ReadProject, read_project};
pub use read_errors::FileErrors;
pub use report::RunReport;
pub use sql_work::{SqlInput, SqlWork};
pub use work::{Extraction, ModelWork, StepContext, Work};
pub use work_zone::RUNS;
