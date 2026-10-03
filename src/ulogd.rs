//! ulogd parsing
//!
//! Netfilter packet logs list the packet's fields as `KEY=value` pairs and
//! bare flags, such as `IN=eth0 OUT= SRC=10.0.0.5 ... PROTO=TCP SPT=52144
//! DPT=22 SYN`, after the prefix the firewall rule set, such as
//! `[UFW BLOCK]`. ulogd's `LOGEMU` output writes them behind a date and the
//! host, and the kernel's `LOG` target sends them to syslog under the
//! `kernel` program, sometimes after the seconds since boot.
use crate::ftp;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "ulogd";

/// The seconds since boot the kernel puts before a message
static UPTIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\[\s*\d+\.\d+\])( )").unwrap());

/// Where the packet fields start
static FIELDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)IN=").unwrap());

/// A word of the packet fields, or the spaces between words
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\S+|\s+").unwrap());

/// A `KEY=value` packet field
static PAIR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Z][A-Z0-9_]*)=(.*)$").unwrap());

/// A bare flag, such as `SYN`, `DF`, or `FRAG:123`
static FLAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z]+(?::\d+)?$").unwrap());

/// Words in a rule's prefix that say what the rule did with the packet
static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("ACCEPT", TokenKind::Success),
        ("ALLOW", TokenKind::Success),
        ("AUDIT", TokenKind::Warning),
        ("LIMIT", TokenKind::Warning),
        ("BLOCK", TokenKind::Failure),
        ("DENY", TokenKind::Failure),
        ("DROP", TokenKind::Failure),
        ("REJECT", TokenKind::Failure),
    ])
});

/// The ulogd plugin
pub struct UlogdPlugin {
    metadata: PluginMetadata,
}

impl UlogdPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Netfilter packet logs from ulogd and the kernel",
                "splash",
            ),
        }
    }
}

impl Default for UlogdPlugin {
    fn default() -> Self {
        UlogdPlugin::new()
    }
}

impl Plugin for UlogdPlugin {
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

/// Parses one packet log line, or returns `None` when the line has no header
/// or no packet fields.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, mut body) = match syslog::split_header(line) {
        Some(header) => (header.tokens, header.body),
        None => syslog::split_bare_header(line)?,
    };

    if let Some(caps) = UPTIME.captures(body) {
        tokens.extend([
            Token::new(caps.get(1).unwrap().as_str(), TokenKind::Timestamp),
            Token::new(caps.get(2).unwrap().as_str(), TokenKind::Plain),
        ]);

        body = &body[caps.get(0).unwrap().end()..];
    }

    let start = FIELDS.find(body)?.end() - "IN=".len();

    mail::push_words(&mut tokens, &body[..start], &WORDS);
    push_fields(&mut tokens, &body[start..]);

    Some(ParsedLine::new(tokens))
}

/// The kind a field's value is colored in, by the field's name
fn value_kind(key: &str, value: &str) -> TokenKind {
    match key {
        "IN" | "OUT" | "PHYSIN" | "PHYSOUT" => TokenKind::Module,
        "SRC" | "DST" | "GATEWAY" => ftp::host_kind(value),
        "PROTO" => TokenKind::Protocol,
        "LEN" => TokenKind::Size,
        "UID" | "GID" => TokenKind::UserId,
        "MAC" => TokenKind::Message,
        _ => TokenKind::Number,
    }
}

/// Colors the packet fields: each `KEY=value` pair by its key, and each bare
/// flag as a tag.
fn push_fields<'a>(tokens: &mut Vec<Token<'a>>, fields: &'a str) {
    for word in WORD.find_iter(fields) {
        let text = word.as_str();

        if let Some(caps) = PAIR.captures(text) {
            let key = caps.get(1).unwrap().as_str();
            let value = caps.get(2).unwrap().as_str();

            tokens.extend([
                Token::new(key, TokenKind::Header),
                Token::new("=", TokenKind::Punctuation),
            ]);

            if !value.is_empty() {
                tokens.push(Token::new(value, value_kind(key, value)));
            }
        } else if FLAG.is_match(text) {
            tokens.push(Token::new(text, TokenKind::Tag));
        } else if text.trim().is_empty() {
            tokens.push(Token::new(text, TokenKind::Plain));
        } else {
            tokens.push(Token::new(text, TokenKind::Message));
        }
    }
}
