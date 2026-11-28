//! What stage 5 reports of each computed step as it runs (MG-REQ-0710.2).

use mergus_contracts::generated::mergus_plan_v1::Step;

/// How a computed step ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepEnd {
    /// Whether its candidate is complete; `false` when it failed, was cancelled or stopped
    /// by a limit.
    pub succeeded: bool,
    /// The rows written to its candidate; 0 unless it succeeded.
    pub rows: u64,
    /// The size of its candidate's rows file; 0 unless it succeeded.
    pub bytes: u64,
}

/// Receives the start and the end of each computed step of stage 5, in the order they
/// happen. A `skip` step is never reported (MG-REQ-0710.2).
pub trait StepEvents {
    /// The computed `step` starts.
    fn started(&mut self, step: &Step);

    /// The computed `step`, reported started before, has ended as `end` says.
    fn finished(&mut self, step: &Step, end: &StepEnd);
}

/// Reports nothing.
pub(crate) struct NoEvents;

impl StepEvents for NoEvents {
    fn started(&mut self, _step: &Step) {}

    fn finished(&mut self, _step: &Step, _end: &StepEnd) {}
}
