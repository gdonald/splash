//! Squid access log parsing
//!
//! Squid writes its access log in one of two shapes. The native format leads
//! with a Unix timestamp and pads its fields to fixed widths, and the
//! httpd-emulated format that `emulate_httpd_log` turns on is the Common Log
//! Format with the cache result and hierarchy appended. Both color the result
//! code by whether the request was served from the cache.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "squid";

/// The native access log, whose fields are separated by runs of spaces that
/// pad each one to a fixed width
static NATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\s*)                  # leading padding
        (\d+\.\d+)             # timestamp
        (\s+)
        (\d+)                  # elapsed milliseconds
        (\s+)
        (\S+)                  # client
        (\s+)
        ([A-Z_]+)/(\d+)        # result code and status
        (\s+)
        (\d+|-)                # bytes
        (\s+)
        ([A-Z]+)               # method
        (\s+)
        (\S+)                  # url
        (\s+)
        (\S+)                  # user
        (\s+)
        ([A-Z_]+)/(\S+)        # hierarchy code and server
        (\s+)
        (\S+)                  # content type
        (.*)                   # any fields a later squid adds
        $",
    )
    .unwrap()
});

/// The httpd-emulated access log: the Common Log Format with the cache result
/// and the hierarchy code appended
static EMULATED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        ^
        (\S+)\                        # client
        (\S+)\                        # user identifier
        (\S+)\                        # userid
        (\[[^\]]*\])\                 # timestamp
        "([A-Z]+)\ (\S+)\ (\S+)"\     # method, url, protocol
        (\d{3})\                      # status
        (\d+|-)\                      # bytes
        ([A-Z_]+):(\S+)               # result code and hierarchy code
        $"#,
    )
    .unwrap()
});

/// The Squid access log plugin
pub struct SquidPlugin {
    metadata: PluginMetadata,
}

impl SquidPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Squid access logs, native and httpd-emulated",
                "splash",
            ),
        }
    }
}

impl Default for SquidPlugin {
    fn default() -> Self {
        SquidPlugin::new()
    }
}

impl Plugin for SquidPlugin {
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

/// Parses one Squid access log line, or returns `None` when the line is
/// neither of the two formats.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_native_line(line).or_else(|| parse_emulated_line(line))
}

/// Parses a native Squid access log line.
pub fn parse_native_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = NATIVE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = Vec::new();

    if !field(1).is_empty() {
        tokens.push(Token::new(field(1), TokenKind::Plain));
    }

    tokens.extend([
        Token::new(field(2), TokenKind::Timestamp),
        Token::new(field(3), TokenKind::Plain),
        Token::new(field(4), TokenKind::Duration),
        Token::new(field(5), TokenKind::Plain),
        Token::new(field(6), TokenKind::Client),
        Token::new(field(7), TokenKind::Plain),
        Token::new(field(8), cache_kind(field(8))),
        Token::new("/", TokenKind::Punctuation),
        Token::new(field(9), TokenKind::Status),
        Token::new(field(10), TokenKind::Plain),
        Token::new(field(11), TokenKind::Size),
        Token::new(field(12), TokenKind::Plain),
        Token::new(field(13), TokenKind::Method),
        Token::new(field(14), TokenKind::Plain),
        Token::new(field(15), TokenKind::Request),
        Token::new(field(16), TokenKind::Plain),
        Token::new(field(17), TokenKind::UserId),
        Token::new(field(18), TokenKind::Plain),
        Token::new(field(19), TokenKind::Hierarchy),
        Token::new("/", TokenKind::Punctuation),
        Token::new(field(20), TokenKind::Client),
        Token::new(field(21), TokenKind::Plain),
        Token::new(field(22), TokenKind::ContentType),
    ]);

    if !field(23).is_empty() {
        tokens.push(Token::new(field(23), TokenKind::Plain));
    }

    Some(ParsedLine::new(tokens))
}

/// Parses an httpd-emulated Squid access log line.
pub fn parse_emulated_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = EMULATED.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let tokens = vec![
        Token::new(field(1), TokenKind::Client),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::UserIdentifier),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::UserId),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(field(5), TokenKind::Method),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(6), TokenKind::Request),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(7), TokenKind::Protocol),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(8), TokenKind::Status),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(9), TokenKind::Size),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(10), cache_kind(field(10))),
        Token::new(":", TokenKind::Punctuation),
        Token::new(field(11), TokenKind::Hierarchy),
    ];

    Some(ParsedLine::new(tokens))
}

/// The style a result code is painted in, which says whether the request was
/// served from the cache. A code such as `TCP_DENIED` is neither.
fn cache_kind(code: &str) -> TokenKind {
    if code.ends_with("_HIT") || code == "HIT" {
        return TokenKind::CacheHit;
    }

    if code.ends_with("_MISS") || code == "MISS" {
        return TokenKind::CacheMiss;
    }

    TokenKind::CacheResult
}
