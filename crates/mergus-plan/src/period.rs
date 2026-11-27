//! Periods: half-open UTC ranges of one grain of a `partition_by` column (MG-REQ-0652.1).

use std::fmt;

/// Seconds in a day.
const DAY: i64 = 86_400;

/// The first instant the planner handles: `0001-01-01T00:00:00Z`.
const FIRST: i64 = -62_135_596_800;

/// The first instant after the last the planner handles: `9999-01-01T00:00:00Z`, so the end
/// of every period it makes still has a four-digit year.
const BEYOND: i64 = 253_370_764_800;

/// An instant in UTC, to the second, from `0001-01-01T00:00:00Z` to before
/// `9999-01-01T00:00:00Z`. It shows as an RFC 3339 timestamp, `YYYY-MM-DDTHH:MM:SSZ`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(i64);

impl Instant {
    /// The instant `seconds` after `1970-01-01T00:00:00Z`, or `None` outside the years the
    /// planner handles.
    #[must_use]
    pub fn from_unix_seconds(seconds: i64) -> Option<Self> {
        (FIRST..BEYOND).contains(&seconds).then_some(Self(seconds))
    }

    /// Midnight UTC of `year-month-day`, as a `DATE` value is read (MG-REQ-0652.1), or
    /// `None` when that is no date the planner handles.
    #[must_use]
    pub fn from_date(year: i32, month: u32, day: u32) -> Option<Self> {
        let last = days_in_month(i64::from(year), month)?;
        if day == 0 || day > last {
            return None;
        }
        let days = days_from_civil(i64::from(year), i64::from(month), i64::from(day));
        Self::from_unix_seconds(days.checked_mul(DAY)?)
    }

    /// The seconds since `1970-01-01T00:00:00Z`.
    #[must_use]
    pub fn unix_seconds(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Instant {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (days, seconds) = (self.0.div_euclid(DAY), self.0.rem_euclid(DAY));
        let (year, month, day) = civil_from_days(days);
        write!(
            formatter,
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
            seconds / 3600,
            seconds % 3600 / 60,
            seconds % 60
        )
    }
}

/// The half-open range `[start, end)` of instants, `start` before `end`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Range {
    start: Instant,
    end: Instant,
}

impl Range {
    /// The range `[start, end)`, or `None` when `start` is not before `end`.
    #[must_use]
    pub fn new(start: Instant, end: Instant) -> Option<Self> {
        (start < end).then_some(Self { start, end })
    }

    /// Its first instant.
    #[must_use]
    pub fn start(self) -> Instant {
        self.start
    }

    /// The first instant after it.
    #[must_use]
    pub fn end(self) -> Instant {
        self.end
    }
}

/// The grain of a `partition_by` column (MG-REQ-0366.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Grain {
    /// `day`.
    Day,
    /// `week`, from Monday (ISO 8601).
    Week,
    /// `month`.
    Month,
    /// `quarter`, from January, April, July and October.
    Quarter,
    /// `year`.
    Year,
}

impl Grain {
    /// The grain named `name` in `partition_by`.
    pub(crate) fn named(name: &str) -> Option<Self> {
        match name {
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            "quarter" => Some(Self::Quarter),
            "year" => Some(Self::Year),
            _ => None,
        }
    }

    /// The period of this grain that holds `at` (MG-REQ-0652.1).
    #[must_use]
    pub fn period(self, at: Instant) -> Range {
        let today = at.0.div_euclid(DAY);
        let (year, month, _) = civil_from_days(today);
        let (start, end) = match self {
            Self::Day => (today, today + 1),
            Self::Week => {
                // 1970-01-01 was a Thursday: day 0 is weekday 3 counted from Monday.
                let monday = today - (today + 3).rem_euclid(7);
                (monday, monday + 7)
            }
            Self::Month => months(year, month, 1),
            Self::Quarter => months(year, (month - 1) / 3 * 3 + 1, 3),
            Self::Year => months(year, 1, 12),
        };
        Range {
            start: Instant(start * DAY),
            end: Instant(end * DAY),
        }
    }
}

/// The days of `count` months from the first of `year-month`, as `(first, after)`.
fn months(year: i64, month: i64, count: i64) -> (i64, i64) {
    let after = month - 1 + count;
    (
        days_from_civil(year, month, 1),
        days_from_civil(year + after / 12, after % 12 + 1, 1),
    )
}

/// The days in `year-month`, or `None` for a month that is not 1 to 12.
fn days_in_month(year: i64, month: u32) -> Option<u32> {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if leap { 29 } else { 28 }),
        _ => None,
    }
}

/// The days from `1970-01-01` to `year-month-day` of the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let shifted = (month + 9) % 12;
    let of_year = (153 * shifted + 2) / 5 + day - 1;
    let of_era_days = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_era_days - 719_468
}

/// The `(year, month, day)` that is `days` after `1970-01-01`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let of_era = days - era * 146_097;
    let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted + 2) / 5 + 1;
    let month = if shifted < 10 {
        shifted + 3
    } else {
        shifted - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The ranges of `ranges`, those that overlap or touch merged, sorted by start
/// (MG-REQ-0652.3).
pub(crate) fn merge(mut ranges: Vec<Range>) -> Vec<Range> {
    ranges.sort();
    let mut merged: Vec<Range> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}
