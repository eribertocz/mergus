//! One run cycle: its scratch directory and candidates from start to end (MG-REQ-0503,
//! MG-REQ-0705, MG-REQ-0707, MG-REQ-0708), its limits (MG-REQ-0718) and its finalizer
//! (MG-REQ-0709).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mergus_plan::{Cleared, Plan};
use mergus_ports::{Clock, FileSystem};

use super::candidate::Candidates;
use super::execute::Stage;
use super::limits::Meter;
use super::work_zone::{open_scratch, remove_scratch};
use super::{CancelToken, Executed, Limits, RunError, RunReport, Work};

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
///
/// The finalizer cannot be skipped: [`Run::finish`] runs it and returns the report, and a
/// run dropped without it is finalized on drop, its own failures then unreported.
pub struct Run<'a> {
    files: &'a dyn FileSystem,
    root: &'a Path,
    run_id: String,
    cancel: CancelToken,
    meter: Meter<'a>,
    scratch: PathBuf,
    available: BTreeSet<String>,
    finalized: bool,
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
        Self::start(
            files,
            root,
            run_id,
            cancel,
            Meter::new(Limits::default(), None),
        )
    }

    /// [`Run::begin`] with `limits`, its wall time measured on `clock` from now
    /// (MG-REQ-0718.2).
    ///
    /// # Errors
    ///
    /// Fails with [`RunError::Work`] when the work zone cannot be read or written.
    pub fn begin_with_limits(
        files: &'a dyn FileSystem,
        root: &'a Path,
        run_id: &str,
        cancel: CancelToken,
        limits: Limits,
        clock: &'a dyn Clock,
    ) -> Result<Self, RunError> {
        Self::start(files, root, run_id, cancel, Meter::new(limits, Some(clock)))
    }

    fn start(
        files: &'a dyn FileSystem,
        root: &'a Path,
        run_id: &str,
        cancel: CancelToken,
        meter: Meter<'a>,
    ) -> Result<Self, RunError> {
        let scratch = open_scratch(files, root, run_id)?;
        let available = match Candidates::new(files, root).prune_incomplete() {
            Ok(available) => available,
            Err(error) => {
                // No run exists to finalize: remove the scratch directory here. The start's
                // own error is the one reported.
                drop(remove_scratch(files, root, &scratch));
                return Err(error);
            }
        };
        Ok(Self {
            files,
            root,
            run_id: run_id.to_owned(),
            cancel,
            meter,
            scratch,
            available,
            finalized: false,
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
    /// (MG-REQ-0704), within the run's limits (MG-REQ-0718.2).
    ///
    /// # Errors
    ///
    /// Fails with the [`RunError`] of the first failed step in plan order, or of an
    /// extraction, or with [`RunError::Limit`] when a limit stopped the run.
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
            meter: &self.meter,
        };
        stage.execute(plan, work)
    }

    /// The finalizer: ends the run, whether it succeeded, failed, was cancelled or stopped
    /// by a limit. Deletes the incomplete candidates and the scratch directory, keeps the
    /// complete candidates (MG-REQ-0503.1, MG-REQ-0708.2, MG-REQ-0709.1), and returns the
    /// report of `outcome`, with the finalizer's own failures as warnings after it
    /// (MG-REQ-0709.2). Notifications are a no-op (MG-REQ-0709.3).
    #[must_use]
    pub fn finish(mut self, outcome: Result<Executed, RunError>) -> RunReport {
        let warnings = self.finalize();
        RunReport { outcome, warnings }
    }

    /// Cleans the work zone once, and returns what failed.
    fn finalize(&mut self) -> Vec<RunError> {
        if self.finalized {
            return Vec::new();
        }
        self.finalized = true;
        let pruned = Candidates::new(self.files, self.root).prune_incomplete();
        let removed = remove_scratch(self.files, self.root, &self.scratch);
        [pruned.err(), removed.err()]
            .into_iter()
            .flatten()
            .collect()
    }
}

impl Drop for Run<'_> {
    /// Finalizes a run its caller did not finish, so no path skips the finalizer.
    fn drop(&mut self) {
        // Nobody is left to report to: the caller dropped the run without its report.
        drop(self.finalize());
    }
}
