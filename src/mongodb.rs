//! MongoDB log parsing
//!
//! MongoDB 4.4 and later write one JSON object per line, as in
//! `{"t":{"$date":"2023-10-03T12:00:01.123+00:00"},"s":"I", "c":"NETWORK",
//! "id":22943, "ctx":"listener","msg":"Connection accepted","attr":{...}}`.
//! The object is read by the shared JSON scanner, and the severity in `s`
//! (`F`, `E`, `W`, `I`, or `D1` to `D5`) is colored by how severe it is.
//! Earlier releases write the same fields as text:
//! `2019-10-03T12:00:01.123+0000 I  NETWORK  [listener] connection accepted`.
use crate::json;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "mongodb";

/// A line in the text format MongoDB wrote before 4.4
static TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[-+]\d{2}:?\d{2}))   # time
        (\ )([FEWI]|D\d?)                   # severity
        (\s+)(\S+)                          # component
        (\s+)(\[)([^\]]*)(\])               # context
        (.*)                                # message, with its leading space
        $
        ",
    )
    .unwrap()
});

/// The MongoDB log plugin
pub struct MongodbPlugin {
    metadata: PluginMetadata,
}

impl MongodbPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "MongoDB structured and text logs",
                "splash",
            ),
        }
    }
}

impl Default for MongodbPlugin {
    fn default() -> Self {
        MongodbPlugin::new()
    }
}

impl Plugin for MongodbPlugin {
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

/// The kind a severity is colored in: `F` and `E` are failures, `W` is a
/// warning, and the rest are levels
pub fn severity_kind(severity: &str) -> TokenKind {
    match severity {
        "F" | "E" => TokenKind::Failure,
        "W" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// The kind a value of a structured log line is colored in, by its key
pub fn value_kind(key: &str, value: &str) -> Option<TokenKind> {
    match key {
        "$date" => Some(TokenKind::Timestamp),
        "s" => Some(severity_kind(value)),
        "c" | "ns" | "db" => Some(TokenKind::Module),
        "id" => Some(TokenKind::Transaction),
        "ctx" | "svc" | "tags" => Some(TokenKind::Tag),
        "msg" => Some(TokenKind::Message),
        "remote" | "session_remote" | "address" | "host" => Some(TokenKind::Host),
        "user" => Some(TokenKind::UserId),
        "durationMillis" | "workingMillis" => Some(TokenKind::Duration),
        "errmsg" | "error" => Some(TokenKind::Failure),
        _ => None,
    }
}

/// Parses one MongoDB log line, structured or text, or returns `None` when
/// the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    json::parse_object(line, value_kind).or_else(|| parse_text_line(line))
}

/// Parses a line in the text format MongoDB wrote before 4.4.
pub fn parse_text_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = TEXT.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), severity_kind(field(3))),
        Token::new(field(4), TokenKind::Plain),
        Token::new(field(5), TokenKind::Module),
        Token::new(field(6), TokenKind::Plain),
        Token::new(field(7), TokenKind::Punctuation),
    ];

    if !field(8).is_empty() {
        tokens.push(Token::new(field(8), TokenKind::Tag));
    }

    tokens.push(Token::new(field(9), TokenKind::Punctuation));
    syslog::push_text(&mut tokens, field(10));

    Some(ParsedLine::new(tokens))
}
