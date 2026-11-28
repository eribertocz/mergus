//! Stage 5: executes the approved plan exactly (MG-REQ-0704). Extractions run first, in
//! order; then each computed step starts once every step it reads has finished, with at most
//! the plan's `max_parallel` running at once. A `skip` step runs nothing. The bytes the plan
//! reads are checked before anything runs, and wall time before each segment (MG-REQ-0718.2).

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use mergus_contracts::generated::mergus_plan_v1::{Step, StepAction};
use mergus_plan::Plan;

use super::candidate::Candidates;
use super::copy::{Copied, copy_result};
use super::events::{StepEnd, StepEvents};
use super::limits::Meter;
use super::{CancelToken, ModelWork, RunError, StepContext, Work};

/// How stage 5 ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Executed {
    /// Every step of the plan has its result.
    Finished {
        /// The rows of each computed model.
        rows: BTreeMap<String, u64>,
    },
    /// The cancel token was set: no segment started after it (MG-REQ-0708).
    Cancelled,
}

impl Executed {
    /// The exit code of the run: 0, or 7 when cancelled (MG-REQ-0708.2).
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Finished { .. } => 0,
            Self::Cancelled => super::CANCELLED_EXIT_CODE,
        }
    }
}

/// What one step's thread sends back: `None` when the token was set.
type Done = (String, Result<Option<Copied>, RunError>);

/// Everything a step needs, shared by the threads of stage 5.
pub(crate) struct Stage<'a> {
    pub(crate) candidates: &'a Candidates<'a>,
    pub(crate) files: &'a dyn mergus_ports::FileSystem,
    pub(crate) root: &'a Path,
    pub(crate) scratch: &'a Path,
    pub(crate) run_id: &'a str,
    pub(crate) cancel: &'a CancelToken,
    pub(crate) meter: &'a Meter<'a>,
}

/// The progress of the computed steps.
struct Progress<'p> {
    pending: Vec<&'p Step>,
    finished: BTreeSet<String>,
    results: BTreeMap<String, PathBuf>,
    rows: BTreeMap<String, u64>,
    running: usize,
    /// Each failure by the position of its step in the plan.
    failures: BTreeMap<usize, RunError>,
    cancelled: bool,
}

impl Stage<'_> {
    /// Runs the extractions of `work`, then the steps of `plan`, reporting each computed step
    /// to `events`.
    pub(crate) fn execute(
        &self,
        plan: &Plan,
        work: &mut Work<'_>,
        events: &mut dyn StepEvents,
    ) -> Result<Executed, RunError> {
        self.meter
            .check_bytes(work.bytes_scanned)
            .map_err(RunError::Limit)?;
        for extraction in &mut work.extractions {
            if self.cancel.is_cancelled() {
                return Ok(Executed::Cancelled);
            }
            extraction(self.cancel).map_err(RunError::Extraction)?;
        }
        let steps = &plan.document().steps;
        for step in steps
            .iter()
            .filter(|step| step.action == StepAction::Compute)
        {
            if !work.models.contains_key(&step.model) {
                return Err(RunError::NoWork(step.model.clone()));
            }
        }
        let limit = usize::try_from(plan.document().max_parallel)
            .unwrap_or(1)
            .max(1);
        self.schedule(steps, &work.models, limit, events)
    }

    /// Runs the computed steps in dependency order, at most `limit` at once.
    fn schedule(
        &self,
        steps: &[Step],
        models: &BTreeMap<String, Box<dyn ModelWork + '_>>,
        limit: usize,
        events: &mut dyn StepEvents,
    ) -> Result<Executed, RunError> {
        let mut progress = self.progress(steps)?;
        let planned: BTreeSet<&str> = steps.iter().map(|step| step.model.as_str()).collect();
        thread::scope(|scope| {
            let (sender, receiver) = mpsc::channel::<Done>();
            loop {
                while progress.running < limit && progress.failures.is_empty() {
                    if self.cancel.is_cancelled() {
                        progress.cancelled = true;
                        break;
                    }
                    let Some(step) = progress.next_ready(&planned) else {
                        break;
                    };
                    let inputs = progress.inputs_of(step);
                    let sender = sender.clone();
                    let work = models.get(&step.model).map(AsRef::as_ref);
                    progress.running += 1;
                    events.started(step);
                    scope.spawn(move || {
                        // The receiver lives until every thread is done.
                        let _ =
                            sender.send((step.model.clone(), self.guarded(step, work, &inputs)));
                    });
                }
                if progress.running == 0 {
                    break;
                }
                let Ok((model, done)) = receiver.recv() else {
                    break;
                };
                if let Some(step) = steps.iter().find(|step| step.model == model) {
                    events.finished(step, &ended(&done));
                }
                progress.record(self.candidates, steps, model, done);
            }
        });
        progress.end()
    }

    /// Runs `step` with `work`, its panic turned into [`RunError::Panicked`] so that the
    /// run never waits for it.
    fn guarded(
        &self,
        step: &Step,
        work: Option<&dyn ModelWork>,
        inputs: &BTreeMap<String, PathBuf>,
    ) -> Result<Option<Copied>, RunError> {
        let run = || {
            work.ok_or_else(|| RunError::NoWork(step.model.clone()))
                .and_then(|work| self.run_step(step, work, inputs))
        };
        panic::catch_unwind(AssertUnwindSafe(run))
            .unwrap_or_else(|_| Err(RunError::Panicked(step.model.clone())))
    }

    /// The progress before any computed step runs: `skip` steps are finished, with the
    /// candidate they name as their result when it is complete (MG-REQ-0704.1).
    fn progress<'p>(&self, steps: &'p [Step]) -> Result<Progress<'p>, RunError> {
        let mut progress = Progress {
            pending: Vec::new(),
            finished: BTreeSet::new(),
            results: BTreeMap::new(),
            rows: BTreeMap::new(),
            running: 0,
            failures: BTreeMap::new(),
            cancelled: false,
        };
        for step in steps {
            match step.action {
                StepAction::Compute => progress.pending.push(step),
                StepAction::Skip => {
                    if self.candidates.is_complete(&step.fingerprint)? {
                        let result = self.candidates.result(&step.fingerprint);
                        progress.results.insert(step.model.clone(), result);
                    }
                    progress.finished.insert(step.model.clone());
                }
            }
        }
        Ok(progress)
    }

    /// Runs one computed step: checks the token, runs its work into its scratch directory,
    /// then copies the rows into its candidate's temporary result, checking the token
    /// between batches, and completes it. `None` means the token was set.
    fn run_step(
        &self,
        step: &Step,
        work: &dyn ModelWork,
        inputs: &BTreeMap<String, PathBuf>,
    ) -> Result<Option<Copied>, RunError> {
        if self.cancel.is_cancelled() {
            return Ok(None);
        }
        self.meter.check_time().map_err(RunError::Limit)?;
        let scratch = self.scratch.join("steps").join(&step.model);
        self.files
            .create_dir_all(&scratch)
            .map_err(|error| RunError::scratch(self.root, &scratch, &error))?;
        let output = scratch.join(super::candidate::RESULT);
        let context = StepContext {
            model: &step.model,
            inputs,
            output: &output,
            scratch: &scratch,
            cancel: self.cancel,
        };
        work.run(&context).map_err(RunError::Step)?;
        self.candidates.start(&step.fingerprint)?;
        let target = self.candidates.result_temporary(&step.fingerprint);
        let failed = |error| RunError::work(self.root, &target, &error);
        let copied = copy_result(
            self.files,
            &output,
            &target,
            self.cancel,
            self.meter,
            failed,
        )?;
        let Some(copied) = copied else {
            return Ok(None);
        };
        self.candidates
            .complete(&step.fingerprint, self.run_id, copied.rows)?;
        Ok(Some(copied))
    }
}

