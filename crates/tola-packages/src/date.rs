//! Calendar dates and timestamps shared by metadata schemas and publication formats.

use std::fmt;

use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};
use typst::foundations::{Datetime, Value};

/// A date or timestamp that cannot be interpreted under the supported calendar rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateError(&'static str);

impl fmt::Display for DateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for DateError {}

/// Parse a native date or a strict `YYYY-MM-DD` string, with a year from 1 through 9999.
///
/// The result is a pure date. Values carrying a time are rejected rather than truncated.
pub fn parse_date(value: &Value) -> Result<Datetime, DateError> {
    match value {
        Value::Datetime(Datetime::Date(date)) if (1..=9999).contains(&date.year()) => {
            Ok(Datetime::Date(*date))
        }
        Value::Str(text) => {
            let date = calendar_date(text.as_str())?;
            Datetime::from_ymd(date.year(), date.month() as u8, date.day() as u8).ok_or(DateError(
                "expected a calendar date with a year from 1 through 9999",
            ))
        }
        _ => Err(DateError(
            "expected a calendar date without a time, such as 2026-09-01",
        )),
    }
}

/// A timestamp retaining its local spelling and arbitrarily precise fractional second.
///
/// Whole UTC seconds and the normalized fractional digits form an exact ordering key. Fixed
/// offset conversion for existing feed serializers is the only operation that truncates to
/// nanoseconds.
#[derive(Debug)]
pub struct Timestamp {
    rfc3339: String,
    whole: DateTime<FixedOffset>,
    subsecond: String,
}

impl Timestamp {
    /// Parse a strict RFC 3339 string or a native datetime with a complete calendar date.
    ///
    /// Years are 1 through 9999, `T`/`t` separates the date and time, and the timezone is `Z`/`z`
    /// or `±hh:mm`. Leap seconds and surrounding whitespace are rejected. Native datetimes use
    /// UTC; a pure native date uses midnight, while a time without a date is rejected.
    pub fn parse(value: &Value) -> Result<Self, DateError> {
        match value {
            Value::Str(text) => Self::from_text(text.as_str()),
            Value::Datetime(datetime) => {
                let (date, time) = match datetime {
                    Datetime::Date(date) => (*date, None),
                    Datetime::Datetime(datetime) => (datetime.date(), Some(datetime.time())),
                    Datetime::Time(_) => {
                        return Err(DateError("a timestamp needs a year, month, and day"));
                    }
                };
                let (hour, minute, second, nanosecond) = time
                    .map(|time| (time.hour(), time.minute(), time.second(), time.nanosecond()))
                    .unwrap_or((0, 0, 0, 0));
                let mut text = format!(
                    "{:04}-{:02}-{:02}T{hour:02}:{minute:02}:{second:02}",
                    date.year(),
                    date.month() as u8,
                    date.day(),
                );
                if nanosecond != 0 {
                    text.push('.');
                    text.push_str(format!("{nanosecond:09}").trim_end_matches('0'));
                }
                text.push('Z');
                Self::from_text(&text)
            }
            _ => Err(DateError(
                "expected a calendar datetime or an RFC 3339 timestamp with timezone",
            )),
        }
    }

    fn from_text(text: &str) -> Result<Self, DateError> {
        let invalid =
            DateError("expected an RFC 3339 timestamp with timezone, such as 2026-09-01T12:00:00Z");
        let bytes = text.as_bytes();
        if bytes.len() < 20
            || bytes[..19]
                .iter()
                .enumerate()
                .any(|(index, byte)| match index {
                    4 | 7 => *byte != b'-',
                    10 => !matches!(byte, b'T' | b't'),
                    13 | 16 => *byte != b':',
                    _ => !byte.is_ascii_digit(),
                })
        {
            return Err(invalid);
        }
        // The fixed-width prefix is ASCII before any string slicing.
        if decimal(&bytes[..4]) == 0 {
            return Err(DateError("the calendar year must be from 1 through 9999"));
        }
        if decimal(&bytes[17..19]) > 59 {
            return Err(DateError(
                "timestamp seconds must be from 00 through 59; leap seconds are unsupported",
            ));
        }
        let suffix = &text[19..];
        let (fraction, zone) = if let Some(fraction) = suffix.strip_prefix('.') {
            let length = fraction.bytes().take_while(u8::is_ascii_digit).count();
            if length == 0 {
                return Err(invalid);
            }
            (&fraction[..length], &fraction[length..])
        } else {
            ("", suffix)
        };
        if !valid_zone(zone) {
            return Err(invalid);
        }
        // Validate the whole timestamp separately so Chrono never discards fractional digits.
        let whole = DateTime::parse_from_rfc3339(&format!("{}{zone}", &text[..19]))
            .map_err(|_| DateError("expected a valid calendar date, clock time, and timezone"))?;
        Ok(Self {
            rfc3339: text.to_owned(),
            whole,
            subsecond: fraction.trim_end_matches('0').to_owned(),
        })
    }

