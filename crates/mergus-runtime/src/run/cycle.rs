//! One run cycle: its scratch directory and candidates from start to end (MG-REQ-0503,
//! MG-REQ-0705, MG-REQ-0707, MG-REQ-0708).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mergus_plan::{Cleared, Plan};
use mergus_ports::FileSystem;

use super::candidate::Candidates;
use super::execute::Stage;
use super::work_zone::{open_scratch, remove_scratch};
use super::{CancelToken, Executed, RunError, Work};

/// A plan whose unnamed candidates [`Run::adopt`] deleted: the only way to stage 5.
#[derive(Debug)]
pub struct Adopted<'p> {
    plan: &'p Plan,
}

impl Adopted<'_> {
    /// The plan.
    #[must_use]
    pub fn plan(&self) -> &Plan {
        self.plan
    }
}

/// A run in progress: it owns its scratch directory, `.mergus/work/runs/<run_id>/`.
pub struct Run<'a> {
    files: &'a dyn FileSystem,
    root: &'a Path,
    run_id: String,
    cancel: CancelToken,
    scratch: PathBuf,
    available: BTreeSet<String>,
}

impl<'a> Run<'a> {
    /// Starts the run `run_id` of the project at `root`: deletes the scratch directories
    /// a crash left (MG-REQ-0503.1) and the incomplete candidates (MG-REQ-0707.1), and
    /// creates its own scratch directory.
    ///
    /// # Errors
    ///
    /// Fails with [`RunError::Work`] when the work zone cannot be read or written.
    pub fn begin(
        files: &'a dyn FileSystem,
        root: &'a Path,
        run_id: &str,
        cancel: CancelToken,
    ) -> Result<Self, RunError> {
        let scratch = open_scratch(files, root, run_id)?;
        let available = Candidates::new(files, root).prune_incomplete()?;
        Ok(Self {
            files,
            root,
            run_id: run_id.to_owned(),
            cancel,
            scratch,
            available,
        })
    }

    /// The fingerprints of the complete candidates, for the plan's available results
    /// (MG-REQ-0707.1).
    #[must_use]
    pub fn available(&self) -> &BTreeSet<String> {
        &self.available
    }

    /// The run's scratch directory.
    #[must_use]
    pub fn scratch(&self) -> &Path {
        &self.scratch
    }

    /// The run's cancel token.
    #[must_use]
    pub fn cancel(&self) -> &CancelToken {
        &self.cancel
    }

    /// Takes `plan` as the run's plan: deletes every complete candidate no step of it names
    /// (MG-REQ-0707.2), and returns the plan adopted, which stage 5 needs.
    ///
    /// # Errors
    ///
    /// Fails with [`RunError::Work`] when a candidate cannot be deleted.
    pub fn adopt<'p>(&self, plan: &'p Plan) -> Result<Adopted<'p>, RunError> {
        Candidates::new(self.files, self.root).prune_unnamed(plan)?;
        Ok(Adopted { plan })
    }

    /// Stage 5: executes the adopted plan, which stage 4 cleared, with `work`
    /// (MG-REQ-0704).
    ///
    /// # Errors
    ///
    /// Fails with the [`RunError`] of the first failed step in plan order, or of an
    /// extraction.
    pub fn execute(
        &self,
        adopted: &Adopted<'_>,
        _cleared: &Cleared,
        work: &mut Work<'_>,
    ) -> Result<Executed, RunError> {
        let plan = adopted.plan;
        let candidates = Candidates::new(self.files, self.root);
        let stage = Stage {
            candidates: &candidates,
            files: self.files,
            root: self.root,
            scratch: &self.scratch,
            run_id: &self.run_id,
            cancel: &self.cancel,
        };
        stage.execute(plan, work)
    }

    /// Ends the run, whether it succeeded, failed or was cancelled: deletes the incomplete
    /// candidates and the scratch directory, and keeps the complete candidates
    /// (MG-REQ-0503.1, MG-REQ-0708.2).
    ///
    /// # Errors
    ///
    /// Fails with [`RunError::Work`] when something cannot be deleted.
    pub fn finish(self) -> Result<(), RunError> {
        let pruned = Candidates::new(self.files, self.root).prune_incomplete();
        let removed = remove_scratch(self.files, self.root, &self.scratch);
        pruned.and(removed)
    }
}
