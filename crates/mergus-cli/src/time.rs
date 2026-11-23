//! RFC 3339 timestamps in UTC with milliseconds (MG-REQ-0296.3).

use mergus_ports::Timestamp;

/// Formats `time` as `YYYY-MM-DDTHH:MM:SS.mmmZ`, rounded down to the millisecond.
pub(crate) fn rfc3339(time: Timestamp) -> String {
    let millis = time.unix_nanos().div_euclid(1_000_000);
    let seconds = millis.div_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3_600,
        of_day / 60 % 60,
        of_day % 60,
        millis.rem_euclid(1_000)
    )
}

/// The proleptic Gregorian date of the day `days` after 1970-01-01 (H. Hinnant's
/// `civil_from_days`).
fn civil(days: i128) -> (i128, i128, i128) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let of_era = shifted.rem_euclid(146_097);
    let year_of_era = (of_era - of_era / 1_460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * of_year + 2) / 153;
    let day = of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i128::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use mergus_ports::Timestamp;

    use super::rfc3339;

    #[test]
    fn timestamps_are_utc_with_milliseconds() {
        let at = |nanos| rfc3339(Timestamp::from_unix_nanos(nanos));
        assert_eq!(at(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(at(1_795_389_314_123_999_999), "2026-11-22T23:15:14.123Z");
        assert_eq!(at(951_782_400_000_000_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(at(-1), "1969-12-31T23:59:59.999Z");
    }
}
