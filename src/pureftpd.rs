//! pure-ftpd log parsing
//!
//! pure-ftpd sends its messages to syslog as `(user@host) [LEVEL] message`,
//! with `?` standing in for a user or host it does not know yet. A finished
//! transfer is logged as the file, `downloaded` or `uploaded`, and the bytes
//! and rate. Its `AltLog` transfer log is read here in the `clf` and `w3c`
//! formats. The `stats` and `xferlog` formats have plugins of their own.
use crate::ftp;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "pure-ftpd";

/// The user, host, and level that open a syslog message
static SESSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        \((.*)@([^@)]*)\)                                       # user and host
        \ (?:\[(INFO|NOTICE|WARNING|ERROR|DEBUG)\]\ )?          # level
        ",
    )
    .unwrap()
});

/// A finished transfer: the file, what happened to it, its size, and the
/// rate
static TRANSFER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (.+?)
        \ (downloaded|uploaded|partially\ uploaded)
        (\s+\()(\d+)(\ bytes,\ )(\d+(?:\.\d+)?)(\S*/sec)(\))
        $
        ",
    )
    .unwrap()
});

/// A login, with the user who logged in
static LOGGED_IN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\S+)( )(is now logged in)$").unwrap());

/// A failed login, with the user in brackets
static AUTH_FAILED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(Authentication failed)( for user )\[([^\]]*)\]$").unwrap());

/// A `w3c` transfer log header, such as `#Fields: date time c-ip`
static W3C_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#)([A-Za-z-]+)(:)(.*)$").unwrap());

/// A `w3c` transfer log record
static W3C_RECORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{4}-\d{2}-\d{2}\ \d{2}:\d{2}:\d{2})   # date and time
        \ (\S+)                                  # remote host
        \ (\[\])(\S+)                            # action
        \ (.+)                                   # file
        \ (\d{3})                                # status
        \ (\S+)                                  # user
        \ (\d+)                                  # size
        $
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Anonymous user logged in", TokenKind::Success),
        ("Timeout", TokenKind::Warning),
        ("Login authentication failed", TokenKind::Failure),
        ("Could not delete", TokenKind::Failure),
    ])
});

/// The pure-ftpd log plugin
pub struct PureftpdPlugin {
    metadata: PluginMetadata,
}

impl PureftpdPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "pure-ftpd syslog and transfer logs",
                "splash",
            ),
        }
    }
}

impl Default for PureftpdPlugin {
    fn default() -> Self {
        PureftpdPlugin::new()
    }
}

impl Plugin for PureftpdPlugin {
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

/// Parses one pure-ftpd log line, from syslog or from a `clf` or `w3c`
/// transfer log, or returns `None` when the line is none of them.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_syslog_line(line)
        .or_else(|| ftp::parse_transfer_line(line))
        .or_else(|| parse_w3c_header(line))
        .or_else(|| parse_w3c_record(line))
}

/// Parses a line pure-ftpd sent to syslog.
pub fn parse_syslog_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if header.program != NAME {
        return None;
    }

    let mut tokens = header.tokens;
    let mut body = header.body;

    if let Some(caps) = SESSION.captures(body) {
        let user = caps.get(1).unwrap().as_str();
        let host = caps.get(2).unwrap().as_str();

        tokens.push(Token::new("(", TokenKind::Punctuation));

        if !user.is_empty() {
            tokens.push(Token::new(user, TokenKind::UserId));
        }

        tokens.push(Token::new("@", TokenKind::Punctuation));

        if !host.is_empty() {
            tokens.push(Token::new(host, ftp::host_kind(host)));
        }

        tokens.extend([
            Token::new(")", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);

        if let Some(level) = caps.get(3) {
            tokens.extend([
                Token::new("[", TokenKind::Punctuation),
                Token::new(level.as_str(), TokenKind::Level),
                Token::new("]", TokenKind::Punctuation),
                Token::new(" ", TokenKind::Plain),
            ]);
        }

        body = &body[caps.get(0).unwrap().end()..];
    }

    push_message(&mut tokens, body);

    Some(ParsedLine::new(tokens))
}

/// Colors a syslog message: a transfer, a login, a failed login, or free text.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    if let Some(caps) = TRANSFER.captures(message) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        let outcome = if field(2) == "partially uploaded" {
            TokenKind::Warning
        } else {
            TokenKind::Success
        };

        tokens.extend([
            Token::new(field(1), TokenKind::Path),
            Token::new(" ", TokenKind::Plain),
            Token::new(field(2), outcome),
            Token::new(field(3), TokenKind::Punctuation),
            Token::new(field(4), TokenKind::Size),
            Token::new(field(5), TokenKind::Message),
            Token::new(field(6), TokenKind::Number),
            Token::new(field(7), TokenKind::Message),
            Token::new(field(8), TokenKind::Punctuation),
        ]);
    } else if let Some(caps) = LOGGED_IN.captures(message) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(field(1), TokenKind::UserId),
            Token::new(field(2), TokenKind::Plain),
            Token::new(field(3), TokenKind::Success),
        ]);
    } else if let Some(caps) = AUTH_FAILED.captures(message) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(field(1), TokenKind::Failure),
            Token::new(field(2), TokenKind::Message),
            Token::new("[", TokenKind::Punctuation),
        ]);

        if !field(3).is_empty() {
            tokens.push(Token::new(field(3), TokenKind::UserId));
        }

        tokens.push(Token::new("]", TokenKind::Punctuation));
    } else {
        mail::push_words(tokens, message, &WORDS);
    }
}

/// Parses a header line of a `w3c` transfer log.
pub fn parse_w3c_header(line: &str) -> Option<ParsedLine<'_>> {
    let caps = W3C_HEADER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Header),
        Token::new(field(3), TokenKind::Punctuation),
    ];

    if !field(4).is_empty() {
        tokens.push(Token::new(field(4), TokenKind::Message));
    }

    Some(ParsedLine::new(tokens))
}

/// Parses a record line of a `w3c` transfer log.
pub fn parse_w3c_record(line: &str) -> Option<ParsedLine<'_>> {
    let caps = W3C_RECORD.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    Some(ParsedLine::new(vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), ftp::host_kind(field(2))),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Method),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(5), TokenKind::Path),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(6), TokenKind::Status),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(7), TokenKind::UserId),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(8), TokenKind::Size),
    ]))
}
