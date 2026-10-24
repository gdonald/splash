//! Timestamps
//!
//! Log timestamps are read into seconds since the Unix epoch, in UTC. A
//! timestamp with no zone is read in the zone it is handed, and one with no
//! year, as in syslog's `Oct  3 12:00:01`, is placed in the latest year that
//! does not put it more than a day after the current time.
use crate::zone::Zone;
use regex::Regex;
use std::fmt;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const SECONDS_PER_DAY: i64 = 86_400;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The days from 1970-01-01 to a date in the proleptic Gregorian calendar
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}

/// The year, month, and day a count of days from 1970-01-01 falls on
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;

    (year_of_era + era * 400 + i64::from(month <= 2), month, day)
}

/// The day of the week a count of days from 1970-01-01 falls on, 0 for Sunday
pub fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

/// The number of days in a month
pub fn days_in_month(year: i64, month: u32) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The seconds since the epoch the system clock reads
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// A wall clock date and time, before a zone places it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Civil {
    /// A date and time, or `None` when a field is out of range
    pub fn new(
        year: i64,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> Option<Civil> {
        let valid = (1..=12).contains(&month)
            && day >= 1
            && i64::from(day) <= days_in_month(year, month)
            && hour < 24
            && minute < 60
            && second < 61;

        valid.then_some(Civil {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// The date and time a count of seconds since the epoch reads as
    pub fn from_seconds(seconds: i64) -> Civil {
        let (year, month, day) = civil_from_days(seconds.div_euclid(SECONDS_PER_DAY));
        let time = seconds.rem_euclid(SECONDS_PER_DAY) as u32;

        Civil {
            year,
            month,
            day,
            hour: time / 3600,
            minute: time / 60 % 60,
            second: time % 60,
        }
    }

    /// The seconds since the epoch this date and time reads as, taken as UTC
    pub fn seconds(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day) * SECONDS_PER_DAY
            + i64::from(self.hour * 3600 + self.minute * 60 + self.second)
    }
}

/// `2023-10-03T12:00:01.123Z`, with a space or `T`, a fraction after `.` or
/// `,`, and a zone of `Z`, `UTC`, `GMT`, or an offset in hours or hours and
/// minutes, any of them optional. RFC 3339 allows a lowercase `t` and `z`.
/// journalctl's `Tue 2023-10-03 12:00:01 UTC` opens with a weekday, and
/// nginx writes the date as `2023/10/03`.
static ISO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^(?:[A-Z][a-z]{2}\ )?
        (\d{4})[-/](\d{2})[-/](\d{2})
        [Tt\ ](\d{2}):(\d{2})(?::(\d{2}))?
        (?:[.,]\d+)?
        \ ?(Z|z|UTC|GMT|[-+]\d{2}(?::?\d{2})?)?
        ",
    )
    .unwrap()
});

/// ISO 8601's basic form, `20231003T120001Z`, with no separators
static ISO_BASIC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\d{4})(\d{2})(\d{2})[Tt](\d{2})(\d{2})(\d{2})(?:[.,]\d+)?(Z|z|[-+]\d{2}(?:\d{2})?)?$",
    )
    .unwrap()
});

/// `10/Oct/2000:13:55:36 -0700`, as in the Common Log Format, PHP's
/// `03-Oct-2023 12:00:01 UTC`, and Redis's `03 Oct 2023 12:00:00.001`
static DAY_MONTH_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^(\d{1,2})[/\-\ ]([A-Z][a-z]{2})[/\-\ ](\d{4})[:\ ](\d{2}):(\d{2}):(\d{2})
        (?:\.\d+)?
        \ ?(Z|UTC|GMT|[-+]\d{2}:?\d{2})?
        ",
    )
    .unwrap()
});

/// klog's `1003 12:00:01.123456`, a month and day with no year
static KLOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{2})(\d{2}) (\d{2}):(\d{2}):(\d{2})(?:\.\d+)?$").unwrap());

/// syslog's `Oct  3 12:00:01`, and `ctime`'s `Tue Oct  3 12:00:01 2023`
static MONTH_DAY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[A-Z][a-z]{2} )?([A-Z][a-z]{2})\s+(\d{1,2}) (\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?: (\d{4}))?",
    )
    .unwrap()
});

/// Seconds since the epoch, with an optional fraction, or milliseconds,
/// microseconds, or nanoseconds
static EPOCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:(\d{9,10})(?:\.\d+)?|(\d{13})|(\d{16})|(\d{19}))$").unwrap());

