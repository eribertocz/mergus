//! The text of incremental values: the text DuckDB gives for `CAST(value AS VARCHAR)`
//! (MG-REQ-0413), and the overlap subtracted from `DATE`, `TIMESTAMP` and `TIMESTAMPTZ`
//! columns (MG-REQ-0412.2).

use std::time::Duration;

use arrow::array::{Array, AsArray};
use arrow::compute::cast;
use arrow::datatypes::{DataType, Date32Type, Date64Type, TimeUnit, TimestampMicrosecondType};
use mergus_ports::PortError;

/// Microseconds in a day.
const DAY: i64 = 86_400_000_000;

/// An incremental column the overlap is subtracted from (MG-REQ-0412.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Temporal {
    /// `DATE`.
    Date,
    /// `TIMESTAMP`, read as UTC.
    Timestamp,
    /// `TIMESTAMPTZ`.
    TimestampTz,
}

impl Temporal {
    /// The temporal kind of the DuckDB type `column_type`, as `DESCRIBE` names it; `None`
    /// for any other type, which is read from `>= watermark` (MG-REQ-0412.2).
    #[must_use]
    pub fn of(column_type: &str) -> Option<Self> {
        match column_type.trim().to_ascii_uppercase().as_str() {
            "DATE" => Some(Self::Date),
            "TIMESTAMP" => Some(Self::Timestamp),
            "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" => Some(Self::TimestampTz),
            _ => None,
        }
    }

    /// The microseconds since the epoch, in UTC, of the DuckDB text `text` of a value of this
    /// kind; `None` when it is not such a text.
    #[must_use]
    pub fn parse(self, text: &str) -> Option<i64> {
        let text = text.trim();
        let (date, rest) = text.split_at_checked(10)?;
        let day = parse_date(date)?;
        if self == Self::Date {
            return rest.is_empty().then_some(day * DAY);
        }
        let rest = rest.strip_prefix([' ', 'T'])?;
        let (time, offset) = match rest.find(['+', '-', 'Z']) {
            Some(at) => rest.split_at(at),
            None => (rest, ""),
        };
        let offset = parse_offset(offset)?;
        Some(day * DAY + parse_time(time)? - offset)
    }

    /// The DuckDB text of `micros`, microseconds since the epoch in UTC, as a value of this
    /// kind: `2026-01-02`, `2026-01-02 03:04:05.5` or `2026-01-02 03:04:05.5+00`.
    #[must_use]
    pub fn format(self, micros: i64) -> String {
        let day = micros.div_euclid(DAY);
        let (year, month, date) = civil(day);
        let day_text = format!("{year:04}-{month:02}-{date:02}");
        if self == Self::Date {
            return day_text;
        }
        let time = micros.rem_euclid(DAY);
        let seconds = time / 1_000_000;
        let mut text = format!(
            "{day_text} {:02}:{:02}:{:02}",
            seconds / 3_600,
            seconds / 60 % 60,
            seconds % 60
        );
        let fraction = time % 1_000_000;
        if fraction != 0 {
            let digits = format!("{fraction:06}");
            text.push('.');
            text.push_str(digits.trim_end_matches('0'));
        }
        if self == Self::TimestampTz {
            text.push_str("+00");
        }
        text
    }

    /// The text of `watermark − overlap` for a column of this kind (MG-REQ-0412.2). From a
    /// `DATE` the overlap is subtracted as whole days, rounded up to the next day. `None` when
    /// `watermark` is not a text of this kind.
    #[must_use]
    pub fn minus(self, watermark: &str, overlap: Duration) -> Option<String> {
        let mut overlap = i64::try_from(overlap.as_micros()).ok()?;
        if self == Self::Date {
            overlap = overlap.div_euclid(DAY) + i64::from(overlap.rem_euclid(DAY) != 0);
            overlap = overlap.checked_mul(DAY)?;
        }
        Some(self.format(self.parse(watermark)?.checked_sub(overlap)?))
    }
}

