//! A simulated object store: errors, latency, partial responses and lost writes.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mergus_ports::{Clock, ObjectStore, PortError};

use crate::SimClock;
use crate::rng::{Rate, SimRng, lock};

/// The object-store faults to inject.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectStoreFaults {
    /// How often a call fails.
    pub error: Rate,
    /// How often a call first waits `latency_duration` on the simulated clock.
    pub latency: Rate,
    /// How long a slow call waits.
    pub latency_duration: Duration,
    /// How often `get` returns only a prefix of the object.
    pub partial: Rate,
    /// How often `put` reports success but stores nothing.
    pub lost_write: Rate,
}

#[derive(Debug)]
struct State {
    rng: SimRng,
    objects: BTreeMap<String, Vec<u8>>,
}

/// Objects in memory, with injected faults. Latency advances the shared [`SimClock`].
#[derive(Debug)]
pub struct SimObjectStore {
    faults: ObjectStoreFaults,
    clock: Arc<SimClock>,
    state: Mutex<State>,
}

impl SimObjectStore {
    /// Returns an empty store with the given seed, faults and clock.
    #[must_use]
    pub const fn new(seed: u64, faults: ObjectStoreFaults, clock: Arc<SimClock>) -> Self {
        Self {
            faults,
            clock,
            state: Mutex::new(State {
                rng: SimRng::new(seed),
                objects: BTreeMap::new(),
            }),
        }
    }

    /// Draws the latency and error faults of one call of `operation` on `key`.
    fn enter(&self, state: &mut State, operation: &str, key: &str) -> Result<(), PortError> {
        if state.rng.hits(self.faults.latency) {
            self.clock.sleep(self.faults.latency_duration);
        }
        if state.rng.hits(self.faults.error) {
            return Err(PortError::Failed(format!(
                "simulated object-store error on {operation} `{key}`"
            )));
        }
        Ok(())
    }
}

impl ObjectStore for SimObjectStore {
    fn get(&self, key: &str) -> Result<Vec<u8>, PortError> {
        let mut state = lock(&self.state);
        self.enter(&mut state, "get", key)?;
        let partial = state.rng.hits(self.faults.partial);
        let mut bytes = state
            .objects
            .get(key)
            .cloned()
            .ok_or_else(|| PortError::NotFound(key.to_owned()))?;
        if partial && !bytes.is_empty() {
            let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            let kept = usize::try_from(state.rng.below(length)).unwrap_or(0);
            bytes.truncate(kept);
        }
        Ok(bytes)
    }

    fn put(&self, key: &str, bytes: &[u8]) -> Result<(), PortError> {
        let mut state = lock(&self.state);
        self.enter(&mut state, "put", key)?;
        if !state.rng.hits(self.faults.lost_write) {
            state.objects.insert(key.to_owned(), bytes.to_vec());
        }
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), PortError> {
        let mut state = lock(&self.state);
        self.enter(&mut state, "delete", key)?;
        state
            .objects
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| PortError::NotFound(key.to_owned()))
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>, PortError> {
        let mut state = lock(&self.state);
        self.enter(&mut state, "list", prefix)?;
        Ok(state
            .objects
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect())
    }
}