/// Reads a log timestamp into seconds since the epoch.
///
/// `zone` places a timestamp that has no zone of its own, and `now` places
/// one with no year.
pub fn parse_timestamp(text: &str, zone: &Zone, now: i64) -> Option<i64> {
    let (start, end) = timestamp_span(text);
    let text = &text[start..end];

    if let Some(caps) = ISO.captures(text).or_else(|| ISO_BASIC.captures(text)) {
        let number = |index: usize| {
            caps.get(index)
                .map_or(Some(0), |found| found.as_str().parse().ok())
        };

        let civil = Civil::new(
            number(1)?,
            number(2)? as u32,
            number(3)? as u32,
            number(4)? as u32,
            number(5)? as u32,
            number(6)? as u32,
        )?;

        return place(civil, caps.get(7).map(|found| found.as_str()), zone);
    }

    if let Some(caps) = DAY_MONTH_YEAR.captures(text) {
        let number = |index: usize| caps[index].parse::<u32>().ok();

        let civil = Civil::new(
            i64::from(number(3)?),
            month_number(&caps[2])?,
            number(1)?,
            number(4)?,
            number(5)?,
            number(6)?,
        )?;

        return place(civil, caps.get(7).map(|found| found.as_str()), zone);
    }

    if let Some(caps) = MONTH_DAY.captures(text) {
        let number = |index: usize| caps[index].parse::<u32>().ok();
        let date = (month_number(&caps[1])?, number(2)?);
        let time = (number(3)?, number(4)?, number(5)?);

        return match caps.get(6) {
            Some(year) => {
                let civil = Civil::new(
                    year.as_str().parse().ok()?,
                    date.0,
                    date.1,
                    time.0,
                    time.1,
                    time.2,
                )?;
                Some(zone.to_utc(civil.seconds()))
            }
            None => place_in_recent_year(date, time, zone, now),
        };
    }

    if let Some(caps) = KLOG.captures(text) {
        let number = |index: usize| caps[index].parse::<u32>().ok();

        return place_in_recent_year(
            (number(1)?, number(2)?),
            (number(3)?, number(4)?, number(5)?),
            zone,
            now,
        );
    }

    let caps = EPOCH.captures(text)?;

    if let Some(seconds) = caps.get(1) {
        return seconds.as_str().parse().ok();
    }

    let (digits, per_second) = [(2, 1_000), (3, 1_000_000), (4, 1_000_000_000)]
        .into_iter()
        .find_map(|(index, per_second)| Some((caps.get(index)?, per_second)))?;

    digits
        .as_str()
        .parse::<i64>()
        .ok()
        .map(|count| count / per_second)
}

fn is_wrapping(character: char) -> bool {
    character.is_whitespace() || matches!(character, '[' | ']' | '"')
}

/// The byte range of a timestamp inside the brackets, quotes, and spaces
/// around it
pub fn timestamp_span(text: &str) -> (usize, usize) {
    let start = text.len() - text.trim_start_matches(is_wrapping).len();
    let end = text.trim_end_matches(is_wrapping).len().max(start);

    (start, end)
}

/// Whether a timestamp is written as seconds or milliseconds since the epoch
pub fn is_epoch(text: &str) -> bool {
    let (start, end) = timestamp_span(text);

    EPOCH.is_match(&text[start..end])
}

/// The fraction of a second after the seconds of a timestamp, such as
/// `123` in `12:00:01.123Z` or `5241024` in `1646861401.5241024`
static FRACTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^\d+|:\d{2})[.,](\d+)").unwrap());

/// The digits of a timestamp's fraction of a second, if it has one
pub fn fraction(text: &str) -> Option<&str> {
    FRACTION
        .captures(text)
        .map(|caps| caps.get(1).unwrap().as_str())
}

/// A moment written as RFC 3339 in a zone, such as
/// `2023-10-03T07:00:01.123-05:00`, with the fraction digits given
pub fn format_rfc3339(moment: i64, fraction: Option<&str>, zone: &Zone) -> String {
    let offset = zone.offset_at(moment);
    let civil = Civil::from_seconds(moment + offset);

    let suffix = if *zone == Zone::Utc {
        "Z".to_string()
    } else {
        let sign = if offset < 0 { '-' } else { '+' };
        let minutes = offset.abs() / 60;

        format!("{}{:02}:{:02}", sign, minutes / 60, minutes % 60)
    };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}{}",
        civil.year,
        civil.month,
        civil.day,
        civil.hour,
        civil.minute,
        civil.second,
        fraction
            .map(|digits| format!(".{}", digits))
            .unwrap_or_default(),
        suffix
    )
}

/// The units a relative time is counted in, largest first
const RELATIVE_UNITS: [(i64, &str); 5] = [
    (365 * SECONDS_PER_DAY, "year"),
    (SECONDS_PER_DAY, "day"),
    (3600, "hour"),
    (60, "minute"),
    (1, "second"),
];