/// The value at `row` of `column` as a text DuckDB casts back to the column's type: dates
/// and timestamps in DuckDB's form, any other type as Arrow writes it. It is bound to the
/// query that gives DuckDB's own text of the value (MG-REQ-0413).
pub(crate) fn bound_text(column: &dyn Array, row: usize) -> Result<String, PortError> {
    let failed = |error: arrow::error::ArrowError| {
        PortError::Failed(format!(
            "cannot write an incremental value as text: {error}"
        ))
    };
    let micros = |column: &dyn Array| -> Result<i64, PortError> {
        let utc = DataType::Timestamp(TimeUnit::Microsecond, None);
        let cast = cast(&column.slice(row, 1), &utc).map_err(failed)?;
        Ok(cast.as_primitive::<TimestampMicrosecondType>().value(0))
    };
    let text = match column.data_type() {
        DataType::Date32 => {
            let day = i64::from(column.as_primitive::<Date32Type>().value(row));
            Temporal::Date.format(day * DAY)
        }
        DataType::Date64 => {
            let millis = column.as_primitive::<Date64Type>().value(row);
            Temporal::Date.format(millis.saturating_mul(1_000))
        }
        DataType::Timestamp(_, None) => Temporal::Timestamp.format(micros(column)?),
        DataType::Timestamp(_, Some(_)) => {
            let naive = strip_zone(column).map_err(failed)?;
            Temporal::TimestampTz.format(micros(naive.as_ref())?)
        }
        _ => {
            let text = cast(&column.slice(row, 1), &DataType::Utf8).map_err(failed)?;
            text.as_string::<i32>().value(0).to_owned()
        }
    };
    Ok(text)
}

/// `column`, a timestamp with a time zone, as the same instants without one.
fn strip_zone(column: &dyn Array) -> Result<arrow::array::ArrayRef, arrow::error::ArrowError> {
    let micros = cast(
        column,
        &DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
    )?;
    let raw = cast(&micros, &DataType::Int64)?;
    cast(&raw, &DataType::Timestamp(TimeUnit::Microsecond, None))
}

/// The days since the epoch of `YYYY-MM-DD`.
fn parse_date(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    let year: i64 = text.get(..4)?.parse().ok()?;
    let month: i64 = text.get(5..7)?.parse().ok()?;
    let day: i64 = text.get(8..10)?.parse().ok()?;
    ((1..=12).contains(&month) && (1..=31).contains(&day)).then(|| days(year, month, day))
}

/// The microseconds of `HH:MM:SS[.ffffff]`.
fn parse_time(text: &str) -> Option<i64> {
    let (clock, fraction) = text.split_once('.').unwrap_or((text, ""));
    let mut parts = clock.split(':');
    let hours: i64 = parts.next()?.parse().ok()?;
    let minutes: i64 = parts.next()?.parse().ok()?;
    let seconds: i64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || fraction.len() > 6 || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let micros: i64 = format!("{fraction:0<6}").parse().ok()?;
    Some(((hours * 60 + minutes) * 60 + seconds) * 1_000_000 + micros)
}

/// The microseconds east of UTC of an offset `+HH`, `-HH:MM`, `+HH:MM:SS` or `Z`; zero when
/// there is none.
fn parse_offset(text: &str) -> Option<i64> {
    if text.is_empty() || text == "Z" {
        return Some(0);
    }
    let (sign, rest) = match text.split_at_checked(1)? {
        ("+", rest) => (1, rest),
        ("-", rest) => (-1, rest),
        _ => return None,
    };
    let mut seconds = 0;
    for (index, part) in rest.split(':').enumerate() {
        let value: i64 = part.parse().ok()?;
        let unit = match index {
            0 => 3_600,
            1 => 60,
            2 => 1,
            _ => return None,
        };
        seconds += value * unit;
    }
    Some(sign * seconds * 1_000_000)
}

/// The days since 1970-01-01 of a date of the proleptic Gregorian calendar.
fn days(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let shifted = (month + 9) % 12;
    let of_year = (153 * shifted + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_cycle - 719_468
}

/// The year, month and day of `days` since 1970-01-01.
fn civil(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let of_era = shifted - era * 146_097;
    let year_of_era = (of_era - of_era / 1_460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
