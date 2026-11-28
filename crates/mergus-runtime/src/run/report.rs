//! The run report the finalizer writes (MG-REQ-0709.2).

use super::{Executed, RunError};

/// How a run ended, for its `run_finished` event (MG-REQ-0709.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunReport {
    /// What the run did: stage 5 finished or cancelled, with its exit code, or the run's
    /// own error, with its code.
    pub outcome: Result<Executed, RunError>,
    /// The failures of the finalizer itself, reported as warnings after the run's error,
    /// which they never replace.
    pub warnings: Vec<RunError>,
}

impl RunReport {
    /// The code of the run's error; `None` when the run did not fail, or for an internal
    /// error.
    #[must_use]
    pub fn error_code(&self) -> Option<&'static str> {
        self.outcome.as_ref().err().and_then(RunError::code)
    }
}
