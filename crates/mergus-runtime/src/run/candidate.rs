//! Candidates: the results of stage 5, kept in the work zone by fingerprint and seen by no
//! catalog and no command (MG-REQ-0705), reused by a retried run (MG-REQ-0707).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mergus_plan::Plan;
use mergus_ports::{FileSystem, PortError};

use super::RunError;
use super::work_zone::{remove_entry, under};

/// The folder of the candidates, relative to the project root (MG-REQ-0705.1).
pub const CANDIDATES: &str = ".mergus/work/candidates";

/// The last file of a candidate: it exists only once the candidate is complete.
pub const COMPLETE: &str = "complete.json";

/// The temporary name `complete.json` is written to before it is renamed.
const COMPLETE_TEMPORARY: &str = "complete.json.tmp";

/// The rows of a candidate, as an Arrow IPC file.
pub const RESULT: &str = "result.arrow";

/// The temporary name the rows are copied to before they are renamed.
const RESULT_TEMPORARY: &str = "result.arrow.tmp";

/// The candidates of the project at `root`.
pub(crate) struct Candidates<'a> {
    files: &'a dyn FileSystem,
    root: &'a Path,
    dir: PathBuf,
}

impl<'a> Candidates<'a> {
    /// The candidates of the project at `root`, read and written through `files`.
    pub(crate) fn new(files: &'a dyn FileSystem, root: &'a Path) -> Self {
        Self {
            files,
            root,
            dir: under(root, CANDIDATES),
        }
    }

    fn failed(&self, path: &Path) -> impl Fn(PortError) -> RunError + '_ {
        let path = path.to_path_buf();
        move |error| RunError::work(self.root, &path, &error)
    }

    /// The directory of the candidate of `fingerprint`.
    pub(crate) fn dir(&self, fingerprint: &str) -> PathBuf {
        self.dir.join(fingerprint)
    }

    /// The result file of the candidate of `fingerprint`.
    pub(crate) fn result(&self, fingerprint: &str) -> PathBuf {
        self.dir(fingerprint).join(RESULT)
    }

    /// The file the rows of the candidate of `fingerprint` are copied to, before
    /// [`Candidates::complete`] renames it.
    pub(crate) fn result_temporary(&self, fingerprint: &str) -> PathBuf {
        self.dir(fingerprint).join(RESULT_TEMPORARY)
    }

    /// Whether the candidate of `fingerprint` is complete: its `complete.json` exists.
    pub(crate) fn is_complete(&self, fingerprint: &str) -> Result<bool, RunError> {
        let complete = self.dir(fingerprint).join(COMPLETE);
        self.files.exists(&complete).map_err(self.failed(&complete))
    }

    /// The fingerprints of every candidate, complete or not, sorted.
    fn fingerprints(&self) -> Result<Vec<String>, RunError> {
        if !self
            .files
            .exists(&self.dir)
            .map_err(self.failed(&self.dir))?
        {
            return Ok(Vec::new());
        }
        let entries = self
            .files
            .list_dir(&self.dir)
            .map_err(self.failed(&self.dir))?;
        Ok(entries
            .iter()
            .filter_map(|entry| entry.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect())
    }

    /// Deletes the candidate of `fingerprint`, or the stray file of that name.
    pub(crate) fn remove(&self, fingerprint: &str) -> Result<(), RunError> {
        let dir = self.dir(fingerprint);
        if self.files.exists(&dir).map_err(self.failed(&dir))? {
            remove_entry(self.files, &dir).map_err(self.failed(&dir))?;
        }
        Ok(())
    }

    /// Deletes every incomplete candidate and returns the fingerprints of the complete ones,
    /// the results available to the plan (MG-REQ-0707.1, MG-REQ-0708.2).
    pub(crate) fn prune_incomplete(&self) -> Result<BTreeSet<String>, RunError> {
        let mut complete = BTreeSet::new();
        for fingerprint in self.fingerprints()? {
            if self.is_complete(&fingerprint)? {
                complete.insert(fingerprint);
            } else {
                self.remove(&fingerprint)?;
            }
        }
        Ok(complete)
    }

    /// Deletes every candidate whose fingerprint no step of `plan` names (MG-REQ-0707.2).
    pub(crate) fn prune_unnamed(&self, plan: &Plan) -> Result<(), RunError> {
        let named: BTreeSet<&str> = plan
            .document()
            .steps
            .iter()
            .map(|step| step.fingerprint.as_str())
            .collect();
        for fingerprint in self.fingerprints()? {
            if !named.contains(fingerprint.as_str()) {
                self.remove(&fingerprint)?;
            }
        }
        Ok(())
    }

    /// Starts the candidate of `fingerprint` afresh: an empty, incomplete directory.
    pub(crate) fn start(&self, fingerprint: &str) -> Result<(), RunError> {
        self.remove(fingerprint)?;
        let dir = self.dir(fingerprint);
        self.files.create_dir_all(&dir).map_err(self.failed(&dir))
    }

    /// Completes the candidate of `fingerprint`: renames its rows from `result.arrow.tmp`
    /// to `result.arrow`, which puts them on the disk, then writes `complete.json` with its
    /// fingerprint, the run id and its row count to a temporary name and renames it
    /// (MG-REQ-0705.1). So a `complete.json` never stands over rows that are not there.
    pub(crate) fn complete(
        &self,
        fingerprint: &str,
        run_id: &str,
        rows: u64,
    ) -> Result<(), RunError> {
        let rows_temporary = self.result_temporary(fingerprint);
        let result = self.result(fingerprint);
        self.files
            .rename(&rows_temporary, &result)
            .map_err(self.failed(&result))?;
        let dir = self.dir(fingerprint);
        let temporary = dir.join(COMPLETE_TEMPORARY);
        let complete = dir.join(COMPLETE);
        let document = serde_json::json!({
            "fingerprint": fingerprint,
            "run_id": run_id,
            "rows": rows,
        });
        let mut bytes = document.to_string().into_bytes();
        bytes.push(b'\n');
        self.files
            .write(&temporary, &bytes)
            .map_err(self.failed(&temporary))?;
        self.files
            .rename(&temporary, &complete)
            .map_err(self.failed(&complete))
    }
}
