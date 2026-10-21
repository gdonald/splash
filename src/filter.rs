//! Line filters and match highlighting
//!
//! Filters judge each parsed line. `--grep`, `--exclude`, and `--filter`
//! judge a line on its own. `--level`, `--since`, and `--until` judge the
//! entry a line belongs to: a line with no timestamp and no level, such as a
//! stack trace line, belongs to the entry above it and is kept or dropped
//! with that entry.
use crate::keywords::restyle;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::time::{self, BadMoment};
use crate::zone::Zone;
use regex::Regex;
use std::fmt;

/// Error returned when a filter given on the command line cannot be built
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterError {
    UnknownLevel(String),
    Moment(BadMoment),
    BadPattern { pattern: String, message: String },
    BadField(String),
    UnknownField(String),
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterError::UnknownLevel(name) => write!(
                f,
                "Unknown log level '{}' (expected trace, debug, info, notice, warn, error, or critical)",
                name
            ),
            FilterError::Moment(error) => write!(f, "{}", error),
            FilterError::BadPattern { pattern, message } => {
                write!(f, "Invalid pattern '{}': {}", pattern, message)
            }
            FilterError::BadField(value) => write!(
                f,
                "Invalid field filter '{}' (expected KEY=VALUE, such as ip=192.168.1.1)",
                value
            ),
            FilterError::UnknownField(key) => write!(f, "Unknown field '{}'", key),
        }
    }
}

impl std::error::Error for FilterError {}

impl From<BadMoment> for FilterError {
    fn from(error: BadMoment) -> Self {
        FilterError::Moment(error)
    }
}

/// The severity a log line is written at
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
}

impl Level {
    /// The level's name in reports
    pub fn name(&self) -> &'static str {
        match self {
            Level::Trace => "trace",
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Notice => "notice",
            Level::Warning => "warning",
            Level::Error => "error",
            Level::Critical => "critical",
        }
    }

    /// The level a name such as `WARN`, `err`, or `fatal` stands for
    pub fn from_name(name: &str) -> Option<Level> {
        match name.to_ascii_uppercase().as_str() {
            "TRACE" => Some(Level::Trace),
            "DEBUG" | "DBG" => Some(Level::Debug),
            "INFO" | "INFORMATION" => Some(Level::Info),
            "NOTICE" => Some(Level::Notice),
            "WARN" | "WARNING" => Some(Level::Warning),
            "ERROR" | "ERR" | "SEVERE" => Some(Level::Error),
            "CRIT" | "CRITICAL" | "FATAL" | "ALERT" | "EMERG" | "EMERGENCY" | "PANIC" => {
                Some(Level::Critical)
            }
            _ => None,
        }
    }

    /// The level a level field holds: a name, such as `[error]` or
    /// PostgreSQL's `DEBUG2` and `LOG`, a single letter, such as MongoDB's `I`
    /// or `D2`, or one of the marks Redis writes
    pub fn from_field(text: &str) -> Option<Level> {
        match text {
            "." | "-" => return Some(Level::Debug),
            "*" => return Some(Level::Notice),
            "#" => return Some(Level::Warning),
            _ => {}
        }

        let name = text.trim_matches(|character: char| !character.is_ascii_alphanumeric());
        let without_digits = name.trim_end_matches(|character: char| character.is_ascii_digit());

        if let Some(level) = Level::from_name(without_digits) {
            return Some(level);
        }

        if matches!(
            without_digits.to_ascii_uppercase().as_str(),
            "LOG" | "NOTE" | "SYSTEM"
        ) {
            return Some(Level::Info);
        }

        let mut characters = name.chars();
        let letter = characters.next()?;

        if !characters.all(|character| character.is_ascii_digit()) {
            return None;
        }

        match letter.to_ascii_uppercase() {
            'T' => Some(Level::Trace),
            'D' => Some(Level::Debug),
            'I' => Some(Level::Info),
            'N' => Some(Level::Notice),
            'W' => Some(Level::Warning),
            'E' => Some(Level::Error),
            'C' | 'F' => Some(Level::Critical),
            _ => None,
        }
    }

    /// The level a parsed line is written at: its first level field, or
    /// failing that the first word in it that names a level
    pub fn of_line(parsed: &ParsedLine) -> Option<Level> {
        let field = parsed
            .tokens
            .iter()
            .filter(|token| {
                matches!(
                    token.kind,
                    TokenKind::Level | TokenKind::Warning | TokenKind::Failure
                )
            })
            .find_map(|token| Level::from_field(&token.text));

        field.or_else(|| {
            parsed
                .text()
                .split(|character: char| !character.is_ascii_alphanumeric())
                .find_map(Level::from_name)
        })
    }
}

