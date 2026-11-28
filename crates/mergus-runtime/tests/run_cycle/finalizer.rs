//! The finalizer runs once at the end of every run, and a failure of its own is a warning
//! after the run's error (MG-REQ-0709).

use std::path::{Path, PathBuf};

use mergus_ports::{FileSystem, PortError, SeekRead};
use mergus_runtime::{CANDIDATES, CancelToken, ModelWork, Run, RunError, StepContext, StepFailure};
use mergus_sim::SimFileSystem;

use crate::fixture::{
    COUNT, Log, at, cleared, database, fingerprint, models, orders, read, root, tree,
};

/// A work that fails with the code `MG-0101`.
#[cfg(test)]
pub struct Failing;

#[cfg(test)]
impl ModelWork for Failing {
    fn run(&self, context: &StepContext<'_>) -> Result<(), StepFailure> {
        Err(StepFailure {
            name: context.model.to_owned(),
            code: Some("MG-0101"),
            message: "broken".to_owned(),
        })
    }
}

/// The complete candidate of `stg_orders`, the only one kept: its directory and its files.
#[cfg(test)]
pub fn only_stg_kept(files: &dyn FileSystem, stg: &str) {
    let stg = format!("{CANDIDATES}/{stg}");
    assert_eq!(
        tree(files, &at(CANDIDATES)),
        [
            stg.clone(),
            format!("{stg}/complete.json"),
            format!("{stg}/result.arrow")
        ]
    );
}

#[test]
fn mg_req_0709_finalizer_runs_on_failure() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let planned = crate::fixture::plan(&project, run.available(), 1);
    let adopted = run.adopt(&planned).unwrap();
    // An incomplete candidate, as a step stopped half-way leaves it.
    let partial = at(CANDIDATES).join(fingerprint(&planned, "orders_count"));
    files.create_dir_all(&partial).unwrap();
    files
        .write(&partial.join("result.arrow.tmp"), b"rows")
        .unwrap();
    let mut work = models(&files, &database, &project, &Log::default(), None);
    work.models
        .insert("orders_big".to_owned(), Box::new(Failing));
    let failed = run.execute(&adopted, &cleared(), &mut work);
    let scratch = run.scratch().to_path_buf();
    let report = run.finish(failed);

    assert_eq!(report.error_code(), Some("MG-0101"));
    assert!(report.outcome.is_err());
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert!(
        !files.exists(&scratch).unwrap(),
        "the scratch directory is kept"
    );
    only_stg_kept(&files, &fingerprint(&planned, "stg_orders"));
}

#[test]
fn mg_req_0709_finalizer_failure_is_a_warning_after_the_error() {
    let files = orders(COUNT);
    let stubborn = Stubborn {
        inner: &files,
        dir: at(".mergus/work/runs/run1"),
    };
    let database = database();
    let project = read(&files);
    let run = Run::begin(&stubborn, root(), "run1", CancelToken::new()).unwrap();
    let planned = crate::fixture::plan(&project, run.available(), 1);
    let adopted = run.adopt(&planned).unwrap();
    let mut work = models(&files, &database, &project, &Log::default(), None);
    work.models
        .insert("stg_orders".to_owned(), Box::new(Failing));
    let failed = run.execute(&adopted, &cleared(), &mut work);
    let report = run.finish(failed);

    assert_eq!(report.error_code(), Some("MG-0101"));
    assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
    assert!(matches!(report.warnings[0], RunError::Scratch { .. }));
    assert_eq!(report.warnings[0].code(), Some("MG-0508"));
}

#[test]
fn mg_req_0709_finalizer_runs_on_success() {
    let files = orders(COUNT);
    let database = database();
    let project = read(&files);
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let planned = crate::fixture::plan(&project, run.available(), 2);
    let adopted = run.adopt(&planned).unwrap();
    let mut work = models(&files, &database, &project, &Log::default(), None);
    let executed = run.execute(&adopted, &cleared(), &mut work);
    let scratch = run.scratch().to_path_buf();
    let report = run.finish(executed);

    assert_eq!(report.error_code(), None);
    assert_eq!(report.outcome.as_ref().map(|done| done.exit_code()), Ok(0));
    assert!(report.warnings.is_empty());
    assert!(!files.exists(&scratch).unwrap());
}

#[test]
fn mg_req_0709_a_run_dropped_unfinished_is_finalized() {
    let files = orders(COUNT);
    let run = Run::begin(&files, root(), "run1", CancelToken::new()).unwrap();
    let scratch = run.scratch().to_path_buf();
    let partial = at(CANDIDATES).join("abc");
    files.create_dir_all(&partial).unwrap();
    assert!(files.exists(&scratch).unwrap());
    drop(run);

    assert!(
        !files.exists(&scratch).unwrap(),
        "the scratch directory is kept"
    );
    assert!(
        !files.exists(&partial).unwrap(),
        "an incomplete candidate is kept"
    );
}

/// The simulated file system, refusing to delete `dir`.
#[cfg(test)]
struct Stubborn<'a> {
    inner: &'a SimFileSystem,
    dir: PathBuf,
}

#[cfg(test)]
impl FileSystem for Stubborn<'_> {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PortError> {
        self.inner.read(path)
    }

    fn open(&self, path: &Path) -> Result<Box<dyn SeekRead>, PortError> {
        self.inner.open(path)
    }

    fn create(&self, path: &Path) -> Result<Box<dyn std::io::Write + Send>, PortError> {
        self.inner.create(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> Result<(), PortError> {
        self.inner.write(path, contents)
    }

    fn remove_file(&self, path: &Path) -> Result<(), PortError> {
        self.inner.remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), PortError> {
        if path == self.dir {
            return Err(PortError::Failed("permission denied".to_owned()));
        }
        self.inner.remove_dir_all(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), PortError> {
        self.inner.rename(from, to)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), PortError> {
        self.inner.create_dir_all(path)
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<PathBuf>, PortError> {
        self.inner.list_dir(path)
    }

    fn exists(&self, path: &Path) -> Result<bool, PortError> {
        self.inner.exists(path)
    }
}

#[test]
fn mg_req_0709_a_failed_start_leaves_no_scratch() {
    let files = orders(COUNT);
    // The candidates folder is a file: it cannot be listed, so the start fails after the
    // scratch directory was created.
    files.create_dir_all(&at(".mergus/work")).unwrap();
    files.write(&at(CANDIDATES), b"a file").unwrap();
    let failed = Run::begin(&files, root(), "run1", CancelToken::new());
    assert_eq!(failed.err().and_then(|error| error.code()), Some("MG-0704"));
    assert!(!files.exists(&at(".mergus/work/runs/run1")).unwrap());
}
