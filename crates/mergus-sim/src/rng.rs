//! The seeded generator behind every simulated port, and fault rates.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// How often a fault happens: a probability in parts per million.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rate {
    per_million: u32,
}

impl Rate {
    /// The fault never happens.
    pub const NEVER: Self = Self { per_million: 0 };
    /// The fault happens on every call.
    pub const ALWAYS: Self = Self {
        per_million: 1_000_000,
    };

    /// Returns a rate of `per_million` calls in a million, capped at every call.
    #[must_use]
    pub const fn per_million(per_million: u32) -> Self {
        if per_million > 1_000_000 {
            Self::ALWAYS
        } else {
            Self { per_million }
        }
    }
}

/// A SplitMix64 generator: small, fast and the same on every platform.
#[derive(Debug)]
pub(crate) struct SimRng {
    state: u64,
}

impl SimRng {
    /// Returns a generator started from `seed`.
    pub(crate) const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next number.
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a number below `bound`, or 0 when `bound` is 0.
    pub(crate) fn below(&mut self, bound: u64) -> u64 {
        let draw = self.next_u64();
        if bound == 0 { 0 } else { draw % bound }
    }

    /// Returns whether a fault of `rate` happens now. It always draws one number, so the
    /// sequence does not depend on the rates.
    pub(crate) fn hits(&mut self, rate: Rate) -> bool {
        self.below(1_000_000) < u64::from(rate.per_million)
    }
}

/// Locks `mutex`. A simulated port holds no invariant a panic can break, so a poisoned lock
/// is used as it is.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::{Rate, SimRng};

    #[test]
    fn same_seed_gives_the_same_sequence() {
        let mut left = SimRng::new(7);
        let mut right = SimRng::new(7);
        for _ in 0..100 {
            assert_eq!(left.next_u64(), right.next_u64());
        }
        assert_ne!(SimRng::new(7).next_u64(), SimRng::new(8).next_u64());
    }

    #[test]
    fn never_and_always_rates_are_exact() {
        let mut rng = SimRng::new(1);
        for _ in 0..1_000 {
            assert!(!rng.hits(Rate::NEVER));
            assert!(rng.hits(Rate::ALWAYS));
        }
        assert_eq!(Rate::per_million(2_000_000), Rate::ALWAYS);
    }
}