/// A moment written relative to `now` in its largest whole unit, such as
/// `2 hours ago`, `in 5 minutes`, or `just now`
pub fn format_relative(moment: i64, now: i64) -> String {
    let distance = (now - moment).abs();

    let Some((size, unit)) = RELATIVE_UNITS.iter().find(|(size, _)| distance >= *size) else {
        return "just now".to_string();
    };

    let count = distance / size;
    let plural = if count == 1 { "" } else { "s" };

    if moment <= now {
        format!("{} {}{} ago", count, unit, plural)
    } else {
        format!("in {} {}{}", count, unit, plural)
    }
}

/// Places a date written with no year in the latest year that does not put
/// it more than a day after `now`
fn place_in_recent_year(
    (month, day): (u32, u32),
    (hour, minute, second): (u32, u32, u32),
    zone: &Zone,
    now: i64,
) -> Option<i64> {
    let at_year = |year: i64| {
        Civil::new(year, month, day, hour, minute, second).map(|civil| zone.to_utc(civil.seconds()))
    };

    let this_year = Civil::from_seconds(now + zone.offset_at(now)).year;

    match at_year(this_year) {
        Some(moment) if moment <= now + SECONDS_PER_DAY => Some(moment),
        _ => at_year(this_year - 1),
    }
}

/// The month number for an English month abbreviation
fn month_number(name: &str) -> Option<u32> {
    MONTHS
        .iter()
        .position(|month| *month == name)
        .map(|index| index as u32 + 1)
}

/// Places a wall clock time with the zone it was written with, or in `zone`,
/// or returns `None` for an offset beyond 23 hours and 59 minutes
fn place(civil: Civil, written_zone: Option<&str>, zone: &Zone) -> Option<i64> {
    match written_zone {
        Some("Z" | "z" | "UTC" | "GMT") => Some(civil.seconds()),
        Some(offset) => Some(civil.seconds() - offset_seconds(offset)?),
        None => Some(zone.to_utc(civil.seconds())),
    }
}

/// The seconds east of UTC in an offset such as `-0700`, `+05:30`, or `+05`
fn offset_seconds(offset: &str) -> Option<i64> {
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let digits: String = offset.chars().filter(char::is_ascii_digit).collect();
    let hours: i64 = digits[..2].parse().unwrap();
    let minutes: i64 = digits[2..].parse().unwrap_or(0);

    (hours < 24 && minutes < 60).then_some(sign * (hours * 3600 + minutes * 60))
}

/// Error returned when a moment given on the command line cannot be read
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadMoment(pub String);

impl fmt::Display for BadMoment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Invalid time '{}' (expected a timestamp, a date such as 2024-01-01, \
             today, yesterday, now, or an age such as 2h ago)",
            self.0
        )
    }
}

impl std::error::Error for BadMoment {}

/// An age such as `2h ago` or `30 minutes ago`
static AGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\d+)\s*(s|secs?|seconds?|m|mins?|minutes?|h|hrs?|hours?|d|days?|w|weeks?)\s+ago$",
    )
    .unwrap()
});

/// A date with no time, such as `2024-01-01`
static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{4})-(\d{2})-(\d{2})$").unwrap());

/// Reads a moment given to `--since` or `--until` into seconds since the
/// epoch: a timestamp, a date, which means its midnight, `today`,
/// `yesterday`, `now`, or an age such as `2h ago`.
pub fn parse_moment(text: &str, zone: &Zone, now: i64) -> Result<i64, BadMoment> {
    let trimmed = text.trim();
    let bad = || BadMoment(text.to_string());

    let midnight = |days_back: i64| {
        let local = Civil::from_seconds(now + zone.offset_at(now));
        let day = days_from_civil(local.year, local.month, local.day) - days_back;

        zone.to_utc(day * SECONDS_PER_DAY)
    };

    match trimmed {
        "now" => return Ok(now),
        "today" => return Ok(midnight(0)),
        "yesterday" => return Ok(midnight(1)),
        _ => {}
    }

    if let Some(caps) = AGE.captures(trimmed) {
        let count: i64 = caps[1].parse().map_err(|_| bad())?;
        let unit = match &caps[2][..1] {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => SECONDS_PER_DAY,
            _ => 7 * SECONDS_PER_DAY,
        };

        return count.checked_mul(unit).map(|age| now - age).ok_or_else(bad);
    }

    if let Some(caps) = DATE.captures(trimmed) {
        let civil = Civil::new(
            caps[1].parse().unwrap(),
            caps[2].parse().unwrap(),
            caps[3].parse().unwrap(),
            0,
            0,
            0,
        )
        .ok_or_else(bad)?;

        return Ok(zone.to_utc(civil.seconds()));
    }

    parse_timestamp(trimmed, zone, now).ok_or_else(bad)
}
