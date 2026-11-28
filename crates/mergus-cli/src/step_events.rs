//! The `step_started` and `step_finished` events of stage 5 (MG-REQ-0710.2): `step_id` is
//! the step's fingerprint, `engine` the engine of its segment, `rows` the rows written to its
//! candidate and `bytes` the size of the candidate's rows file.

use std::collections::BTreeMap;

use mergus_contracts::generated::mergus_plan_v1::Step;
use mergus_ports::MonotonicTime;
use mergus_runtime::{StepEnd, StepEvents};

use crate::error::CliError;
use crate::output::Output;

/// The step events of one run, written to its output.
pub(crate) struct Events<'o, 'a> {
    output: &'o mut Output<'a>,
    /// The engine of each model's segment, by model.
    engines: &'o BTreeMap<String, &'static str>,
    /// When each running step started, by fingerprint.
    started: BTreeMap<String, MonotonicTime>,
    /// The first failure to write an event; the run goes on, and reports it at its end.
    failed: Option<CliError>,
}

impl<'o, 'a> Events<'o, 'a> {
    /// The events of a run written to `output`, with the engine of each model in `engines`.
    pub(crate) fn new(
        output: &'o mut Output<'a>,
        engines: &'o BTreeMap<String, &'static str>,
    ) -> Self {
        Self {
            output,
            engines,
            started: BTreeMap::new(),
            failed: None,
        }
    }

    /// Ends the events, with the first event that could not be written.
    ///
    /// # Errors
    ///
    /// Fails with that failure.
    pub(crate) fn end(self) -> Result<(), CliError> {
        self.failed.map_or(Ok(()), Err)
    }

    fn keep(&mut self, written: Result<(), CliError>) {
        if let (Err(error), None) = (written, &self.failed) {
            self.failed = Some(error);
        }
    }
}

impl StepEvents for Events<'_, '_> {
    fn started(&mut self, step: &Step) {
        let now = self.output.clock().monotonic();
        self.started.insert(step.fingerprint.clone(), now);
        let engine = self.engines.get(&step.model).copied().unwrap_or_default();
        let written = self
            .output
            .step_started(&step.fingerprint, &step.model, engine);
        self.keep(written);
    }

    fn finished(&mut self, step: &Step, end: &StepEnd) {
        let now = self.output.clock().monotonic().elapsed();
        let since = self
            .started
            .remove(&step.fingerprint)
            .map_or(now, MonotonicTime::elapsed);
        let written = self.output.step_finished(
            &step.fingerprint,
            &step.model,
            end,
            now.saturating_sub(since),
        );
        self.keep(written);
    }
}
