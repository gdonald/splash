//! Varnish log parsing
//!
//! `varnishlog` writes one transaction at a time. A header names the
//! transaction and its id, and the records under it are indented with one
//! dash per level of nesting. `varnishlog -g raw` writes the same records
//! unheaded, each prefixed with its transaction id and the side it came from.
//! Each record's payload is colored by the tag that introduces it.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::parser::push_message;
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "varnish";

/// The line that opens a transaction, such as `*   << Request  >> 32770`
static TRANSACTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\*+)(\s+)     # nesting depth
        (<<)(\s*)
        (\S+)          # transaction type
        (\s*)(>>)(\s+)
        (\d+)          # transaction id
        $",
    )
    .unwrap()
});

/// One record under a transaction, such as `-   ReqMethod      GET`
static RECORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (-+)(\s+)      # nesting depth
        (\w+)          # tag
        (\s*)
        (.*)           # payload
        $",
    )
    .unwrap()
});

/// One record of `varnishlog -g raw`, which leads with the transaction id and
/// the side the record came from
static RAW_RECORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\s*)
        (\d+)          # transaction id
        (\s+)
        (\w+)          # tag
        (\s+)
        ([bc-])        # backend, client, or neither
        (\s*)
        (.*)           # payload
        $",
    )
    .unwrap()
});

/// The Varnish log plugin
pub struct VarnishPlugin {
    metadata: PluginMetadata,
}

impl VarnishPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Varnish request and response logs",
                "splash",
            ),
        }
    }
}

impl Default for VarnishPlugin {
    fn default() -> Self {
        VarnishPlugin::new()
    }
}

impl Plugin for VarnishPlugin {
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

/// Parses one Varnish log line, or returns `None` when the line is neither a
/// transaction header nor a record.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_transaction_line(line)
        .or_else(|| parse_record_line(line))
        .or_else(|| parse_raw_record_line(line))
}

/// Parses the line that opens a transaction.
pub fn parse_transaction_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = TRANSACTION.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), TokenKind::Punctuation),
    ];

    push_gap(&mut tokens, field(4));
    tokens.push(Token::new(field(5), TokenKind::Transaction));
    push_gap(&mut tokens, field(6));
    tokens.push(Token::new(field(7), TokenKind::Punctuation));
    tokens.push(Token::new(field(8), TokenKind::Plain));
    tokens.push(Token::new(field(9), TokenKind::Number));

    Some(ParsedLine::new(tokens))
}

/// Parses one record under a transaction.
pub fn parse_record_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = RECORD.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), TokenKind::Tag),
    ];

    push_gap(&mut tokens, field(4));
    push_payload(&mut tokens, field(3), field(5));

    Some(ParsedLine::new(tokens))
}

/// Parses one record of `varnishlog -g raw`.
pub fn parse_raw_record_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = RAW_RECORD.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = Vec::new();

    push_gap(&mut tokens, field(1));

    tokens.extend([
        Token::new(field(2), TokenKind::Number),
        Token::new(field(3), TokenKind::Plain),
        Token::new(field(4), TokenKind::Tag),
        Token::new(field(5), TokenKind::Plain),
        Token::new(field(6), TokenKind::Transaction),
    ]);

    push_gap(&mut tokens, field(7));
    push_payload(&mut tokens, field(4), field(8));

    Some(ParsedLine::new(tokens))
}

/// Keeps the spacing between two fields, which varnishlog pads to a fixed
/// width and which can be empty when a record has no payload.
fn push_gap<'a>(tokens: &mut Vec<Token<'a>>, gap: &'a str) {
    if !gap.is_empty() {
        tokens.push(Token::new(gap, TokenKind::Plain));
    }
}

/// Colors a record's payload according to the tag that introduces it. A tag
/// splash has no field for leaves its payload as message text, with any
/// addresses inside it colored.
fn push_payload<'a>(tokens: &mut Vec<Token<'a>>, tag: &str, payload: &'a str) {
    if payload.is_empty() {
        return;
    }

    if tag.ends_with("Header") {
        push_header(tokens, payload);
        return;
    }

    match payload_kind(tag) {
        Some(kind) => tokens.push(Token::new(payload, kind)),
        None => push_message(tokens, payload),
    }
}

/// The style a payload takes from its tag, for the tags that name a field
/// splash already colors elsewhere.
fn payload_kind(tag: &str) -> Option<TokenKind> {
    match tag {
        "ReqMethod" | "BereqMethod" => Some(TokenKind::Method),
        "ReqURL" | "BereqURL" => Some(TokenKind::Request),
        "ReqProtocol" | "RespProtocol" | "BereqProtocol" | "BerespProtocol" => {
            Some(TokenKind::Protocol)
        }
        "RespStatus" | "BerespStatus" => Some(TokenKind::Status),
        "Timestamp" => Some(TokenKind::Timestamp),
        _ => None,
    }
}

/// Splits an HTTP header payload so the header name is colored apart from its
/// value.
fn push_header<'a>(tokens: &mut Vec<Token<'a>>, payload: &'a str) {
    match payload.find(':') {
        Some(split) => {
            tokens.push(Token::new(&payload[..split], TokenKind::Header));
            tokens.push(Token::new(":", TokenKind::Punctuation));
            push_message(tokens, &payload[split + 1..]);
        }
        None => push_message(tokens, payload),
    }
}
