//! Simulated sources: they change, fail or return partial data mid-extraction.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use mergus_ports::{PortError, SourceConnector, SourceReader};

use crate::rng::{Rate, SimRng, lock};

/// The source faults to inject, drawn before each batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceFaults {
    /// How often reading the next batch fails.
    pub fail: Rate,
    /// How often the source ends early, so the extraction gets partial data.
    pub partial: Rate,
    /// How often the source switches to its changed version, from the next batch on.
    pub change: Rate,
}

#[derive(Clone, Debug)]
struct Versions {
    batches: Vec<Vec<u8>>,
    changed: Option<Vec<Vec<u8>>>,
}

/// Sources registered by the test, read batch by batch, with injected faults.
#[derive(Debug)]
pub struct SimSourceConnector {
    faults: SourceFaults,
    rng: Arc<Mutex<SimRng>>,
    sources: Mutex<BTreeMap<String, Versions>>,
}

impl SimSourceConnector {
    /// Returns a connector that knows no source.
    #[must_use]
    pub fn new(seed: u64, faults: SourceFaults) -> Self {
        Self {
            faults,
            rng: Arc::new(Mutex::new(SimRng::new(seed))),
            sources: Mutex::new(BTreeMap::new()),
        }
    }

    /// Registers the source `name` with its `batches`, and the batches of the version it
    /// changes to under the `change` fault, if any.
    pub fn register(&self, name: &str, batches: Vec<Vec<u8>>, changed: Option<Vec<Vec<u8>>>) {
        lock(&self.sources).insert(name.to_owned(), Versions { batches, changed });
    }
}

impl SourceConnector for SimSourceConnector {
    fn open(&self, source: &str) -> Result<Box<dyn SourceReader>, PortError> {
        let versions = lock(&self.sources)
            .get(source)
            .cloned()
            .ok_or_else(|| PortError::NotFound(source.to_owned()))?;
        Ok(Box::new(SimSourceReader {
            name: source.to_owned(),
            faults: self.faults,
            rng: Arc::clone(&self.rng),
            versions,
            next: 0,
            ended: false,
        }))
    }
}

/// Reads one simulated source.
struct SimSourceReader {
    name: String,
    faults: SourceFaults,
    rng: Arc<Mutex<SimRng>>,
    versions: Versions,
    next: usize,
    ended: bool,
}

impl SourceReader for SimSourceReader {
    fn next_batch(&mut self) -> Result<Option<Vec<u8>>, PortError> {
        if self.ended {
            return Ok(None);
        }
        let (fails, partial, changes) = {
            let mut rng = lock(&self.rng);
            let fails = rng.hits(self.faults.fail);
            let partial = rng.hits(self.faults.partial);
            (fails, partial, rng.hits(self.faults.change))
        };
        if fails {
            return Err(PortError::Failed(format!(
                "simulated failure of source `{}`",
                self.name
            )));
        }
        if changes && let Some(changed) = self.versions.changed.take() {
            self.versions.batches = changed;
        }
        if partial {
            self.ended = true;
            return Ok(None);
        }
        let batch = self.versions.batches.get(self.next).cloned();
        self.next += 1;
        self.ended = batch.is_none();
        Ok(batch)
    }
}
