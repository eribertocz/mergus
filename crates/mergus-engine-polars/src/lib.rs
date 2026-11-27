//! Execute Polars steps.
//!
//! Until a phase adds the `native` and `in_process` levels, every Polars step runs at the
//! `subprocess` level (MG-REQ-0635.3): a Python child started through the `Python` port, in the
//! step's own fresh directory under the run's work directory, with only the allowlisted
//! environment of [`child_environment`] (MG-REQ-0617.1, MG-REQ-0636.1).
//!
//! [`plan_step`] calls the step once with empty LazyFrames that carry the input schemas and
//! returns its output schema (MG-REQ-0617.2). [`execute_step`] passes each input as
//! `pl.scan_ipc(<file>)`, collects the result once and returns it (MG-REQ-0617.3). The child
//! imports the model file by its path and adds nothing to its module, so a step stays a
//! plain function a Python test can call (MG-REQ-0619.1).

#![forbid(unsafe_code)]

mod error;
mod ipc;
mod step;

pub use error::{PolarsError, StepFailure};
pub use step::{
    Frame, Output, Parameter, Step, child_environment, execute_step, plan_step, step_dir,
};
