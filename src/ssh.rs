//! OpenSSH server log parsing
//!
//! sshd logs through syslog under `sshd`, or `sshd-session` for the per
//! connection process of OpenSSH 9.8 and later. A login attempt is logged as
//! `Accepted publickey for alice from 10.0.0.5 port 52144 ssh2: ED25519
//! SHA256:...`, with `Failed`, `Postponed`, or `Partial` in place of
//! `Accepted`. Lines from before authentication end in `[preauth]`. Users,
//! addresses, ports, and PAM lines are colored as in the auth mode.
use crate::auth;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "ssh";

/// The parts of an sshd message auth does not color: the outcome and method
/// of a login attempt, the key type and fingerprint, the protocol, and the
/// `[preauth]` tag
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        \b(Accepted|Failed|Postponed|Partial)\ ([\w/-]+)(?:\ for\b)   # outcome and method
        |
        \b(ssh2)\b                                                  # protocol
        |
        \b([A-Z0-9-]+(?:-CERT)?)\ (SHA256:[A-Za-z0-9+/=]+|MD5:[0-9a-f:]+)   # key
        |
        (\[preauth\])                                               # before authentication
        ",
    )
    .unwrap()
});

/// The OpenSSH server plugin
pub struct SshPlugin {
    metadata: PluginMetadata,
}

impl SshPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "OpenSSH server logs",
                "splash",
            ),
        }
    }
}

impl Default for SshPlugin {
    fn default() -> Self {
        SshPlugin::new()
    }
}

impl Plugin for SshPlugin {
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

/// The kind the outcome of a login attempt is colored in
fn outcome_kind(outcome: &str) -> TokenKind {
    match outcome {
        "Accepted" => TokenKind::Success,
        "Failed" => TokenKind::Failure,
        _ => TokenKind::Warning,
    }
}

/// Parses one sshd line, or returns `None` when the line was not logged by
/// sshd.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if !header.program.starts_with("sshd") {
        return None;
    }

    let mut tokens = header.tokens;

    push_message(&mut tokens, header.body);

    Some(ParsedLine::new(tokens))
}

/// Colors an sshd message.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let field = |index: usize| caps.get(index).map(|found| found.as_str());
        let whole = caps.get(0).unwrap();

        if let Some(outcome) = field(1) {
            // " for" stays in the text that follows, so the user after it is
            // read as the auth mode reads it.
            let end = caps.get(2).unwrap().end();

            auth::push_message(tokens, &message[cursor..whole.start()]);
            tokens.extend([
                Token::new(outcome, outcome_kind(outcome)),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(2).unwrap(), TokenKind::Protocol),
            ]);

            cursor = end;
            continue;
        }

        auth::push_message(tokens, &message[cursor..whole.start()]);

        if let Some(protocol) = field(3) {
            tokens.push(Token::new(protocol, TokenKind::Protocol));
        } else if let Some(key_type) = field(4) {
            tokens.extend([
                Token::new(key_type, TokenKind::Module),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(5).unwrap(), TokenKind::Transaction),
            ]);
        } else {
            tokens.push(Token::new(field(6).unwrap(), TokenKind::Tag));
        }

        cursor = whole.end();
    }

    auth::push_message(tokens, &message[cursor..]);
}
