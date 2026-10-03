//! Dovecot log parsing
//!
//! Dovecot names the service that wrote each message, such as `imap-login` or
//! `lmtp`, with the user, process id, and session id it concerns in brackets
//! after it: `imap(alice)<4321><Xy7AbC>: `. A level follows in Dovecot's own
//! log file and in syslog for anything above info. The message itself is
//! free text with `key=value` fields, where `user=`, `rip=`, and `session=`
//! name who connected, from where, and in which session.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "dovecot";

/// The timestamp Dovecot's own log file leads with, set by `log_timestamp`
/// and `%b %d %H:%M:%S` by default
static TIMESTAMP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Z][a-z]{2} \d{2} \d{2}:\d{2}:\d{2}) ").unwrap());

/// The service that wrote the message, with the user or process id in
/// parentheses and the process and session ids in angle brackets
static SERVICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        ([a-z][\w-]*)               # service
        (?:\(([^)]+)\))?            # user or process id
        ((?:<[^<>\s]+>)*)           # process and session ids
        :\x20
        (?:(Debug|Info|Warning|Error|Fatal|Panic):\ )?
        ",
    )
    .unwrap()
});

/// One of the angle bracketed ids after the service
static BRACKETED_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<([^<>]+)>").unwrap());

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Login", TokenKind::Success),
        ("saved mail to", TokenKind::Success),
        ("Aborted login", TokenKind::Failure),
        ("auth failed", TokenKind::Failure),
        ("Password mismatch", TokenKind::Failure),
        ("unknown user", TokenKind::Failure),
        ("Quota exceeded", TokenKind::Failure),
    ])
});

/// The Dovecot log plugin
pub struct DovecotPlugin {
    metadata: PluginMetadata,
}

impl DovecotPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Dovecot IMAP, POP3, and LMTP logs",
                "splash",
            ),
        }
    }
}

impl Default for DovecotPlugin {
    fn default() -> Self {
        DovecotPlugin::new()
    }
}

impl Plugin for DovecotPlugin {
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

/// Parses one Dovecot log line, from syslog or from Dovecot's own log file,
/// or returns `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = split_syslog_header(line).or_else(|| split_timestamp(line))?;
    let caps = SERVICE.captures(body)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());

    tokens.push(Token::new(field(1).unwrap(), TokenKind::Module));

    if let Some(owner) = field(2) {
        tokens.push(Token::new("(", TokenKind::Punctuation));
        push_owner(&mut tokens, owner);
        tokens.push(Token::new(")", TokenKind::Punctuation));
    }

    push_ids(&mut tokens, field(3).unwrap());

    tokens.extend([
        Token::new(":", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ]);

    if let Some(level) = field(4) {
        tokens.extend([
            Token::new(level, TokenKind::Level),
            Token::new(":", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);
    }

    mail::push_fields(&mut tokens, &body[caps.get(0).unwrap().end()..], &WORDS);

    Some(ParsedLine::new(tokens))
}

/// Splits the syslog header off a line Dovecot sent to syslog.
fn split_syslog_header(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let header = syslog::split_header(line)?;

    if header.program != "dovecot" {
        return None;
    }

    Some((header.tokens, header.body))
}

/// Splits the timestamp off a line from Dovecot's own log file.
fn split_timestamp(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let caps = TIMESTAMP.captures(line)?;

    let tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
    ];

    Some((tokens, &line[caps.get(0).unwrap().end()..]))
}

/// Colors what the service names in parentheses: a process id when it is all
/// digits, and otherwise a user.
fn push_owner<'a>(tokens: &mut Vec<Token<'a>>, owner: &'a str) {
    let kind = if owner.bytes().all(|byte| byte.is_ascii_digit()) {
        TokenKind::Pid
    } else {
        TokenKind::UserId
    };

    tokens.push(Token::new(owner, kind));
}

/// Colors the angle bracketed ids after the service. The first is the
/// process id and the second the session id.
fn push_ids<'a>(tokens: &mut Vec<Token<'a>>, ids: &'a str) {
    for (position, caps) in BRACKETED_ID.captures_iter(ids).enumerate() {
        let kind = if position == 0 {
            TokenKind::Pid
        } else {
            TokenKind::Transaction
        };

        tokens.extend([
            Token::new("<", TokenKind::Punctuation),
            Token::new(caps.get(1).unwrap().as_str(), kind),
            Token::new(">", TokenKind::Punctuation),
        ]);
    }
}