/// A pattern given to `--grep` or `--exclude`
#[derive(Debug, Clone)]
pub struct Pattern {
    pub regex: Regex,
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.regex.as_str() == other.regex.as_str()
    }
}

impl Eq for Pattern {}

impl Pattern {
    fn compile(pattern: &str) -> Result<Pattern, FilterError> {
        Regex::new(pattern)
            .map(|regex| Pattern { regex })
            .map_err(|error| FilterError::BadPattern {
                pattern: pattern.to_string(),
                message: error.to_string(),
            })
    }
}

/// A field a line must hold, as given to `--filter`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub kinds: Vec<TokenKind>,
    pub value: String,
}

impl Field {
    /// Reads `KEY=VALUE`, where the key is a token kind name. `ip` also
    /// matches the client address of an access log.
    pub fn parse(text: &str) -> Result<Field, FilterError> {
        let bad = || FilterError::BadField(text.to_string());
        let (key, value) = text.split_once('=').ok_or_else(bad)?;
        let key = key.trim();

        if key.is_empty() || value.is_empty() {
            return Err(bad());
        }

        let kinds = match key {
            "ip" => vec![TokenKind::Ip, TokenKind::Client],
            _ => vec![TokenKind::from_name(key)
                .ok_or_else(|| FilterError::UnknownField(key.to_string()))?],
        };

        Ok(Field {
            kinds,
            value: value.to_string(),
        })
    }

    fn matches(&self, parsed: &ParsedLine) -> bool {
        parsed.tokens.iter().any(|token| {
            self.kinds.contains(&token.kind) && token.text.trim_matches('"') == self.value
        })
    }
}

/// The time range `--since` and `--until` keep, with the zone and clock that
/// place a log timestamp
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeRange {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub zone: Zone,
    pub now: i64,
}

impl TimeRange {
    fn contains(&self, moment: i64) -> bool {
        self.since.is_none_or(|since| moment >= since)
            && self.until.is_none_or(|until| moment <= until)
    }

    /// The moment a parsed line was written, from its first timestamp field
    fn moment_of(&self, parsed: &ParsedLine) -> Option<i64> {
        parsed
            .tokens
            .iter()
            .filter(|token| is_timestamp(token))
            .find_map(|token| time::parse_timestamp(&token.text, &self.zone, self.now))
    }
}

fn is_timestamp(token: &Token) -> bool {
    matches!(token.kind, TokenKind::Timestamp | TokenKind::DateTime)
}

/// The filters given on the command line, before they are checked
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterOptions {
    pub levels: Vec<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub grep: Vec<String>,
    pub exclude: Vec<String>,
    pub fields: Vec<String>,
}

/// The filters a run applies to each parsed line
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub levels: Vec<Level>,
    pub time: Option<TimeRange>,
    pub grep: Vec<Pattern>,
    pub exclude: Vec<Pattern>,
    pub fields: Vec<Field>,
}

/// How a filter judged one line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Judgement {
    /// Whether the line passes the filters that judge a line on its own
    pub matches: bool,
    /// Whether the entry the line starts is kept, or `None` when the line
    /// belongs to the entry above it
    pub entry: Option<bool>,
}

impl Filter {
    /// Checks the filters given on the command line, reading times in the
    /// local zone
    pub fn build(options: &FilterOptions) -> Result<Filter, FilterError> {
        if options.since.is_none() && options.until.is_none() {
            return Filter::build_at(options, Zone::Utc, 0);
        }

        Filter::build_at(options, Zone::local(), time::now())
    }

