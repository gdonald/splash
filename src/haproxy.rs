//! HAProxy log parsing
//!
//! HAProxy writes three kinds of line. `option httplog` adds the status, the
//! captured cookies, and the quoted request to the connection fields,
//! `option tcplog` stops at the connection counters, and an error line names
//! the listener and then says what went wrong. All three can arrive with or
//! without the syslog header that names the host and the process.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::parser::push_message;
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "haproxy";

/// The syslog header, such as `Feb  6 12:14:14 gateway haproxy[14389]: `
static SYSLOG_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        ([A-Z][a-z]{2}\s+\d{1,2}\ \d{2}:\d{2}:\d{2})  # timestamp
        \ (\S+)                                       # host
        \ (\S+)                                       # process
        \[(\d+)\]:\                                   # process id
        ",
    )
    .unwrap()
});

/// The fields every connection line opens with
const CONNECTION_FIELDS: &str = r"(\S+)\ (\[[^\]]*\])\ (\S+)\ ([^/\s]+)/(\S+)\ (\S+)\ ";

/// A connection line written under `option httplog`
static HTTP_LOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?x)^{}{}$",
        CONNECTION_FIELDS, r"(-?\d+)\ (\S+)\ (\S+)\ (\S+)\ (\S{4})\ (\S+)\ (\S+)(.*)"
    ))
    .unwrap()
});

/// A connection line written under `option tcplog`
static TCP_LOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?x)^{}{}$",
        CONNECTION_FIELDS, r"(\S+)\ (\S{2})\ (\S+)\ (\S+)"
    ))
    .unwrap()
});

/// An error line, which names the listener that refused the connection
static ERROR_LOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\S+)\ (\[[^\]]*\])\   # client and accept date
        ([^/\s]+)/(\S+):       # frontend and listener
        (.*)
        $",
    )
    .unwrap()
});

/// The quoted request that closes an `option httplog` line, with whatever
/// captured headers precede it
static HTTP_REQUEST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(.*)"([A-Z]+) (\S+) (\S+)"$"#).unwrap());

/// The HAProxy log plugin
pub struct HaproxyPlugin {
    metadata: PluginMetadata,
}

impl HaproxyPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "HAProxy connection and error logs",
                "splash",
            ),
        }
    }
}

impl Default for HaproxyPlugin {
    fn default() -> Self {
        HaproxyPlugin::new()
    }
}

impl Plugin for HaproxyPlugin {
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

/// Parses one HAProxy log line, with or without its syslog header, or returns
/// `None` when the line is none of the three formats.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = split_syslog_prefix(line);

    let parsed = parse_http_log(body)
        .or_else(|| parse_tcp_log(body))
        .or_else(|| parse_error_log(body))?;

    tokens.extend(parsed.tokens);

    Some(ParsedLine::new(tokens))
}

/// Splits the syslog header off a line, returning its tokens and the HAProxy
/// fields that follow it. A line logged straight to stdout has no header.
pub fn split_syslog_prefix(line: &str) -> (Vec<Token<'_>>, &str) {
    let Some(caps) = SYSLOG_PREFIX.captures(line) else {
        return (Vec::new(), line);
    };

    let field = |index: usize| caps.get(index).unwrap().as_str();

    let tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::Host),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::Tag),
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Pid),
        Token::new("]:", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ];

    (tokens, &line[caps.get(0).unwrap().end()..])
}

/// Parses a connection line written under `option httplog`.
pub fn parse_http_log(line: &str) -> Option<ParsedLine<'_>> {
    let caps = HTTP_LOG.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = connection_tokens(&field);

    tokens.extend([
        Token::new(field(7), TokenKind::Status),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(8), TokenKind::Size),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(9), TokenKind::Header),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(10), TokenKind::Header),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(11), TokenKind::Termination),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(12), TokenKind::Counters),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(13), TokenKind::Counters),
    ]);

    push_request(&mut tokens, field(14));

    Some(ParsedLine::new(tokens))
}

/// Parses a connection line written under `option tcplog`.
pub fn parse_tcp_log(line: &str) -> Option<ParsedLine<'_>> {
    let caps = TCP_LOG.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = connection_tokens(&field);

    tokens.extend([
        Token::new(field(7), TokenKind::Size),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(8), TokenKind::Termination),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(9), TokenKind::Counters),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(10), TokenKind::Counters),
    ]);

    Some(ParsedLine::new(tokens))
}

/// Parses an error line, which names the listener and then the failure.
pub fn parse_error_log(line: &str) -> Option<ParsedLine<'_>> {
    let caps = ERROR_LOG.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Client),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::Frontend),
        Token::new("/", TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Server),
        Token::new(":", TokenKind::Punctuation),
    ];

    push_message(&mut tokens, field(5));

    Some(ParsedLine::new(tokens))
}

/// The fields both connection formats open with: the client, the date the
/// connection was accepted, the proxies it went through, and the timers.
fn connection_tokens<'a>(field: &dyn Fn(usize) -> &'a str) -> Vec<Token<'a>> {
    vec![
        Token::new(field(1), TokenKind::Client),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::Frontend),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4), TokenKind::Backend),
        Token::new("/", TokenKind::Punctuation),
        Token::new(field(5), TokenKind::Server),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(6), TokenKind::Timers),
        Token::new(" ", TokenKind::Plain),
    ]
}

/// Colors the quoted request that closes an `option httplog` line. Anything
/// before it, such as captured headers, is left as message text.
fn push_request<'a>(tokens: &mut Vec<Token<'a>>, rest: &'a str) {
    let gap = rest.len() - rest.trim_start().len();

    if gap > 0 {
        tokens.push(Token::new(&rest[..gap], TokenKind::Plain));
    }

    let rest = &rest[gap..];

    let Some(caps) = HTTP_REQUEST.captures(rest) else {
        push_message(tokens, rest);
        return;
    };

    let field = |index: usize| caps.get(index).unwrap().as_str();

    push_message(tokens, field(1));

    tokens.extend([
        Token::new("\"", TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Method),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::Request),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4), TokenKind::Protocol),
        Token::new("\"", TokenKind::Punctuation),
    ]);
}
