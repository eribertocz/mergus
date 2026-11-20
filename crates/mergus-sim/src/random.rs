//! Seeded random numbers.

use std::sync::Mutex;

use mergus_ports::Random;

use crate::rng::{SimRng, lock};

/// Random numbers drawn from a seed.
#[derive(Debug)]
pub struct SimRandom {
    rng: Mutex<SimRng>,
}

impl SimRandom {
    /// Returns a generator started from `seed`.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            rng: Mutex::new(SimRng::new(seed)),
        }
    }
}

impl Random for SimRandom {
    fn next_u64(&self) -> u64 {
        lock(&self.rng).next_u64()
    }

    fn fill(&self, bytes: &mut [u8]) {
        let mut rng = lock(&self.rng);
        for chunk in bytes.chunks_mut(8) {
            let draw = rng.next_u64().to_le_bytes();
            chunk.copy_from_slice(&draw[..chunk.len()]);
        }
    }
}
