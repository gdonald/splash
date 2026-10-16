//! Time zones
//!
//! Many log timestamps carry no zone and are read in the local zone. A zone
//! is read from a TZif file, as installed under `/usr/share/zoneinfo`, or from
//! a POSIX `TZ` rule such as `EST5EDT,M3.2.0,M11.1.0`, which is also the rule
//! a TZif file gives for the times after its last transition.
use crate::time::{civil_from_days, days_from_civil, days_in_month, weekday, SECONDS_PER_DAY};
use std::fs;
use std::path::Path;

/// Where zone files are installed
pub const ZONEINFO: &str = "/usr/share/zoneinfo";

/// The file naming the local zone when `TZ` is not set
pub const LOCALTIME: &str = "/etc/localtime";

/// A time zone: the offset from UTC, in seconds, at every moment
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Zone {
    #[default]
    Utc,
    Rule(Rule),
    Table(Table),
}

impl Zone {
    /// The zone `TZ` names, or the one `/etc/localtime` holds, or UTC when
    /// neither can be read
    pub fn local() -> Zone {
        Zone::from_env(std::env::var("TZ").ok().as_deref(), Path::new(LOCALTIME))
    }

    /// The zone a `TZ` value names, falling back to the zone file at
    /// `localtime` when the value is unset or empty, and to UTC when nothing
    /// can be read
    pub fn from_env(tz: Option<&str>, localtime: &Path) -> Zone {
        let found = match tz.filter(|value| !value.is_empty()) {
            Some(value) => Zone::named(value.trim_start_matches(':')),
            None => Zone::from_file(localtime),
        };

        found.unwrap_or(Zone::Utc)
    }

    /// The zone with a name such as `UTC` or `America/Chicago`, a path to a
    /// zone file, or a POSIX rule
    pub fn named(name: &str) -> Option<Zone> {
        if matches!(name, "UTC" | "Z" | "GMT" | "Etc/UTC" | "UTC0") {
            return Some(Zone::Utc);
        }

        if name.starts_with('/') {
            return Zone::from_file(Path::new(name));
        }

        if !name.contains("..") {
            if let Some(zone) = Zone::from_file(&Path::new(ZONEINFO).join(name)) {
                return Some(zone);
            }
        }

        Rule::parse(name).map(Zone::Rule)
    }

    /// The zone held in a TZif file
    pub fn from_file(path: &Path) -> Option<Zone> {
        Table::parse(&fs::read(path).ok()?).map(Zone::Table)
    }

    /// The offset from UTC, in seconds, at a moment given in UTC
    pub fn offset_at(&self, utc: i64) -> i64 {
        match self {
            Zone::Utc => 0,
            Zone::Rule(rule) => rule.offset_at(utc),
            Zone::Table(table) => table.offset_at(utc),
        }
    }

    /// The moment in UTC that a wall clock time in this zone names. A time a
    /// clock change repeats reads as its first occurrence, and a time a clock
    /// change skips reads with the offset in force before the change.
    pub fn to_utc(&self, local: i64) -> i64 {
        let before = self.offset_at(local - SECONDS_PER_DAY);
        let after = self.offset_at(local + SECONDS_PER_DAY);

        [before, after]
            .into_iter()
            .find(|offset| self.offset_at(local - offset) == *offset)
            .map_or(local - before, |offset| local - offset)
    }
}

/// A zone file's transitions, read from its 64-bit data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// The moments, in UTC, at which the offset changes
    pub transitions: Vec<i64>,
    /// The offset that takes effect at each transition
    pub offsets: Vec<i64>,
    /// The offset before the first transition
    pub initial: i64,
    /// The rule for moments after the last transition
    pub rule: Option<Rule>,
}

impl Table {
    /// Reads a TZif file, version 2 or later
    pub fn parse(bytes: &[u8]) -> Option<Table> {
        let mut reader = Reader { bytes, position: 0 };

        let version_one = Header::read(&mut reader)?;
        reader.skip(version_one.data_length(4))?;

        let header = Header::read(&mut reader)?;

        let transitions: Vec<i64> = (0..header.transitions)
            .map(|_| reader.i64())
            .collect::<Option<_>>()?;

        let indexes: Vec<usize> = (0..header.transitions)
            .map(|_| reader.u8().map(usize::from))
            .collect::<Option<_>>()?;

        let types: Vec<i64> = (0..header.types)
            .map(|_| {
                let offset = reader.i32()?;
                reader.skip(2)?;
                Some(i64::from(offset))
            })
            .collect::<Option<_>>()?;

        reader.skip(header.trailing_length(8))?;

        let offsets = indexes
            .iter()
            .map(|index| types.get(*index).copied())
            .collect::<Option<Vec<i64>>>()?;

        let footer = String::from_utf8_lossy(reader.rest()).to_string();

        Some(Table {
            transitions,
            offsets,
            initial: *types.first()?,
            rule: Rule::parse(footer.trim()),
        })
    }