    /// Checks the filters given on the command line, reading times in `zone`
    /// as of `now`
    pub fn build_at(options: &FilterOptions, zone: Zone, now: i64) -> Result<Filter, FilterError> {
        let mut levels = Vec::new();

        for name in options.levels.iter().flat_map(|value| value.split(',')) {
            let name = name.trim();

            if name.is_empty() {
                continue;
            }

            levels.push(
                Level::from_name(name)
                    .ok_or_else(|| FilterError::UnknownLevel(name.to_string()))?,
            );
        }

        let moment = |value: &Option<String>| {
            value
                .as_deref()
                .map(|text| time::parse_moment(text, &zone, now))
                .transpose()
        };

        let since = moment(&options.since)?;
        let until = moment(&options.until)?;

        let time = (since.is_some() || until.is_some()).then(|| TimeRange {
            since,
            until,
            zone: zone.clone(),
            now,
        });

        Ok(Filter {
            levels,
            time,
            grep: compile_all(&options.grep)?,
            exclude: compile_all(&options.exclude)?,
            fields: options
                .fields
                .iter()
                .map(|field| Field::parse(field))
                .collect::<Result<_, _>>()?,
        })
    }

    /// Judges one parsed line
    pub fn judge(&self, parsed: &ParsedLine) -> Judgement {
        Judgement {
            matches: self.matches(parsed),
            entry: self.entry(parsed),
        }
    }

    fn matches(&self, parsed: &ParsedLine) -> bool {
        if self.grep.is_empty() && self.exclude.is_empty() && self.fields.is_empty() {
            return true;
        }

        let text = parsed.text();

        (self.grep.is_empty()
            || self
                .grep
                .iter()
                .any(|pattern| pattern.regex.is_match(&text)))
            && !self
                .exclude
                .iter()
                .any(|pattern| pattern.regex.is_match(&text))
            && self.fields.iter().all(|field| field.matches(parsed))
    }

    fn entry(&self, parsed: &ParsedLine) -> Option<bool> {
        let level = if self.levels.is_empty() {
            Some(true)
        } else {
            match Level::of_line(parsed) {
                Some(level) => Some(self.levels.contains(&level)),
                None if parsed.tokens.iter().any(is_timestamp) => Some(false),
                None => None,
            }
        };

        let time = match &self.time {
            None => Some(true),
            Some(range) => range.moment_of(parsed).map(|moment| range.contains(moment)),
        };

        match (level, time) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        }
    }

    /// Restyles the text `--grep` matched as a match, and the text each of
    /// its capture groups matched as a capture
    pub fn highlight<'a>(&self, parsed: ParsedLine<'a>) -> ParsedLine<'a> {
        if self.grep.is_empty() {
            return parsed;
        }

        let text = parsed.text();
        let mut marks: Vec<Option<TokenKind>> = vec![None; text.len()];

        for pattern in &self.grep {
            for caps in pattern.regex.captures_iter(&text) {
                for (index, group) in caps.iter().enumerate() {
                    let Some(group) = group else { continue };
                    let kind = if index == 0 {
                        TokenKind::Match
                    } else {
                        TokenKind::Capture
                    };

                    marks[group.range()].fill(Some(kind));
                }
            }
        }

        let mut tokens = Vec::new();
        let mut offset = 0;

        for token in parsed.tokens {
            let end = offset + token.text.len();
            tokens.extend(restyle(&token, &marks[offset..end]));
            offset = end;
        }

        ParsedLine::new(tokens)
    }
}

fn compile_all(patterns: &[String]) -> Result<Vec<Pattern>, FilterError> {
    patterns
        .iter()
        .map(|pattern| Pattern::compile(pattern))
        .collect()
}

/// Which entry the lines being rendered belong to, carried from one chunk of
/// input to the next
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Entry {
    kept: bool,
}

impl Entry {
    /// Whether a judged line is kept, given the entries before it. A line
    /// belonging to the entry above the first line is dropped.
    pub fn admit(&mut self, judgement: Judgement) -> bool {
        if let Some(kept) = judgement.entry {
            self.kept = kept;
        }

        judgement.matches && self.kept
    }
}
