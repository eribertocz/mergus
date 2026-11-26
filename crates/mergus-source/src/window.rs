//! The time window of a source: full loads and backfills start only inside it, incremental
//! loads at any time (MG-REQ-0430).

use std::fmt;
use std::time::Duration;

use mergus_ports::{Clock, Timestamp};

/// The minutes of a day.
const DAY_MINUTES: u32 = 24 * 60;

/// The nanoseconds of a minute.
const MINUTE_NANOS: i128 = 60_000_000_000;

/// A daily window `HH:MM-HH:MM` in UTC, which may cross midnight. It opens at its start and
/// closes at its end; a window whose start is its end is open all day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Window {
    /// The minute of the day it opens at.
    start: u32,
    /// The minute of the day it closes at.
    end: u32,
}

/// What a load of a table reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadKind {
    /// The whole table.
    Full,
    /// A past range of the table, read again.
    Backfill,
    /// The rows changed since the last mark.
    Incremental,
}

impl Window {
    /// The window of `text`, written `HH:MM-HH:MM` in 24-hour time.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (start, end) = text.split_once('-')?;
        Some(Self {
            start: minute(start)?,
            end: minute(end)?,
        })
    }

    /// Whether the window is open at `now`.
    #[must_use]
    pub fn contains(&self, now: Timestamp) -> bool {
        let minute = minute_of_day(now);
        if self.start == self.end {
            return true;
        }
        if self.start < self.end {
            (self.start..self.end).contains(&minute)
        } else {
            minute >= self.start || minute < self.end
        }
    }

    /// How long after `now` the window opens; zero when it is open.
    #[must_use]
    pub fn until_open(&self, now: Timestamp) -> Duration {
        if self.contains(now) {
            return Duration::ZERO;
        }
        let nanos = now
            .unix_nanos()
            .rem_euclid(MINUTE_NANOS * i128::from(DAY_MINUTES));
        let target = i128::from(self.start) * MINUTE_NANOS;
        let wait = (target - nanos).rem_euclid(MINUTE_NANOS * i128::from(DAY_MINUTES));
        Duration::from_nanos(u64::try_from(wait).unwrap_or(u64::MAX))
    }
}

impl fmt::Display for Window {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:02}:{:02}-{:02}:{:02}",
            self.start / 60,
            self.start % 60,
            self.end / 60,
            self.end % 60
        )
    }
}

/// Waits on `clock` until a load of `kind` may start: a full load or a backfill waits for
/// `window` to open, an incremental load and a source without a window start at once
/// (MG-REQ-0430). Returns how long it waited.
pub fn wait_for_window(clock: &dyn Clock, window: Option<&Window>, kind: LoadKind) -> Duration {
    let Some(window) = window else {
        return Duration::ZERO;
    };
    if kind == LoadKind::Incremental {
        return Duration::ZERO;
    }
    let wait = window.until_open(clock.now());
    if !wait.is_zero() {
        clock.sleep(wait);
    }
    wait
}

/// The minute of the day of `HH:MM`.
fn minute(text: &str) -> Option<u32> {
    let (hours, minutes) = text.split_once(':')?;
    let (hours, minutes): (u32, u32) = (hours.parse().ok()?, minutes.parse().ok()?);
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

/// The minute of the day of `now`, in UTC.
fn minute_of_day(now: Timestamp) -> u32 {
    let minutes = now.unix_nanos().div_euclid(MINUTE_NANOS);
    u32::try_from(minutes.rem_euclid(i128::from(DAY_MINUTES))).unwrap_or(0)
}
