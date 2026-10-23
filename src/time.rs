//! Parsing of the ISO 8601 timestamps ListenBrainz returns.
//!
//! ListenBrainz reports when a generated playlist retires as a timestamp
//! without a zone, for example `2026-11-05T22:05:18.572700`, while the provider
//! interface counts in unix milliseconds. Everything here is pure and tested on
//! the host target.

/// Parses an ISO 8601 timestamp into unix milliseconds.
///
/// The accepted form is `YYYY-MM-DDTHH:MM:SS`, optionally followed by a
/// fractional second with up to nine digits and optionally by `Z` or a
/// `+HH:MM`/`-HH:MM` offset. A timestamp without a zone is read as UTC, which is
/// what ListenBrainz means by it.
///
/// # Parameters
///
/// - `value`: the timestamp to parse.
///
/// # Returns
///
/// The instant in unix milliseconds, or `None` when `value` is not a timestamp
/// of that form or names a date the calendar does not have.
///
/// # Example
///
/// ```text
/// parse_iso8601_ms("1970-01-01T00:00:00")        -> Some(0)
/// parse_iso8601_ms("1970-01-01T00:00:01.500")    -> Some(1500)
/// parse_iso8601_ms("1970-01-01T01:00:00+01:00")  -> Some(0)
/// parse_iso8601_ms("not a timestamp")            -> None
/// ```
pub(crate) fn parse_iso8601_ms(value: &str) -> Option<u64> {
    let bytes = value.as_bytes();
    if bytes.len() < 19 {
        return None;
    }

    let year = i64::from(parse_digits(&bytes[0..4])?);
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let month = parse_digits(&bytes[5..7])?;
    let day = parse_digits(&bytes[8..10])?;
    if !matches!(bytes[10], b'T' | b't' | b' ') {
        return None;
    }
    let hour = parse_digits(&bytes[11..13])?;
    if bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let minute = parse_digits(&bytes[14..16])?;
    let second = parse_digits(&bytes[17..19])?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let mut index = 19;
    let mut millis: u64 = 0;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return None;
        }
        let digits = &bytes[start..index];
        millis = fraction_millis(digits)?;
    }

    let zone_seconds = zone_offset(&bytes[index..])?;

    let days = days_from_civil(year, month, day);
    let seconds = days
        .checked_mul(86_400)?
        .checked_add(i64::from(hour) * 3_600)?
        .checked_add(i64::from(minute) * 60)?
        .checked_add(i64::from(second))?;
    let seconds = seconds.checked_sub(zone_seconds)?;
    let total = seconds.checked_mul(1_000)?.checked_add(millis as i64)?;

    u64::try_from(total).ok()
}

/// Reads a run of ASCII digits as an unsigned number.
///
/// # Parameters
///
/// - `digits`: the bytes to read.
///
/// # Returns
///
/// The number, or `None` when a byte is not a digit.
fn parse_digits(digits: &[u8]) -> Option<u32> {
    let mut value: u32 = 0;
    for digit in digits {
        if !digit.is_ascii_digit() {
            return None;
        }
        value = value
            .checked_mul(10)?
            .checked_add(u32::from(digit - b'0'))?;
    }
    Some(value)
}

/// Converts the fractional part of a timestamp to whole milliseconds.
///
/// # Parameters
///
/// - `digits`: the fractional digits, at least one.
///
/// # Returns
///
/// The fraction truncated to milliseconds, or `None` for more than nine
/// digits.
fn fraction_millis(digits: &[u8]) -> Option<u64> {
    if digits.len() > 9 {
        return None;
    }
    let mut value: u64 = parse_digits(digits).map(u64::from)?;
    let mut scale = digits.len();
    while scale < 3 {
        value *= 10;
        scale += 1;
    }
    while scale > 3 {
        value /= 10;
        scale -= 1;
    }
    Some(value)
}

/// Reads the optional zone designator at the end of a timestamp.
///
/// # Parameters
///
/// - `rest`: the bytes after the seconds.
///
/// # Returns
///
/// The offset east of UTC in seconds; zero for an empty or `Z` designator, and
/// `None` for anything else.
fn zone_offset(rest: &[u8]) -> Option<i64> {
    match rest {
        [] | [b'Z'] | [b'z'] => Some(0),
        [
            sign @ (b'+' | b'-'),
            hour_a,
            hour_b,
            b':',
            minute_a,
            minute_b,
        ] => {
            let hour = i64::from(*hour_a).checked_sub(i64::from(b'0'))? * 10
                + i64::from(*hour_b).checked_sub(i64::from(b'0'))?;
            let minute = i64::from(*minute_a).checked_sub(i64::from(b'0'))? * 10
                + i64::from(*minute_b).checked_sub(i64::from(b'0'))?;
            if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) {
                return None;
            }
            let seconds = hour * 3_600 + minute * 60;
            Some(if *sign == b'+' { seconds } else { -seconds })
        }
        _ => None,
    }
}

/// Returns the number of days between 1970-01-01 and the given civil date.
///
/// The algorithm is the one Howard Hinnant describes for `days_from_civil`; it
/// counts backwards from the start of the proleptic Gregorian calendar's 400
/// year era so that every year is handled uniformly.
///
/// # Parameters
///
/// - `year`: the year.
/// - `month`: the month, 1 to 12.
/// - `day`: the day of the month.
///
/// # Returns
///
/// The day count, negative before the epoch.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = i64::from((month + 9) % 12);
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_epoch_and_a_whole_second() {
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00"), Some(0));
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:01"), Some(1_000));
    }

    #[test]
    fn parses_a_fraction_to_milliseconds() {
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:01.5"), Some(1_500));
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:01.572700"), Some(1_572));
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00.000123456"), Some(0));
    }

    #[test]
    fn reads_zones_and_defaults_to_utc() {
        assert_eq!(parse_iso8601_ms("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(parse_iso8601_ms("1969-12-31T23:00:00-01:00"), Some(0));
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn parses_the_expiry_listenbrainz_returns() {
        // 2026-11-05T22:05:18.572700Z
        assert_eq!(
            parse_iso8601_ms("2026-11-05T22:05:18.572700"),
            Some(1_793_916_318_572)
        );
    }

    #[test]
    fn rejects_values_that_are_not_timestamps() {
        assert_eq!(parse_iso8601_ms(""), None);
        assert_eq!(parse_iso8601_ms("not a timestamp"), None);
        assert_eq!(parse_iso8601_ms("2026-13-05T00:00:00"), None);
        assert_eq!(parse_iso8601_ms("2026-11-05T25:00:00"), None);
        assert_eq!(parse_iso8601_ms("2026-11-05T22:05:18+2:00"), None);
        assert_eq!(parse_iso8601_ms("2026-11-05T22:05:18."), None);
    }

    #[test]
    fn day_counts_match_the_gregorian_calendar() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(2026, 11, 5), 20_762);
    }
}
