//! sudo and su log parsing
//!
//! sudo logs each command through syslog as `   alice : TTY=pts/0 ;
//! PWD=/home/alice ; USER=root ; COMMAND=/usr/bin/apt update`, with the
//! user padded to eight characters, and writes the same entry to its own log
//! file behind the date, as `Oct  3 12:00:05 : alice : TTY=pts/0 ; ...`. su
//! logs `(to root) alice on pts/1` from util-linux, and
//! `Successful su for root by alice` or `+ /dev/pts/1 alice:root` from
//! shadow. Users, fields, and PAM lines are colored as in the auth mode.
use crate::auth;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "sudo";

/// The programs whose syslog lines this plugin reads
const PROGRAMS: [&str; 2] = ["sudo", "su"];

/// An entry in sudo's own log file, with the date and the user
static LOG_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([A-Z][a-z]{2} [ \d]\d \d{2}:\d{2}:\d{2}(?: \d{4})?)( : )([^\s:]+)( : )").unwrap()
});

/// A switch shadow's su logs as `+ /dev/pts/1 alice:root`, where `+` is a
/// success and `-` a failure
static SWITCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([+-])( )(\S+)( )([^\s:]+)(:)(\S+)$").unwrap());

/// The sudo and su plugin
pub struct SudoPlugin {
    metadata: PluginMetadata,
}

impl SudoPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "sudo and su privilege escalation logs",
                "splash",
            ),
        }
    }
}

impl Default for SudoPlugin {
    fn default() -> Self {
        SudoPlugin::new()
    }
}

impl Plugin for SudoPlugin {
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

/// Parses one sudo or su line, from syslog or from sudo's own log file, or
/// returns `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_syslog_line(line).or_else(|| parse_log_file_line(line))
}

/// Parses a line sudo or su sent to syslog.
pub fn parse_syslog_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if !PROGRAMS.contains(&header.program) {
        return None;
    }

    let mut tokens = header.tokens;

    if let Some(caps) = SWITCH.captures(header.body) {
        let field = |index: usize| caps.get(index).unwrap().as_str();
        let outcome = if field(1) == "+" {
            TokenKind::Success
        } else {
            TokenKind::Failure
        };

        tokens.extend([
            Token::new(field(1), outcome),
            Token::new(field(2), TokenKind::Plain),
            Token::new(field(3), TokenKind::Path),
            Token::new(field(4), TokenKind::Plain),
            Token::new(field(5), TokenKind::UserId),
            Token::new(field(6), TokenKind::Punctuation),
            Token::new(field(7), TokenKind::UserId),
        ]);
    } else {
        let body = if header.program == "sudo" {
            auth::split_sudo_user(&mut tokens, header.body)
        } else {
            header.body
        };

        auth::push_message(&mut tokens, body);
    }

    Some(ParsedLine::new(tokens))
}

/// Parses an entry in sudo's own log file.
pub fn parse_log_file_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LOG_FILE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(field(2), TokenKind::Punctuation),
        Token::new(field(3), TokenKind::UserId),
        Token::new(field(4), TokenKind::Punctuation),
    ];

    auth::push_message(&mut tokens, &line[caps.get(0).unwrap().end()..]);

    Some(ParsedLine::new(tokens))
}
