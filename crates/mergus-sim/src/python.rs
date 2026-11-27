//! A simulated Python interpreter: trees recorded from the real one (MG-REQ-0611).

use std::collections::BTreeMap;
use std::sync::Mutex;

use mergus_ports::{PortError, Python, PythonTree};

use crate::rng::lock;

/// Returns the tree recorded for each source text, and fails with [`PortError::NotFound`] for
/// a text that has none. Nothing is ever run.
#[derive(Debug, Default)]
pub struct SimPython {
    trees: Mutex<BTreeMap<String, String>>,
}

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
}

impl Python for SimPython {
    fn parse(&self, source: &str) -> Result<PythonTree, PortError> {
        let tree = lock(&self.trees)
            .get(source)
            .cloned()
            .ok_or_else(|| PortError::NotFound(missing(source)))?;
        PythonTree::from_json(&tree)
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