    fn offset_at(&self, utc: i64) -> i64 {
        let after = self.transitions.partition_point(|moment| *moment <= utc);

        if after == 0 {
            return self.initial;
        }

        match &self.rule {
            Some(rule) if after == self.transitions.len() => rule.offset_at(utc),
            _ => self.offsets[after - 1],
        }
    }
}

struct Header {
    is_utc: usize,
    is_standard: usize,
    leaps: usize,
    transitions: usize,
    types: usize,
    characters: usize,
}

impl Header {
    fn read(reader: &mut Reader) -> Option<Header> {
        if reader.take(4)? != b"TZif" {
            return None;
        }

        let version = reader.u8()?;
        if version == 0 {
            return None;
        }

        reader.skip(15)?;

        let mut count = || reader.u32().map(|value| value as usize);

        Some(Header {
            is_utc: count()?,
            is_standard: count()?,
            leaps: count()?,
            transitions: count()?,
            types: count()?,
            characters: count()?,
        })
    }

    /// The length of the data block, with times `time_size` bytes long
    fn data_length(&self, time_size: usize) -> usize {
        self.transitions * (time_size + 1) + self.types * 6 + self.trailing_length(time_size)
    }

    /// The length of the data after the local time types
    fn trailing_length(&self, time_size: usize) -> usize {
        self.characters + self.leaps * (time_size + 4) + self.is_standard + self.is_utc
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.position.checked_add(length)?;
        let taken = self.bytes.get(self.position..end)?;
        self.position = end;
        Some(taken)
    }

    fn skip(&mut self, length: usize) -> Option<()> {
        self.take(length).map(|_| ())
    }

    fn rest(&self) -> &'a [u8] {
        &self.bytes[self.position..]
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|bytes| bytes[0])
    }

    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|bytes| u32::from_be_bytes(bytes.try_into().unwrap()))
    }

    fn i32(&mut self) -> Option<i32> {
        self.take(4)
            .map(|bytes| i32::from_be_bytes(bytes.try_into().unwrap()))
    }

    fn i64(&mut self) -> Option<i64> {
        self.take(8)
            .map(|bytes| i64::from_be_bytes(bytes.try_into().unwrap()))
    }
}

/// A POSIX `TZ` rule: a standard offset, and optionally a daylight saving
/// offset with the dates it starts and ends each year
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// The standard offset east of UTC, in seconds
    pub standard: i64,
    pub daylight: Option<Daylight>,
}

/// When daylight saving time is in force under a rule
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Daylight {
    /// The daylight saving offset east of UTC, in seconds
    pub offset: i64,
    /// The day and local standard time daylight saving starts
    pub start: Change,
    /// The day and local daylight saving time it ends
    pub end: Change,
}

/// A day of the year and a time on it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub day: Day,
    /// Seconds after midnight, which may be negative or past one day
    pub time: i64,
}

/// The ways a rule names a day of the year
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Day {
    /// `Mm.w.d`: day `d` (0 is Sunday) of week `w` of month `m`, where week 5
    /// is the last one
    Weekday { month: u32, week: u32, day: u32 },
    /// `Jn`: day `n` from 1 to 365, never counting February 29
    Julian(u32),
    /// `n`: day `n` from 0 to 365, counting February 29
    Ordinal(u32),
}

/// The time a change happens when a rule does not say
const DEFAULT_CHANGE_TIME: i64 = 2 * 3600;

impl Rule {
    /// Reads a POSIX `TZ` rule
    pub fn parse(text: &str) -> Option<Rule> {
        let mut scanner = Scanner { text, position: 0 };

        scanner.name()?;
        let standard = -scanner.duration()?;

        if scanner.at_end() {
            return Some(Rule {
                standard,
                daylight: None,
            });
        }

        scanner.name()?;

        let offset = if scanner.peek() == Some(',') {
            standard + 3600
        } else {
            -scanner.duration()?
        };

        scanner.expect(',')?;
        let start = scanner.change()?;
        scanner.expect(',')?;
        let end = scanner.change()?;

        if !scanner.at_end() {
            return None;
        }

        Some(Rule {
            standard,
            daylight: Some(Daylight { offset, start, end }),
        })
    }

