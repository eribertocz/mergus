//! A simulated Python interpreter: trees recorded from the real one (MG-REQ-0611), and the
//! recorded replies of the programs it runs (MG-REQ-0617.1).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mergus_ports::{PortError, ProcessOutput, Python, PythonTree};

use crate::rng::lock;

/// Returns the tree recorded for each source text, and fails with [`PortError::NotFound`] for
/// a text that has none. Returns the output recorded for each program, arguments and working
/// directory, and fails the same way for a run that has none. Nothing is ever run.
#[derive(Debug, Default)]
pub struct SimPython {
    trees: Mutex<BTreeMap<String, String>>,
    runs: Mutex<BTreeMap<Run, ProcessOutput>>,
    calls: Mutex<Vec<Run>>,
}

/// A program, its arguments and its working directory.
type Run = (String, Vec<String>, PathBuf);

impl SimPython {
    /// Returns an interpreter with no recorded tree.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `tree`, the JSON document the real interpreter printed for `source`. It is read
    /// with [`PythonTree::from_json`] on each [`Python::parse`], as the real port reads it.
    pub fn record(&self, source: impl Into<String>, tree: impl Into<String>) {
        lock(&self.trees).insert(source.into(), tree.into());
    }

    /// Records `output`, what the real interpreter returned when it ran `script` with `args`
    /// in `current_dir`. Each [`Python::run`] of the same three replays it.
    pub fn record_run(
        &self,
        script: impl Into<String>,
        args: &[String],
        current_dir: impl Into<PathBuf>,
        output: ProcessOutput,
    ) {
        let run = (script.into(), args.to_vec(), current_dir.into());
        lock(&self.runs).insert(run, output);
    }

    /// Returns every [`Python::run`] so far, in order, recorded or not: each program, its
    /// arguments and its working directory.
    #[must_use]
    pub fn runs(&self) -> Vec<(String, Vec<String>, PathBuf)> {
        lock(&self.calls).clone()
    }
}

impl Python for SimPython {
    fn parse(&self, source: &str) -> Result<PythonTree, PortError> {
        let tree = lock(&self.trees)
            .get(source)
            .cloned()
            .ok_or_else(|| PortError::NotFound(missing(source)))?;
        PythonTree::from_json(&tree)
    }

    fn run(
        &self,
        script: &str,
        args: &[String],
        current_dir: &Path,
    ) -> Result<ProcessOutput, PortError> {
        let run = (script.to_owned(), args.to_vec(), current_dir.to_path_buf());
        lock(&self.calls).push(run.clone());
        lock(&self.runs).get(&run).cloned().ok_or_else(|| {
            let first = script.lines().next().unwrap_or_default();
            PortError::NotFound(format!(
                "a recorded Python run of the program whose first line is `{first}`, with {} \
                 arguments, in `{}`",
                args.len(),
                current_dir.display()
            ))
        })
    }
}

/// Names a source with no recorded tree by its size and first line, not its whole text.
fn missing(source: &str) -> String {
    let first = source.lines().next().unwrap_or_default();
    format!(
        "a recorded Python tree for the source of {} bytes whose first line is `{first}`",
        source.len()
    )
}
