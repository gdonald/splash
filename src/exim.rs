//! Exim log parsing
//!
//! Exim's main log leads each line with a timestamp and, for a line about one
//! message, that message's id. A two character flag then says what happened:
//! `<=` an arrival, `=>` a delivery, `->` a delivery to an additional address,
//! `*>` a suppressed delivery, `==` a deferral, and `**` a failure. The
//! details follow as fields named by one or two capital letters, such as
//! `H=` for the remote host and `R=` for the router. When Exim logs to syslog
//! the timestamp gives way to the syslog header.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "exim";

/// The timestamp a main log line leads with, with optional milliseconds and
/// time zone (`log_timezone`), and the process id `log_selector = +pid` adds
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{4}-\d{2}-\d{2}\ \d{2}:\d{2}:\d{2}(?:\.\d{3})?(?:\ [-+]\d{4})?)
        \x20
        (?:\[(\d+)\]\ )?
        ",
    )
    .unwrap()
});

/// A message id: `1qnXYZ-000ABC-12` before Exim 4.97, `1qnXYZ-000000ABC-1234`
/// after it
static MESSAGE_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([0-9A-Za-z]{6}-[0-9A-Za-z]{6,11}-[0-9A-Za-z]{2,4})(?: |$)").unwrap()
});

/// The flag that says what happened to the message
static FLAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(<=|=>|->|\*>|==|\*\*) ").unwrap());

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Completed", TokenKind::Success),
        ("Frozen", TokenKind::Warning),
        ("retry time not reached", TokenKind::Warning),
        ("rejected", TokenKind::Failure),
        ("SMTP error", TokenKind::Failure),
    ])
});

/// The Exim log plugin
pub struct EximPlugin {
    metadata: PluginMetadata,
}

impl EximPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Exim mail routing and delivery logs",
                "splash",
            ),
        }
    }
}

impl Default for EximPlugin {
    fn default() -> Self {
        EximPlugin::new()
    }
}

impl Plugin for EximPlugin {
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

/// Parses one Exim log line, from its own log file or from syslog, or
/// returns `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = split_timestamp(line).or_else(|| split_syslog_header(line))?;

    push_body(&mut tokens, body);

    Some(ParsedLine::new(tokens))
}

/// Splits the timestamp and process id off a line from Exim's own log file.
pub fn split_timestamp(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let caps = TIMESTAMP.captures(line)?;

    let mut tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
    ];

    if let Some(pid) = caps.get(2) {
        tokens.extend([
            Token::new("[", TokenKind::Punctuation),
            Token::new(pid.as_str(), TokenKind::Pid),
            Token::new("]", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);
    }

    Some((tokens, &line[caps.get(0).unwrap().end()..]))
}

/// Splits the syslog header off a line Exim sent to syslog.
fn split_syslog_header(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let header = mail::split_syslog_header(line)?;

    if !header.program.starts_with("exim") {
        return None;
    }

    Some((header.tokens, header.body))
}

/// Colors what follows the timestamp: the message id, the flag, and the
/// fields.
fn push_body<'a>(tokens: &mut Vec<Token<'a>>, mut body: &'a str) {
    if let Some(caps) = MESSAGE_ID.captures(body) {
        let id = caps.get(1).unwrap();

        tokens.push(Token::new(id.as_str(), TokenKind::QueueId));

        let end = caps.get(0).unwrap().end();

        if end > id.end() {
            tokens.push(Token::new(" ", TokenKind::Plain));
        }

        body = &body[end..];
    }

    if let Some(caps) = FLAG.captures(body) {
        let flag = caps.get(1).unwrap().as_str();

        tokens.push(Token::new(flag, flag_kind(flag)));
        tokens.push(Token::new(" ", TokenKind::Plain));

        body = &body[caps.get(0).unwrap().end()..];
    }

    mail::push_fields(tokens, body, &WORDS);
}

/// The kind a flag is colored in: an arrival or a delivery went through, a
/// suppressed delivery or a deferral needs attention, and `**` is a failure.
pub fn flag_kind(flag: &str) -> TokenKind {
    match flag {
        "<=" | "=>" | "->" => TokenKind::Success,
        "*>" | "==" => TokenKind::Warning,
        _ => TokenKind::Failure,
    }
}
