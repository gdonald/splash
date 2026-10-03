//! cron parsing
//!
//! cron logs each event as `(user) EVENT (detail)`, such as
//! `(root) CMD (run-parts /etc/cron.hourly)`, through syslog under `CRON`,
//! `cron`, `crond`, or `CROND`. cronie can write the same events to its own
//! log file as `user (10/03-12:00:01-1234) EVENT (detail)`. anacron logs
//! the jobs it runs by name, as in ``Job `cron.daily' started``, and the PAM
//! session lines cron writes are colored as in the auth mode.
use crate::auth;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "cron";

/// The programs whose syslog lines this plugin reads
const PROGRAMS: [&str; 5] = ["CRON", "cron", "crond", "CROND", "anacron"];

/// An event logged through syslog
static EVENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\(([^)]*)\) ([A-Z][A-Z ]*?) \((.*)\)(.*)$").unwrap());

/// An event in cronie's own log file, with the date and process id
static FILE_EVENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\S+)                                       # user
        \ \((\d{2}/\d{2}-\d{2}:\d{2}:\d{2})-(\d+)\)  # date and process id
        \ ([A-Z][A-Z\ ]*?)                          # event
        \ \((.*)\)                                  # detail
        (.*)
        $
        ",
    )
    .unwrap()
});

/// A job anacron names in backquote and quote
static JOB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^']*)'").unwrap());

/// The cron plugin
pub struct CronPlugin {
    metadata: PluginMetadata,
}

impl CronPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "cron and anacron job logs",
                "splash",
            ),
        }
    }
}

impl Default for CronPlugin {
    fn default() -> Self {
        CronPlugin::new()
    }
}

impl Plugin for CronPlugin {
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

/// Parses one cron log line, from syslog or from cronie's log file, or
/// returns `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_syslog_line(line).or_else(|| parse_file_line(line))
}

/// Parses a line cron or anacron sent to syslog.
pub fn parse_syslog_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if !PROGRAMS.contains(&header.program) {
        return None;
    }

    let mut tokens = header.tokens;

    match EVENT.captures(header.body) {
        Some(caps) => {
            let field = |index: usize| caps.get(index).unwrap().as_str();

            tokens.push(Token::new("(", TokenKind::Punctuation));
            push_user(&mut tokens, field(1));
            tokens.extend([
                Token::new(")", TokenKind::Punctuation),
                Token::new(" ", TokenKind::Plain),
            ]);
            push_event(&mut tokens, field(2), field(3), field(4));
        }
        None => push_jobs(&mut tokens, header.body),
    }

    Some(ParsedLine::new(tokens))
}

/// Parses a line from cronie's own log file.
pub fn parse_file_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = FILE_EVENT.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::UserId),
        Token::new(" ", TokenKind::Plain),
        Token::new("(", TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Timestamp),
        Token::new("-", TokenKind::Punctuation),
        Token::new(field(3), TokenKind::Pid),
        Token::new(")", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ];

    push_event(&mut tokens, field(4), field(5), field(6));

    Some(ParsedLine::new(tokens))
}

fn push_user<'a>(tokens: &mut Vec<Token<'a>>, user: &'a str) {
    if !user.is_empty() {
        tokens.push(Token::new(user, TokenKind::UserId));
    }
}

/// The kind an event is colored in: an error is a failure, and any other
/// event is what cron did
fn event_kind(event: &str) -> TokenKind {
    if event.contains("ERROR") {
        TokenKind::Failure
    } else {
        TokenKind::Method
    }
}

/// Colors an event, its detail in parentheses, and any error after it. The
/// detail of a command event is the command.
fn push_event<'a>(tokens: &mut Vec<Token<'a>>, event: &'a str, detail: &'a str, after: &'a str) {
    tokens.extend([
        Token::new(event, event_kind(event)),
        Token::new(" ", TokenKind::Plain),
        Token::new("(", TokenKind::Punctuation),
    ]);

    if !detail.is_empty() {
        if event.starts_with("CMD") {
            tokens.push(Token::new(detail, TokenKind::Request));
        } else {
            syslog::push_text(tokens, detail);
        }
    }

    tokens.push(Token::new(")", TokenKind::Punctuation));
    syslog::push_text(tokens, after);
}

/// Colors a message that is not an event: anacron's job names, with the rest
/// colored as in the auth mode.
fn push_jobs<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in JOB.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let job = caps.get(1).unwrap().as_str();

        auth::push_message(tokens, &message[cursor..whole.start()]);
        tokens.push(Token::new("`", TokenKind::Punctuation));

        if !job.is_empty() {
            tokens.push(Token::new(job, TokenKind::Module));
        }

        tokens.push(Token::new("'", TokenKind::Punctuation));
        cursor = whole.end();
    }

    auth::push_message(tokens, &message[cursor..]);
}
