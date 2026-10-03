//! auth.log parsing
//!
//! `auth.log` (`secure` on Red Hat systems) collects the syslog lines of the
//! programs that authenticate users: sshd, sudo, su, login, the PAM modules
//! they load, and systemd-logind. Each line is colored for the user it is
//! about, where they came from, and whether they got in.
use crate::ftp;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "auth";

/// The parts of an authentication message that carry a value: a PAM module
/// and the service and stage it ran for, the command sudo ran, a `key=value`
/// field, the user a phrase is about, and a port
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (pam_\w+)\(([^)]*)\)                                    # PAM module
        |
        \b(COMMAND)=(.*)$                                       # sudo command
        |
        \b([A-Za-z_]+)=([^\s;)]*)                               # field
        |
        (for\ invalid\ user|[Ii]nvalid\ user|by\ authenticating\ user
            |for\ user|of\ user|from\ user|for|by|user|\(to)
        \ ([a-z_](?:[\w.-]*[\w-])?\$?)                          # user
        |
        \b(port)\ (\d+)                                         # port
        ",
    )
    .unwrap()
});

/// The user sudo names before the command it ran, as in `   alice : TTY=pts/0`
static SUDO_USER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*)([^\s:]+)( : )").unwrap());

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Accepted", TokenKind::Success),
        ("session opened", TokenKind::Success),
        ("New session", TokenKind::Success),
        ("Successful su", TokenKind::Success),
        ("Server listening on", TokenKind::Success),
        ("session closed", TokenKind::Warning),
        ("Removed session", TokenKind::Warning),
        ("Disconnected", TokenKind::Warning),
        ("Received disconnect", TokenKind::Warning),
        ("Connection closed", TokenKind::Warning),
        ("Failed password", TokenKind::Failure),
        ("Failed publickey", TokenKind::Failure),
        ("authentication failure", TokenKind::Failure),
        ("FAILED SU", TokenKind::Failure),
        ("incorrect password attempts", TokenKind::Failure),
        ("NOT in sudoers", TokenKind::Failure),
        ("command not allowed", TokenKind::Failure),
        ("a password is required", TokenKind::Failure),
        ("FAILED su", TokenKind::Failure),
        ("Timeout before authentication", TokenKind::Failure),
        ("Unable to negotiate", TokenKind::Failure),
        ("error", TokenKind::Failure),
        ("fatal", TokenKind::Failure),
        (
            "maximum authentication attempts exceeded",
            TokenKind::Failure,
        ),
    ])
});

/// The auth.log plugin
pub struct AuthPlugin {
    metadata: PluginMetadata,
}

impl AuthPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Authentication logs from sshd, sudo, su, and PAM",
                "splash",
            ),
        }
    }
}

impl Default for AuthPlugin {
    fn default() -> Self {
        AuthPlugin::new()
    }
}

impl Plugin for AuthPlugin {
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

/// Parses one auth.log line, or returns `None` when the line has no syslog
/// header.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;
    let mut tokens = header.tokens;
    let mut body = header.body;

    if header.program == "sudo" {
        body = split_sudo_user(&mut tokens, body);
    }

    push_message(&mut tokens, body);

    Some(ParsedLine::new(tokens))
}

/// Colors the user sudo names before the command it ran, as in
/// `   alice : TTY=pts/0`, and returns the rest of the message. A message
/// that names no user is returned whole.
pub fn split_sudo_user<'a>(tokens: &mut Vec<Token<'a>>, body: &'a str) -> &'a str {
    let Some(caps) = SUDO_USER.captures(body) else {
        return body;
    };

    let indent = caps.get(1).unwrap().as_str();

    if !indent.is_empty() {
        tokens.push(Token::new(indent, TokenKind::Plain));
    }

    tokens.extend([
        Token::new(caps.get(2).unwrap().as_str(), TokenKind::UserId),
        Token::new(caps.get(3).unwrap().as_str(), TokenKind::Punctuation),
    ]);

    &body[caps.get(0).unwrap().end()..]
}

/// The kind a field's value is colored in, by the field's name
fn field_kind(key: &str, value: &str) -> TokenKind {
    match key {
        "USER" | "user" | "ruser" | "logname" => TokenKind::UserId,
        "PWD" => TokenKind::Path,
        "rhost" => ftp::host_kind(value),
        "uid" | "euid" => TokenKind::Number,
        _ => TokenKind::Message,
    }
}

/// Colors an authentication message. Other plugins whose lines carry PAM
/// messages, such as cron, color them with this too.
pub fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(module) = field(1) {
            tokens.extend([
                Token::new(module, TokenKind::Module),
                Token::new("(", TokenKind::Punctuation),
            ]);

            if !field(2).unwrap().is_empty() {
                tokens.push(Token::new(field(2).unwrap(), TokenKind::Module));
            }

            tokens.push(Token::new(")", TokenKind::Punctuation));
        } else if let Some(key) = field(3) {
            tokens.extend([
                Token::new(key, TokenKind::Header),
                Token::new("=", TokenKind::Punctuation),
                Token::new(field(4).unwrap(), TokenKind::Request),
            ]);
        } else if let Some(key) = field(5) {
            let value = field(6).unwrap();

            tokens.extend([
                Token::new(key, TokenKind::Header),
                Token::new("=", TokenKind::Punctuation),
            ]);

            if !value.is_empty() {
                tokens.push(Token::new(value, field_kind(key, value)));
            }
        } else if let Some(phrase) = field(7) {
            let kind = if phrase.contains("nvalid user") {
                TokenKind::Failure
            } else {
                TokenKind::Message
            };

            tokens.extend([
                Token::new(phrase, kind),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(8).unwrap(), TokenKind::UserId),
            ]);
        } else {
            tokens.extend([
                Token::new(field(9).unwrap(), TokenKind::Message),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(10).unwrap(), TokenKind::Number),
            ]);
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
