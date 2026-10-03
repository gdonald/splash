//! super log parsing
//!
//! super(1) writes each command it runs to its log file as
//! `user@host date<TAB>command (arguments)`, where the date is the one
//! `ctime` writes, and to syslog as `command (arguments)` under the `super`
//! program. A `logfile` set up with a program name puts it and a colon
//! before the user.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "super";

/// The program name, user, host, and date that open a log file line
static LOG_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:(\S+?)(:?) )?([^\s@]+)(@)(\S+) ({})(\t)",
        syslog::CTIME
    ))
    .unwrap()
});

/// A command and its arguments in parentheses
static COMMAND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\S+)( )(\()(.*)(\))$").unwrap());

/// The super plugin
pub struct SuperPlugin {
    metadata: PluginMetadata,
}

impl SuperPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "super(1) superuser access logs",
                "splash",
            ),
        }
    }
}

impl Default for SuperPlugin {
    fn default() -> Self {
        SuperPlugin::new()
    }
}

impl Plugin for SuperPlugin {
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

/// Parses one super log line, from its log file or from syslog, or returns
/// `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_log_file_line(line).or_else(|| parse_syslog_line(line))
}

/// Parses a line from super's log file.
pub fn parse_log_file_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LOG_FILE.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());

    let mut tokens = Vec::new();

    if let Some(program) = field(1) {
        tokens.push(Token::new(program, TokenKind::Tag));

        if !field(2).unwrap().is_empty() {
            tokens.push(Token::new(field(2).unwrap(), TokenKind::Punctuation));
        }

        tokens.push(Token::new(" ", TokenKind::Plain));
    }

    tokens.extend([
        Token::new(field(3).unwrap(), TokenKind::UserId),
        Token::new(field(4).unwrap(), TokenKind::Punctuation),
        Token::new(field(5).unwrap(), TokenKind::Host),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(6).unwrap(), TokenKind::Timestamp),
        Token::new(field(7).unwrap(), TokenKind::Plain),
    ]);

    push_message(&mut tokens, &line[caps.get(0).unwrap().end()..]);

    Some(ParsedLine::new(tokens))
}

/// Parses a line super sent to syslog.
pub fn parse_syslog_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if header.program != NAME {
        return None;
    }

    let mut tokens = header.tokens;

    push_message(&mut tokens, header.body);

    Some(ParsedLine::new(tokens))
}

/// Colors a command and its arguments, or colors anything else, such as an
/// error super reports, as message text.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let Some(caps) = COMMAND.captures(message) else {
        syslog::push_text(tokens, message);
        return;
    };

    let field = |index: usize| caps.get(index).unwrap().as_str();

    tokens.extend([
        Token::new(field(1), TokenKind::Method),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), TokenKind::Punctuation),
    ]);

    if !field(4).is_empty() {
        tokens.push(Token::new(field(4), TokenKind::Request));
    }

    tokens.push(Token::new(field(5), TokenKind::Punctuation));
}
