//! Random numbers seeded by the operating system.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Random;

/// Random numbers: SipHash, keyed by the operating system's randomness when the value is
/// created, of a counter. Fit for identifiers, not for secrets.
#[derive(Debug)]
pub struct RealRandom {
    keys: RandomState,
    counter: AtomicU64,
}

impl RealRandom {
    /// Returns a generator keyed by fresh randomness of the operating system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            keys: RandomState::new(),
            counter: AtomicU64::new(0),
        }
    }
}

impl Default for RealRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl Random for RealRandom {
    fn next_u64(&self) -> u64 {
        let mut hasher = self.keys.build_hasher();
        hasher.write_u64(self.counter.fetch_add(1, Ordering::Relaxed));
        hasher.finish()
    }

    fn fill(&self, bytes: &mut [u8]) {
        for chunk in bytes.chunks_mut(8) {
            let draw = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&draw[..chunk.len()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RealRandom;
    use crate::Random;

    #[test]
    fn real_random_draws_differ_and_fill_every_byte() {
        let random = RealRandom::new();
        assert_ne!(random.next_u64(), random.next_u64());
        assert_ne!(RealRandom::new().next_u64(), RealRandom::new().next_u64());
        let mut bytes = [0_u8; 13];
        random.fill(&mut bytes);
        let mut again = [0_u8; 13];
        random.fill(&mut again);
        assert_ne!(bytes, again);
    }
}