    /// The original valid RFC 3339 text, or UTC text generated from a native datetime.
    pub fn rfc3339(&self) -> &str {
        &self.rfc3339
    }

    /// Whole seconds since the Unix epoch, interpreted in UTC.
    pub fn unix_seconds(&self) -> i64 {
        self.whole.timestamp()
    }

    /// Fractional second digits without a decimal point or trailing zeros; empty for zero.
    pub fn subsecond(&self) -> &str {
        &self.subsecond
    }

    /// Convert for feed serializers, preserving the offset and truncating beyond nanoseconds.
    ///
    /// The timestamp's text and ordering digits retain their full precision.
    pub fn to_fixed(&self) -> DateTime<FixedOffset> {
        let length = self.subsecond.len().min(9);
        let nanosecond =
            decimal(&self.subsecond.as_bytes()[..length]) * 10_u32.pow(9 - length as u32);
        self.whole
            .with_nanosecond(nanosecond)
            .expect("a fractional second is below one billion nanoseconds")
    }
}

fn calendar_date(text: &str) -> Result<NaiveDate, DateError> {
    let invalid = DateError("expected a valid calendar date in YYYY-MM-DD form");
    let bytes = text.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| !matches!(index, 4 | 7) && !byte.is_ascii_digit())
    {
        return Err(invalid);
    }
    let year = decimal(&bytes[..4]);
    if year == 0 {
        return Err(DateError("the calendar year must be from 1 through 9999"));
    }
    NaiveDate::from_ymd_opt(year as i32, decimal(&bytes[5..7]), decimal(&bytes[8..10]))
        .ok_or(invalid)
}

fn valid_zone(zone: &str) -> bool {
    match zone.as_bytes() {
        [b'Z' | b'z'] => true,
        [sign, hour_tens, hour_units, b':', minute_tens, minute_units] => {
            matches!(sign, b'+' | b'-')
                && [hour_tens, hour_units, minute_tens, minute_units]
                    .iter()
                    .all(|byte| byte.is_ascii_digit())
                && decimal(&zone.as_bytes()[1..3]) <= 23
                && decimal(&zone.as_bytes()[4..6]) <= 59
        }
        _ => false,
    }
}

