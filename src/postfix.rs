//! Postfix log parsing
//!
//! Every Postfix daemon logs through syslog under a program name such as
//! `postfix/smtpd` or `postfix/qmgr`. A message about a queued mail leads with
//! its queue id, and the transaction is told in `key=value` fields: `from=`
//! and `to=` name the addresses, `relay=` the next hop, and `status=` whether
//! the mail was sent, deferred, or bounced.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "postfix";

/// The queue id a message leads with. A short id is hexadecimal, a long id
/// (`enable_long_queue_ids`) is at least ten letters and digits, a `z`, and
/// more of them, and `NOQUEUE` marks a mail rejected before it was queued.
static QUEUE_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([0-9A-F]{5,}|[0-9A-Za-y]{10,}z[0-9A-Za-y]+|NOQUEUE): ").unwrap()
});

/// The severity or action a message leads with after its queue id
static LEVEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(warning|error|fatal|panic|reject|discard|hold|milter-reject): ").unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("removed", TokenKind::Success),
        ("lost connection", TokenKind::Warning),
        ("timeout", TokenKind::Warning),
        ("Relay access denied", TokenKind::Failure),
        ("Recipient address rejected", TokenKind::Failure),
        ("Sender address rejected", TokenKind::Failure),
        ("authentication failed", TokenKind::Failure),
    ])
});

/// The Postfix log plugin
pub struct PostfixPlugin {
    metadata: PluginMetadata,
}

impl PostfixPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Postfix mail queue, SMTP, and delivery logs",
                "splash",
            ),
        }
    }
}

impl Default for PostfixPlugin {
    fn default() -> Self {
        PostfixPlugin::new()
    }
}

impl Plugin for PostfixPlugin {
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

/// Parses one Postfix log line, or returns `None` when the line was not
/// logged by a Postfix daemon.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = mail::split_syslog_header(line)?;

    if !header.program.starts_with("postfix") {
        return None;
    }

    let mut tokens = header.tokens;
    let mut body = header.body;

    if let Some(caps) = QUEUE_ID.captures(body) {
        let id = caps.get(1).unwrap().as_str();

        tokens.extend([
            Token::new(id, TokenKind::QueueId),
            Token::new(":", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);

        body = &body[caps.get(0).unwrap().end()..];
    }

    if let Some(caps) = LEVEL.captures(body) {
        let level = caps.get(1).unwrap().as_str();

        tokens.extend([
            Token::new(level, level_kind(level)),
            Token::new(":", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);

        body = &body[caps.get(0).unwrap().end()..];
    }

    mail::push_fields(&mut tokens, body, &WORDS);

    Some(ParsedLine::new(tokens))
}

/// The kind a leading severity or action is colored in. A warning or a held
/// mail needs attention, and anything else on the list is a failure.
pub fn level_kind(level: &str) -> TokenKind {
    match level {
        "warning" | "hold" => TokenKind::Warning,
        _ => TokenKind::Failure,
    }
}
