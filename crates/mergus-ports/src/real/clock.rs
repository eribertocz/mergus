//! The system clock.

use std::time::{Duration, Instant, SystemTime};

use crate::{Clock, MonotonicTime, Timestamp};

/// The operating system's clock. Its monotonic origin is the moment it was created.
#[derive(Clone, Copy, Debug)]
pub struct RealClock {
    origin: Instant,
}

impl RealClock {
    /// Returns a clock whose monotonic origin is now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for RealClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for RealClock {
    fn now(&self) -> Timestamp {
        let nanos = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(after) => i128::try_from(after.as_nanos()).unwrap_or(i128::MAX),
            Err(before) => i128::try_from(before.duration().as_nanos()).map_or(i128::MIN, |n| -n),
        };
        Timestamp::from_unix_nanos(nanos)
    }

    fn monotonic(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(self.origin.elapsed())
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RealClock;
    use crate::Clock;

    #[test]
    fn real_clock_is_after_2020_and_monotonic_time_grows() {
        let clock = RealClock::new();
        assert!(clock.now().unix_nanos() > 1_577_836_800_000_000_000);
        let before = clock.monotonic();
        clock.sleep(Duration::from_millis(1));
        assert!(clock.monotonic() > before);
    }
}
