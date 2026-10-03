//! MySQL log parsing
//!
//! MySQL's error log writes `time thread [label] [err_code] [subsystem] msg`,
//! as in `2023-10-03T12:00:01.123456Z 0 [Warning] [MY-010068] [Server] CA
//! certificate ca.pem is self signed.`. MySQL 5.7 and MariaDB leave out the
//! code and subsystem, and MariaDB writes a space where MySQL writes `T`.
//! The slow query log describes each query on `#` comment lines, `# Time:`,
//! `# User@Host:`, and `# Query_time:`, before the statements themselves.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "mysql";

/// An error log line
static ERROR_LOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{4}-\d{2}-\d{2}[T\ ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[-+]\d{2}:\d{2})?)   # time
        \ (\d+)                         # thread
        \ \[([A-Za-z]+)\]               # label
        (?:\ \[(MY-\d+)\])?             # error code
        (?:\ \[([^\]]+)\])?             # subsystem
        (.*)                            # message, with its leading space
        $
        ",
    )
    .unwrap()
});

/// The `# Time:` line of a slow query
static SLOW_TIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(# )(Time)(: )(.+)$").unwrap());

/// The `# User@Host:` line of a slow query
static SLOW_USER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(# )(User@Host)(: )([^\[\s]*)(\[)([^\]]*)(\] @ )(\S*)( \[)([^\]]*)(\])(\s+)(Id)(:)(\s*)(\d+)$")
        .unwrap()
});

/// A `# Query_time:` line, made of `Name: value` pairs
static SLOW_STATS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^# Query_time: ").unwrap());

/// One `Name: value` pair of a `# Query_time:` line
static STAT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\w+)(: )(\S+)").unwrap());

/// A statement in the slow query log
static STATEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)
        ^(?:select|insert|update|delete|replace|set|use|with|create|alter|drop|show|call|begin|commit|rollback)\b
        |;$
        ",
    )
    .unwrap()
});

/// The lines the slow query log opens with
static SLOW_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:\S+, Version: .*|Tcp port: \d+  Unix socket: .*|Time\s+Id Command\s+Argument)$",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("ready for connections", TokenKind::Success),
        ("Shutdown complete", TokenKind::Warning),
        ("Access denied", TokenKind::Failure),
        ("Aborted connection", TokenKind::Warning),
        ("deadlock", TokenKind::Failure),
        ("Deadlock", TokenKind::Failure),
        ("crashed", TokenKind::Failure),
        ("error", TokenKind::Failure),
        ("failed", TokenKind::Failure),
    ])
});

/// The MySQL log plugin
pub struct MysqlPlugin {
    metadata: PluginMetadata,
}

impl MysqlPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "MySQL and MariaDB error and slow query logs",
                "splash",
            ),
        }
    }
}

impl Default for MysqlPlugin {
    fn default() -> Self {
        MysqlPlugin::new()
    }
}

impl Plugin for MysqlPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a> {
        match parse_line(line) {
            Some(parsed) => ParseResult::Parsed(parsed),
            None => ParseResult::NoMatch,
        }
    }
}

/// Parses one line of a MySQL error or slow query log, or returns `None` when
/// the line is in neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_error_line(line)
        .or_else(|| parse_slow_time(line))
        .or_else(|| parse_slow_user(line))
        .or_else(|| parse_slow_stats(line))
        .or_else(|| parse_slow_text(line))
}

/// The kind an error log label is colored in
fn label_kind(label: &str) -> TokenKind {
    match label {
        "ERROR" | "Error" => TokenKind::Failure,
        "Warning" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// Parses an error log line.
pub fn parse_error_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = ERROR_LOG.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());
    let label = field(3).unwrap();

    let mut tokens = vec![
        Token::new(field(1).unwrap(), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2).unwrap(), TokenKind::Pid),
        Token::new(" ", TokenKind::Plain),
        Token::new("[", TokenKind::Punctuation),
        Token::new(label, label_kind(label)),
        Token::new("]", TokenKind::Punctuation),
    ];

    for (index, kind) in [(4, TokenKind::Transaction), (5, TokenKind::Module)] {
        if let Some(text) = field(index) {
            tokens.extend([
                Token::new(" ", TokenKind::Plain),
                Token::new("[", TokenKind::Punctuation),
                Token::new(text, kind),
                Token::new("]", TokenKind::Punctuation),
            ]);
        }
    }

    mail::push_words(&mut tokens, field(6).unwrap(), &WORDS);

    Some(ParsedLine::new(tokens))
}

/// Parses the `# Time:` line of a slow query.
pub fn parse_slow_time(line: &str) -> Option<ParsedLine<'_>> {
    let caps = SLOW_TIME.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    Some(ParsedLine::new(vec![
        Token::new(field(1), TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Header),
        Token::new(field(3), TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Timestamp),
    ]))
}

/// Parses the `# User@Host:` line of a slow query.
pub fn parse_slow_user(line: &str) -> Option<ParsedLine<'_>> {
    let caps = SLOW_USER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let kinds = [
        TokenKind::Punctuation,
        TokenKind::Header,
        TokenKind::Punctuation,
        TokenKind::UserId,
        TokenKind::Punctuation,
        TokenKind::UserId,
        TokenKind::Punctuation,
        TokenKind::Host,
        TokenKind::Punctuation,
        TokenKind::Ip,
        TokenKind::Punctuation,
        TokenKind::Plain,
        TokenKind::Header,
        TokenKind::Punctuation,
        TokenKind::Plain,
        TokenKind::Pid,
    ];

    let tokens = kinds
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !field(index + 1).is_empty())
        .map(|(index, kind)| Token::new(field(index + 1), kind))
        .collect();

    Some(ParsedLine::new(tokens))
}

/// Parses the `# Query_time:` line of a slow query, coloring each time as a
/// duration and each count as a number.
pub fn parse_slow_stats(line: &str) -> Option<ParsedLine<'_>> {
    if !SLOW_STATS.is_match(line) {
        return None;
    }

    let mut tokens = vec![Token::new("# ", TokenKind::Punctuation)];
    let mut cursor = 2;

    for caps in STAT.captures_iter(&line[2..]) {
        let whole = caps.get(0).unwrap();
        let name = caps.get(1).unwrap().as_str();
        let start = whole.start() + 2;

        if start > cursor {
            tokens.push(Token::new(&line[cursor..start], TokenKind::Plain));
        }

        let kind = if name.ends_with("_time") {
            TokenKind::Duration
        } else {
            TokenKind::Number
        };

        tokens.extend([
            Token::new(name, TokenKind::Header),
            Token::new(caps.get(2).unwrap().as_str(), TokenKind::Punctuation),
            Token::new(caps.get(3).unwrap().as_str(), kind),
        ]);

        cursor = whole.end() + 2;
    }

    if cursor < line.len() {
        tokens.push(Token::new(&line[cursor..], TokenKind::Plain));
    }

    Some(ParsedLine::new(tokens))
}

/// Parses a statement or one of the lines a slow query log opens with.
pub fn parse_slow_text(line: &str) -> Option<ParsedLine<'_>> {
    if STATEMENT.is_match(line) {
        return Some(ParsedLine::new(vec![Token::new(line, TokenKind::Request)]));
    }

    SLOW_HEADER
        .is_match(line)
        .then(|| ParsedLine::new(vec![Token::new(line, TokenKind::Message)]))
}
