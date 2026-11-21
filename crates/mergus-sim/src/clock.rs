//! The simulated clock: virtual time, clock jumps and skew.

use std::sync::Mutex;
use std::time::Duration;

use mergus_ports::{Clock, MonotonicTime, Timestamp};

use crate::rng::{Rate, SimRng, lock};

/// The clock faults to inject.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClockFaults {
    /// How often reading the wall clock first makes it jump.
    pub jump: Rate,
    /// How far a jump moves the wall clock, forwards or backwards at random.
    pub jump_size: Duration,
    /// How much faster (positive) or slower (negative) the wall clock runs than the
    /// monotonic clock, in parts per million.
    pub skew_per_million: i64,
}

#[derive(Debug)]
struct State {
    rng: SimRng,
    wall_nanos: i128,
    elapsed: Duration,
}

/// A clock whose time only moves when it sleeps or is advanced. Sleeping returns at once.
#[derive(Debug)]
pub struct SimClock {
    faults: ClockFaults,
    state: Mutex<State>,
}

impl SimClock {
    /// Returns a clock that starts at `start` with the given seed and faults.
    #[must_use]
    pub const fn new(seed: u64, start: Timestamp, faults: ClockFaults) -> Self {
        Self {
            faults,
            state: Mutex::new(State {
                rng: SimRng::new(seed),
                wall_nanos: start.unix_nanos(),
                elapsed: Duration::ZERO,
            }),
        }
    }

    /// Moves both clocks forward by `duration`, the wall clock with its skew.
    pub fn advance(&self, duration: Duration) {
        let mut state = lock(&self.state);
        state.elapsed = state.elapsed.saturating_add(duration);
        let nanos = i128::try_from(duration.as_nanos()).unwrap_or(i128::MAX);
        let skewed = nanos.saturating_add(
            nanos.saturating_mul(i128::from(self.faults.skew_per_million)) / 1_000_000,
        );
        state.wall_nanos = state.wall_nanos.saturating_add(skewed);
    }
}

impl Clock for SimClock {
    fn now(&self) -> Timestamp {
        let mut state = lock(&self.state);
        let jumps = state.rng.hits(self.faults.jump);
        let forwards = state.rng.hits(Rate::per_million(500_000));
        if jumps {
            let size = i128::try_from(self.faults.jump_size.as_nanos()).unwrap_or(i128::MAX);
            let delta = if forwards { size } else { -size };
            state.wall_nanos = state.wall_nanos.saturating_add(delta);
        }
        Timestamp::from_unix_nanos(state.wall_nanos)
    }

    fn monotonic(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(lock(&self.state).elapsed)
    }

    fn sleep(&self, duration: Duration) {
        self.advance(duration);
    }
}