fn decimal(digits: &[u8]) -> u32 {
    digits
        .iter()
        .fold(0, |number, digit| number * 10 + u32::from(digit - b'0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Value {
        Value::Str(value.into())
    }

    #[test]
    fn dates_preserve_calendar_values() {
        for (source, year, month, day) in [
            ("0001-01-01", 1, 1, 1),
            ("2000-02-29", 2000, 2, 29),
            ("2024-02-29", 2024, 2, 29),
            ("9999-12-31", 9999, 12, 31),
        ] {
            let expected = Datetime::from_ymd(year, month, day).unwrap();
            assert_eq!(parse_date(&text(source)).unwrap(), expected);
            assert_eq!(parse_date(&Value::Datetime(expected)).unwrap(), expected);
        }
    }

    #[test]
    fn dates_reject_invalid_values() {
        for source in [
            "1900-02-29",
            "2026-02-30",
            "0000-01-01",
            "10000-01-01",
            "2026-1-01",
            " 2026-01-01",
            "2026-01-01 ",
            "2026-01-01T00:00:00Z",
        ] {
            assert!(parse_date(&text(source)).is_err(), "{source}");
        }
        for datetime in [
            Datetime::from_ymd(0, 1, 1).unwrap(),
            Datetime::from_hms(1, 2, 3).unwrap(),
            Datetime::from_ymd_hms(2026, 1, 1, 1, 2, 3).unwrap(),
        ] {
            assert!(parse_date(&Value::Datetime(datetime)).is_err());
        }
    }

    #[test]
    fn timestamps_keep_local_offsets() {
        let source = "1970-01-01t00:30:00+01:00";
        let timestamp = Timestamp::parse(&text(source)).unwrap();
        assert_eq!(timestamp.rfc3339(), source);
        assert_eq!(timestamp.unix_seconds(), -1800);
        assert_eq!(timestamp.to_fixed().offset().local_minus_utc(), 3600);
        for (source, seconds) in [
            ("1970-01-01T00:00:00+23:59", -86_340),
            ("1970-01-01T00:00:00-23:59", 86_340),
            ("1970-01-01t00:00:00z", 0),
            ("1970-01-01T00:00:00-00:00", 0),
        ] {
            let timestamp = Timestamp::parse(&text(source)).unwrap();
            assert_eq!(timestamp.rfc3339(), source);
            assert_eq!(timestamp.unix_seconds(), seconds);
        }
    }

    #[test]
    fn fractions_preserve_exact_order() {
        let earlier =
            Timestamp::parse(&text("2026-10-11T08:00:00.000000000000000001+08:00")).unwrap();
        let later = Timestamp::parse(&text("2026-10-11T00:00:00.000000000000000002Z")).unwrap();
        assert!(
            (earlier.unix_seconds(), earlier.subsecond())
                < (later.unix_seconds(), later.subsecond())
        );
        assert_eq!(earlier.subsecond(), "000000000000000001");
        assert_eq!(earlier.to_fixed(), later.to_fixed());
        let source = "2026-01-01T00:00:00.123456789987000Z";
        let timestamp = Timestamp::parse(&text(source)).unwrap();
        assert_eq!(timestamp.rfc3339(), source);
        assert_eq!(timestamp.subsecond(), "123456789987");
        assert_eq!(timestamp.to_fixed().nanosecond(), 123_456_789);
        let zero = Timestamp::parse(&text("2026-01-01T00:00:00.000Z")).unwrap();
        assert_eq!(zero.subsecond(), "");
        assert_eq!(zero.to_fixed().nanosecond(), 0);
    }

    #[test]
    fn native_dates_use_utc_midnight() {
        let timestamp =
            Timestamp::parse(&Value::Datetime(Datetime::from_ymd(2024, 2, 29).unwrap())).unwrap();
        assert_eq!(timestamp.rfc3339(), "2024-02-29T00:00:00Z");
        assert_eq!(timestamp.subsecond(), "");
    }

    #[test]
    fn native_nanoseconds_survive_conversion() {
        let Datetime::Datetime(datetime) = Datetime::from_ymd_hms(2026, 1, 2, 3, 4, 5).unwrap()
        else {
            unreachable!();
        };
        let value = Value::Datetime(Datetime::Datetime(
            datetime.replace_nanosecond(123_456_780).unwrap(),
        ));
        let timestamp = Timestamp::parse(&value).unwrap();
        assert_eq!(timestamp.rfc3339(), "2026-01-02T03:04:05.12345678Z");
        assert_eq!(timestamp.subsecond(), "12345678");
        assert_eq!(timestamp.to_fixed().nanosecond(), 123_456_780);
    }

    #[test]
    fn timestamps_reject_invalid_components() {
        for source in [
            "2026-01-01",
            "00:00:00Z",
            "0000-01-01T00:00:00Z",
            "10000-01-01T00:00:00Z",
            "2026-02-30T00:00:00Z",
            "1900-02-29T00:00:00Z",
            "2026-01-01 00:00:00Z",
            "2026-1-01T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:60:00Z",
            "2016-12-31T23:59:60Z",
            "2026-01-01T00:00:00.Z",
            "2026-01-01T00:00:00.12xZ",
            "2026-01-01T00:00:00.1234567890000000000xZ",
            "2026-01-01T00:00:00",
            "2026-01-01T00:00:00+24:00",
            "2026-01-01T00:00:00+23:60",
            "2026-01-01T00:00:00+0800",
            " 2026-01-01T00:00:00Z",
            "2026-01-01T00:00:00Z\n",
            "2026-01-01T00:00:00🦀",
            "日期日期日期日期日期T00:00:00Z",
        ] {
            assert!(Timestamp::parse(&text(source)).is_err(), "{source}");
        }
        assert!(Timestamp::parse(&Value::Datetime(Datetime::from_hms(1, 2, 3).unwrap())).is_err());
        assert!(Timestamp::parse(&Value::None).is_err());
    }
}
