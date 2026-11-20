//! The `Clock` port: current time, monotonic time and sleeping.

use std::time::Duration;

/// A wall-clock time: nanoseconds since 1970-01-01T00:00:00Z, negative before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    nanos: i128,
}

impl Timestamp {
    /// Returns the time that lies `nanos` nanoseconds after the Unix epoch.
    #[must_use]
    pub const fn from_unix_nanos(nanos: i128) -> Self {
        Self { nanos }
    }

    /// Returns the nanoseconds since the Unix epoch.
    #[must_use]
    pub const fn unix_nanos(self) -> i128 {
        self.nanos
    }
}

/// A monotonic time: the time elapsed since an origin fixed by the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonotonicTime {
    elapsed: Duration,
}

impl MonotonicTime {
    /// Returns the monotonic time that lies `elapsed` after the clock's origin.
    #[must_use]
    pub const fn from_elapsed(elapsed: Duration) -> Self {
        Self { elapsed }
    }

    /// Returns the time elapsed since the clock's origin.
    #[must_use]
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }
}

/// Current time, monotonic time and sleeping.
pub trait Clock: Send + Sync {
    /// Returns the current wall-clock time. It may jump backwards or forwards.
    fn now(&self) -> Timestamp;

    /// Returns the current monotonic time. It never decreases.
    fn monotonic(&self) -> MonotonicTime;

    /// Waits for `duration`.
    fn sleep(&self, duration: Duration);
}
