//! Fetchmail log parsing
//!
//! Fetchmail reports each poll as a sentence: how many messages wait for
//! which user at which server, then one `reading message` line per message
//! with its size in octets and whether it was flushed from the server. It
//! logs through syslog, or to its own log file with a `fetchmail: ` prefix on
//! everything but the per-message progress lines.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "fetchmail";

/// The prefix fetchmail puts on its own log file lines
const PREFIX: &str = "fetchmail: ";

/// The per-message progress lines, which the log file carries unprefixed
static PROGRESS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:reading|skipping) message ").unwrap());

/// The parts of a fetchmail sentence that carry a value: a date, the user and
/// server a poll was for, a size in octets, a `key=value` field, an address,
/// and a count or version number
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?x)
        ({})                                  # date
        |
        (for\ )(\S+)(\ at\ )([\w.-]*\w)       # user and server
        |
        (\d+)((?:\ header)?\ octets)          # size
        |
        ([A-Za-z][\w-]*=[^\s,]*)              # field
        |
        (\d{{1,3}}(?:\.\d{{1,3}}){{3}})         # address
        |
        \b\d+(?:\.\d+)*\b                     # count or version
        ",
        mail::CTIME
    ))
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("flushed", TokenKind::Success),
        ("SUCCESS", TokenKind::Success),
        ("not flushed", TokenKind::Warning),
        ("skipped", TokenKind::Warning),
        ("timeout", TokenKind::Warning),
        ("LOCKBUSY", TokenKind::Warning),
        ("Authorization failure", TokenKind::Failure),
        ("AUTHFAIL", TokenKind::Failure),
        ("PROTOCOL", TokenKind::Failure),
        ("SOCKET", TokenKind::Failure),
        ("error", TokenKind::Failure),
    ])
});

/// The Fetchmail log plugin
pub struct FetchmailPlugin {
    metadata: PluginMetadata,
}

impl FetchmailPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Fetchmail email retrieval logs",
                "splash",
            ),
        }
    }
}

impl Default for FetchmailPlugin {
    fn default() -> Self {
        FetchmailPlugin::new()
    }
}

impl Plugin for FetchmailPlugin {
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

/// Parses one fetchmail log line, from syslog or from fetchmail's own log
/// file, or returns `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = split_syslog_header(line)
        .or_else(|| split_prefix(line))
        .or_else(|| PROGRESS.is_match(line).then(|| (Vec::new(), line)))?;

    push_body(&mut tokens, body);

    Some(ParsedLine::new(tokens))
}

/// Splits the syslog header off a line fetchmail sent to syslog.
fn split_syslog_header(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let header = mail::split_syslog_header(line)?;

    if header.program != "fetchmail" {
        return None;
    }

    Some((header.tokens, header.body))
}

/// Splits the `fetchmail: ` prefix off a line from fetchmail's own log file.
fn split_prefix(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let body = line.strip_prefix(PREFIX)?;

    let tokens = vec![
        Token::new(&line[..PREFIX.len() - 2], TokenKind::Tag),
        Token::new(":", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ];

    Some((tokens, body))
}

/// Colors the values in a fetchmail sentence, leaving the words between them
/// to the shared mail field scanner.
fn push_body<'a>(tokens: &mut Vec<Token<'a>>, body: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(body) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_fields(tokens, &body[cursor..whole.start()], &WORDS);

        if let Some(date) = field(1) {
            tokens.push(Token::new(date, TokenKind::Timestamp));
        } else if let Some(user) = field(3) {
            tokens.extend([
                Token::new(field(2).unwrap(), TokenKind::Message),
                Token::new(user, TokenKind::UserId),
                Token::new(field(4).unwrap(), TokenKind::Message),
                Token::new(field(5).unwrap(), TokenKind::Host),
            ]);
        } else if let Some(size) = field(6) {
            tokens.extend([
                Token::new(size, TokenKind::Size),
                Token::new(field(7).unwrap(), TokenKind::Message),
            ]);
        } else if let Some(pair) = field(8) {
            mail::push_fields(tokens, pair, &WORDS);
        } else if let Some(address) = field(9) {
            tokens.push(Token::new(address, TokenKind::Ip));
        } else {
            tokens.push(Token::new(whole.as_str(), TokenKind::Number));
        }

        cursor = whole.end();
    }

    mail::push_fields(tokens, &body[cursor..], &WORDS);
}
