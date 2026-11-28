//! What stage 5 runs: the work of each computed model and the extractions, given by the
//! caller, which maps the read project onto the engines.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{CancelToken, StepFailure};

/// What one model's work sees when it runs.
#[derive(Debug)]
pub struct StepContext<'a> {
    /// The model.
    pub model: &'a str,
    /// The result file, Arrow IPC, of each input that is a step of the plan with a result:
    /// a model computed earlier in the run, or the candidate a `skip` step names. Other
    /// inputs, such as raw tables, are the work's own.
    pub inputs: &'a BTreeMap<String, PathBuf>,
    /// Where the work leaves its rows, as an Arrow IPC file.
    pub output: &'a Path,
    /// The model's own fresh directory in the run's scratch directory.
    pub scratch: &'a Path,
    /// The run's cancel token.
    pub cancel: &'a CancelToken,
}

/// The work of one model: runs its segments and leaves its rows at
/// [`StepContext::output`].
pub trait ModelWork: Send + Sync {
    /// Runs the model.
    ///
    /// # Errors
    ///
    /// Fails with the [`StepFailure`] of the engine.
    fn run(&self, context: &StepContext<'_>) -> Result<(), StepFailure>;
}

/// One extraction, run before any segment (MG-REQ-0704): typically the chunk load of one
/// table by [`crate::load_table`].
pub type Extraction<'a> = Box<dyn FnMut(&CancelToken) -> Result<(), StepFailure> + 'a>;

/// What stage 5 runs, beyond the plan.
pub struct Work<'a> {
    /// The extractions, run in order before any segment.
    pub extractions: Vec<Extraction<'a>>,
    /// The work of each model, by name. Every model the plan computes needs one.
    pub models: BTreeMap<String, Box<dyn ModelWork + 'a>>,
}
