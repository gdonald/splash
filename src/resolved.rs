//! systemd-resolved parsing
//!
//! systemd-resolved logs through the journal under `systemd-resolved`. It
//! reports the DNS servers it switches between, the feature levels it falls
//! back to when a server misbehaves, as in `Using degraded feature set UDP
//! instead of UDP+EDNS0 for DNS server 10.0.0.53.`, and the trust anchors it
//! loads, as `. IN DS 20326 8 2 e06d44b8...`.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::net::Ipv6Addr;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "systemd-resolved";

/// The parts of a message that carry a value: a resource record, a feature
/// level, something shaped like an IPv6 address, and a quoted name
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (\S+)(\ IN\ )([A-Z]+[0-9]*)\b                      # record
        |
        \b((?:UDP|TLS)\+EDNS0(?:\+DO)?|UDP|TCP|TLS)\b      # feature level
        |
        ((?:[0-9a-fA-F]{1,4}:){2,7}:?[0-9a-fA-F]{0,4})     # IPv6 address
        |
        (')([^']*)(')                                      # quoted name
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        (
            "Grace period over, resuming full feature set",
            TokenKind::Success,
        ),
        ("Using degraded feature set", TokenKind::Warning),
        ("downgrading feature level", TokenKind::Warning),
        ("downgrading protocol", TokenKind::Warning),
        ("mitigating potential DNS violation", TokenKind::Warning),
        ("Switching to", TokenKind::Warning),
        ("Positive Trust Anchors", TokenKind::Header),
        ("Negative trust anchors", TokenKind::Header),
        ("NXDOMAIN", TokenKind::Failure),
        ("SERVFAIL", TokenKind::Failure),
        ("REFUSED", TokenKind::Failure),
        ("Failed", TokenKind::Failure),
        ("failed", TokenKind::Failure),
    ])
});

/// The systemd-resolved plugin
pub struct ResolvedPlugin {
    metadata: PluginMetadata,
}

impl ResolvedPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "systemd-resolved DNS resolution logs",
                "splash",
            ),
        }
    }
}

impl Default for ResolvedPlugin {
    fn default() -> Self {
        ResolvedPlugin::new()
    }
}

impl Plugin for ResolvedPlugin {
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

/// Parses one systemd-resolved line, or returns `None` when the line was not
/// logged by systemd-resolved.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if header.program != NAME {
        return None;
    }

    let mut tokens = header.tokens;

    push_message(&mut tokens, header.body);

    Some(ParsedLine::new(tokens))
}

/// Colors a message.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(name) = field(1) {
            tokens.extend([
                Token::new(name, TokenKind::Host),
                Token::new(field(2).unwrap(), TokenKind::Plain),
                Token::new(field(3).unwrap(), TokenKind::Protocol),
            ]);
        } else if let Some(level) = field(4) {
            tokens.push(Token::new(level, TokenKind::Protocol));
        } else if let Some(address) = field(5) {
            let kind = if address.parse::<Ipv6Addr>().is_ok() {
                TokenKind::Ip
            } else {
                TokenKind::Message
            };

            tokens.push(Token::new(address, kind));
        } else {
            tokens.push(Token::new(field(6).unwrap(), TokenKind::Punctuation));

            if !field(7).unwrap().is_empty() {
                tokens.push(Token::new(field(7).unwrap(), TokenKind::Host));
            }

            tokens.push(Token::new(field(8).unwrap(), TokenKind::Punctuation));
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