impl<'p> Progress<'p> {
    /// The first pending step, in plan order, whose planned inputs have all finished.
    fn next_ready(&mut self, planned: &BTreeSet<&str>) -> Option<&'p Step> {
        let position = self.pending.iter().position(|step| {
            step.inputs.iter().all(|input| {
                input == &step.model
                    || !planned.contains(input.as_str())
                    || self.finished.contains(input)
            })
        })?;
        Some(self.pending.remove(position))
    }

    /// The result files of the inputs of `step` that have one.
    fn inputs_of(&self, step: &Step) -> BTreeMap<String, PathBuf> {
        step.inputs
            .iter()
            .filter_map(|input| {
                let result = self.results.get(input)?;
                Some((input.clone(), result.clone()))
            })
            .collect()
    }

    /// Records what the step of `model` did.
    fn record(
        &mut self,
        candidates: &Candidates<'_>,
        steps: &[Step],
        model: String,
        done: Result<Option<Copied>, RunError>,
    ) {
        self.running -= 1;
        match done {
            Ok(Some(Copied { rows, .. })) => {
                if let Some(step) = steps.iter().find(|step| step.model == model) {
                    self.results
                        .insert(model.clone(), candidates.result(&step.fingerprint));
                }
                self.rows.insert(model.clone(), rows);
                self.finished.insert(model);
            }
            Ok(None) => self.cancelled = true,
            Err(error) => {
                let position = steps.iter().position(|step| step.model == model);
                self.failures.insert(position.unwrap_or(usize::MAX), error);
            }
        }
    }

    /// How stage 5 ended: the failure of the first step in plan order, so the same failures
    /// always give the same error (MG-REQ-0005), else cancelled, else finished.
    fn end(mut self) -> Result<Executed, RunError> {
        if let Some((_, failure)) = self.failures.pop_first() {
            return Err(failure);
        }
        if self.cancelled {
            return Ok(Executed::Cancelled);
        }
        if let Some(step) = self.pending.first() {
            return Err(RunError::Unordered(step.model.clone()));
        }
        Ok(Executed::Finished { rows: self.rows })
    }
}

/// How the step that sent `done` ended, for its event.
fn ended(done: &Result<Option<Copied>, RunError>) -> StepEnd {
    match done {
        Ok(Some(copied)) => StepEnd {
            succeeded: true,
            rows: copied.rows,
            bytes: copied.bytes,
        },
        Ok(None) | Err(_) => StepEnd {
            succeeded: false,
            rows: 0,
            bytes: 0,
        },
    }
}
