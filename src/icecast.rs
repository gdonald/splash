//! Icecast log parsing
//!
//! Icecast 2's error log opens each line with the date, the level, and the
//! module and function that logged it, as `[2023-10-03  12:00:01] INFO
//! main/main Icecast 2.4.4 server started`. Its access log is the Combined
//! Log Format with the seconds the listener stayed connected added at the
//! end. Icecast 1 wrote `[03/Oct/2023:12:00:01] [1:main] message`, and a
//! periodic usage line with the bandwidth and the source, client, and admin
//! counts.
use crate::httpd;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "icecast";

/// An Icecast 2 error log line
static ERROR_LOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\[)(\d{4}-\d{2}-\d{2}  \d{2}:\d{2}:\d{2})(\])( )(EROR|WARN|INFO|DBUG)( )(\S+)(.*)$",
    )
    .unwrap()
});

/// The date and thread an Icecast 1 line opens with, and the `Admin` mark
/// on lines about an admin connection
static OLD_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\[)(\d+/[A-Z][a-z]{2}/\d+:\d+:\d+:\d+)(\])( )(?:(Admin)( *))?(\[)(\d+)?(:)?([^\]]*)(\])( )")
        .unwrap()
});

/// An Icecast 1 usage line, after its prefix
static USAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\[)(\d+/[A-Z][a-z]{2}/\d+:\d+:\d+:\d+)(\])( )(Bandwidth)(:)([\d.]+)(\S*)( )(Sources)(:)(\d+)( )(Clients)(:)(\d+)( )(Admins)(:)(\d+)(.*)$",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("server started", TokenKind::Success),
        ("Source logging in", TokenKind::Success),
        ("source shutdown", TokenKind::Warning),
        ("Disconnecting", TokenKind::Warning),
        ("failed", TokenKind::Failure),
        ("Failed", TokenKind::Failure),
        ("error", TokenKind::Failure),
        ("Error", TokenKind::Failure),
    ])
});

/// The Icecast plugin
pub struct IcecastPlugin {
    metadata: PluginMetadata,
}

impl IcecastPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Icecast streaming server logs",
                "splash",
            ),
        }
    }
}

impl Default for IcecastPlugin {
    fn default() -> Self {
        IcecastPlugin::new()
    }
}

impl Plugin for IcecastPlugin {
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

/// Parses one Icecast log line, or returns `None` when the line is in none of
/// its formats.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_error_line(line)
        .or_else(|| httpd::parse_combined_line(line))
        .or_else(|| parse_old_line(line))
}

/// The kind an Icecast 2 level is colored in
fn level_kind(level: &str) -> TokenKind {
    match level {
        "EROR" => TokenKind::Failure,
        "WARN" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// Parses an Icecast 2 error log line.
pub fn parse_error_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = ERROR_LOG.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Timestamp),
        Token::new(field(3), TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Plain),
        Token::new(field(5), level_kind(field(5))),
        Token::new(field(6), TokenKind::Plain),
        Token::new(field(7), TokenKind::Module),
    ];

    mail::push_words(&mut tokens, field(8), &WORDS);

    Some(ParsedLine::new(tokens))
}

/// Parses an Icecast 1 line, a usage line or a message.
pub fn parse_old_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = OLD_PREFIX.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());
    let rest = &line[caps.get(0).unwrap().end()..];

    let mut tokens = vec![
        Token::new(field(1).unwrap(), TokenKind::Punctuation),
        Token::new(field(2).unwrap(), TokenKind::Timestamp),
        Token::new(field(3).unwrap(), TokenKind::Punctuation),
        Token::new(field(4).unwrap(), TokenKind::Plain),
    ];

    if let Some(admin) = field(5) {
        tokens.push(Token::new(admin, TokenKind::Tag));

        if !field(6).unwrap().is_empty() {
            tokens.push(Token::new(field(6).unwrap(), TokenKind::Plain));
        }
    }

    tokens.push(Token::new(field(7).unwrap(), TokenKind::Punctuation));

    if let Some(thread) = field(8) {
        tokens.push(Token::new(thread, TokenKind::Number));
    }

    if let Some(colon) = field(9) {
        tokens.push(Token::new(colon, TokenKind::Punctuation));
    }

    if !field(10).unwrap().is_empty() {
        tokens.push(Token::new(field(10).unwrap(), TokenKind::Module));
    }

    tokens.extend([
        Token::new(field(11).unwrap(), TokenKind::Punctuation),
        Token::new(field(12).unwrap(), TokenKind::Plain),
    ]);

    match USAGE.captures(rest) {
        Some(usage) => push_usage(&mut tokens, &usage),
        None => mail::push_words(&mut tokens, rest, &WORDS),
    }

    Some(ParsedLine::new(tokens))
}

/// Colors an Icecast 1 usage line's date, bandwidth, and counts.
fn push_usage<'a>(tokens: &mut Vec<Token<'a>>, usage: &regex::Captures<'a>) {
    let kinds = [
        TokenKind::Punctuation,
        TokenKind::Timestamp,
        TokenKind::Punctuation,
        TokenKind::Plain,
        TokenKind::Header,
        TokenKind::Punctuation,
        TokenKind::Number,
        TokenKind::Message,
        TokenKind::Plain,
        TokenKind::Header,
        TokenKind::Punctuation,
        TokenKind::Number,
        TokenKind::Plain,
        TokenKind::Header,
        TokenKind::Punctuation,
        TokenKind::Number,
        TokenKind::Plain,
        TokenKind::Header,
        TokenKind::Punctuation,
        TokenKind::Number,
        TokenKind::Message,
    ];

    for (index, kind) in kinds.into_iter().enumerate() {
        let text = usage.get(index + 1).unwrap().as_str();

        if !text.is_empty() {
            tokens.push(Token::new(text, kind));
        }
    }
}
