//! Merging several logs into one stream ordered by time
//!
//! Each log is split into entries: a line with a timestamp and the lines
//! without one that follow it, such as a stack trace. The entries of every
//! log are then ordered by the time of their first line, to the fraction of
//! a second the timestamp gives, keeping the order the logs were named in
//! for entries written at the same time. Entries before a log's first
//! timestamp come first.
use crate::output::{ParsedLine, TokenKind};
use crate::parser;
use crate::time;
use crate::zone::Zone;

/// When an entry was written: seconds since the epoch and nanoseconds
pub type Moment = (i64, u32);

/// One log to merge: its text and the mode its lines are read in
pub struct Log<'a> {
    pub text: &'a str,
    pub mode: &'a str,
}

/// An entry of one log: the index of the log, and the entry's lines
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry<'a> {
    pub log: usize,
    pub text: &'a str,
}

/// The entries of every log, ordered by time
pub fn merge<'a>(logs: &[Log<'a>], zone: &Zone, now: i64) -> Vec<Entry<'a>> {
    let mut timed: Vec<(Option<Moment>, Entry<'a>)> = logs
        .iter()
        .enumerate()
        .flat_map(|(index, log)| entries(index, log, zone, now))
        .collect();

    timed.sort_by_key(|(moment, _)| *moment);
    timed.into_iter().map(|(_, entry)| entry).collect()
}

/// Splits a log into entries, each with the moment its first line was
/// written
fn entries<'a>(
    index: usize,
    log: &Log<'a>,
    zone: &Zone,
    now: i64,
) -> Vec<(Option<Moment>, Entry<'a>)> {
    let mut found: Vec<(Option<Moment>, usize, usize)> = Vec::new();
    let mut start = 0;

    for line in log.text.split_inclusive('\n') {
        let end = start + line.len();
        let moment = moment_of(line.trim_end_matches(['\n', '\r']), log.mode, zone, now);

        match found.last_mut() {
            Some(last) if moment.is_none() => last.2 = end,
            _ => found.push((moment, start, end)),
        }

        start = end;
    }

    found
        .into_iter()
        .map(|(moment, start, end)| {
            let entry = Entry {
                log: index,
                text: &log.text[start..end],
            };

            (moment, entry)
        })
        .collect()
}

/// The moment a line was written, from its first timestamp field
pub fn moment_of(line: &str, mode: &str, zone: &Zone, now: i64) -> Option<Moment> {
    let parsed: ParsedLine = parser::parse_line(line, mode)?;

    parsed
        .tokens
        .iter()
        .filter(|token| matches!(token.kind, TokenKind::Timestamp | TokenKind::DateTime))
        .find_map(|token| {
            let seconds = time::parse_timestamp(&token.text, zone, now)?;

            Some((seconds, nanoseconds(time::fraction(&token.text))))
        })
}

/// The nanoseconds a fraction of a second such as `123456` holds
fn nanoseconds(fraction: Option<&str>) -> u32 {
    let digits: String = fraction
        .unwrap_or_default()
        .chars()
        .chain(std::iter::repeat('0'))
        .take(9)
        .collect();

    digits.parse().unwrap_or(0)
}