    fn offset_at(&self, utc: i64) -> i64 {
        let Some(daylight) = &self.daylight else {
            return self.standard;
        };

        let year = civil_from_days((utc + self.standard).div_euclid(SECONDS_PER_DAY)).0;
        let starts = daylight.start.local_seconds(year) - self.standard;
        let ends = daylight.end.local_seconds(year) - daylight.offset;

        let in_daylight = if starts < ends {
            starts <= utc && utc < ends
        } else {
            !(ends <= utc && utc < starts)
        };

        if in_daylight {
            daylight.offset
        } else {
            self.standard
        }
    }
}

impl Change {
    /// The local seconds since the epoch at which this change happens in a year
    fn local_seconds(&self, year: i64) -> i64 {
        self.day.days(year) * SECONDS_PER_DAY + self.time
    }
}

impl Day {
    /// The days since the epoch of this day in a year
    fn days(&self, year: i64) -> i64 {
        let january_first = days_from_civil(year, 1, 1);

        match *self {
            Day::Weekday { month, week, day } => {
                let first = days_from_civil(year, month, 1);
                let to_weekday = (i64::from(day) - weekday(first)).rem_euclid(7);
                let mut date = 1 + to_weekday + 7 * (i64::from(week) - 1);

                while date > days_in_month(year, month) {
                    date -= 7;
                }

                first + date - 1
            }
            Day::Julian(day) => {
                let leap_skip = i64::from(day > 59 && days_in_month(year, 2) == 29);
                january_first + i64::from(day) - 1 + leap_skip
            }
            Day::Ordinal(day) => january_first + i64::from(day),
        }
    }
}

struct Scanner<'a> {
    text: &'a str,
    position: usize,
}

impl Scanner<'_> {
    fn peek(&self) -> Option<char> {
        self.text[self.position..].chars().next()
    }

    fn at_end(&self) -> bool {
        self.position >= self.text.len()
    }

    fn expect(&mut self, wanted: char) -> Option<()> {
        if self.peek() != Some(wanted) {
            return None;
        }

        self.position += 1;
        Some(())
    }

    fn take_while(&mut self, keep: impl Fn(char) -> bool) -> &str {
        let start = self.position;

        while self.peek().is_some_and(&keep) {
            self.position += 1;
        }

        &self.text[start..self.position]
    }

    /// A zone abbreviation: three or more letters, or anything in `<>`
    fn name(&mut self) -> Option<()> {
        if self.peek() == Some('<') {
            self.position += 1;
            self.take_while(|character| character != '>');
            return self.expect('>');
        }

        (self
            .take_while(|character| character.is_ascii_alphabetic())
            .len()
            >= 3)
            .then_some(())
    }

    fn number(&mut self) -> Option<i64> {
        self.take_while(|character| character.is_ascii_digit())
            .parse()
            .ok()
    }

    /// `[+-]hh[:mm[:ss]]` in seconds
    fn duration(&mut self) -> Option<i64> {
        let sign = match self.peek() {
            Some('-') => {
                self.position += 1;
                -1
            }
            Some('+') => {
                self.position += 1;
                1
            }
            _ => 1,
        };

        let mut seconds = self.number()? * 3600;

        for scale in [60, 1] {
            if self.peek() != Some(':') {
                break;
            }

            self.position += 1;
            seconds += self.number()? * scale;
        }

        Some(sign * seconds)
    }

    fn change(&mut self) -> Option<Change> {
        let day = match self.peek()? {
            'M' => {
                self.position += 1;
                let month = self.number()?;
                self.expect('.')?;
                let week = self.number()?;
                self.expect('.')?;
                let day = self.number()?;

                if !(1..=12).contains(&month) || !(1..=5).contains(&week) || day > 6 {
                    return None;
                }

                Day::Weekday {
                    month: month as u32,
                    week: week as u32,
                    day: day as u32,
                }
            }
            'J' => {
                self.position += 1;
                let day = self.number()?;

                if !(1..=365).contains(&day) {
                    return None;
                }

                Day::Julian(day as u32)
            }
            _ => {
                let day = self.number()?;

                if day > 365 {
                    return None;
                }

                Day::Ordinal(day as u32)
            }
        };

        let time = if self.peek() == Some('/') {
            self.position += 1;
            self.duration()?
        } else {
            DEFAULT_CHANGE_TIME
        };

        Some(Change { day, time })
    }
}
