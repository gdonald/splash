//! PostgreSQL log parsing
//!
//! PostgreSQL's server log opens each line with `log_line_prefix`, whose
//! default `%m [%p] ` writes the time with milliseconds and the process id,
//! and Debian's adds `%q%u@%d ` for the user and database. The severity
//! follows with two spaces after its colon, as in `ERROR:  relation "users"
//! does not exist`, and the `DETAIL`, `HINT`, `QUERY`, `CONTEXT`,
//! `LOCATION`, and `STATEMENT` lines that belong to a message take the same
//! shape. `log_min_duration_statement` logs slow statements as
//! `LOG:  duration: 2345.678 ms  statement: SELECT ...`.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "postgresql";

static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{4}-\d{2}-\d{2}\ \d{2}:\d{2}:\d{2}(?:\.\d+)?(?:\ [\w+-]+)?)   # time
        \ \[(\d+)\]\                                # process id
        (?:([^\s@]*)@(\S*)\ )?                      # user and database
        (DEBUG[1-5]|INFO|NOTICE|WARNING|ERROR|LOG|FATAL|PANIC
            |DETAIL|HINT|QUERY|CONTEXT|LOCATION|STATEMENT)   # severity
        (:\ \ )
        (?:([0-9A-Z]{5})(:\ ))?                     # SQLSTATE
        (.*)
        $
        ",
    )
    .unwrap()
});

/// A slow statement logged by `log_min_duration_statement`
static DURATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(duration)(: )(\d+(?:\.\d+)? ms)(?:(  )(statement|(?:execute|parse|bind) [^:]*)(: )(.*))?$")
        .unwrap()
});

/// A quoted user or database name inside a message
static NAMED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\b(user|database|role)( ")([^"]*)(")"#).unwrap());

/// The PostgreSQL log plugin
pub struct PostgresqlPlugin {
    metadata: PluginMetadata,
}

impl PostgresqlPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "PostgreSQL server logs and slow statements",
                "splash",
            ),
        }
    }
}

impl Default for PostgresqlPlugin {
    fn default() -> Self {
        PostgresqlPlugin::new()
    }
}

impl Plugin for PostgresqlPlugin {
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

/// The kind a severity is colored in. The parts that belong to a message,
/// such as `DETAIL`, are colored as headers.
fn severity_kind(severity: &str) -> TokenKind {
    match severity {
        "ERROR" | "FATAL" | "PANIC" => TokenKind::Failure,
        "WARNING" => TokenKind::Warning,
        "DETAIL" | "HINT" | "QUERY" | "CONTEXT" | "LOCATION" | "STATEMENT" => TokenKind::Header,
        _ => TokenKind::Level,
    }
}

/// Parses one PostgreSQL log line, or returns `None` when the line does not
/// open with the default prefix and a severity.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LINE.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());
    let severity = field(5).unwrap();

    let mut tokens = vec![
        Token::new(field(1).unwrap(), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(2).unwrap(), TokenKind::Pid),
        Token::new("]", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ];

    if let Some(user) = field(3) {
        if !user.is_empty() {
            tokens.push(Token::new(user, TokenKind::UserId));
        }

        tokens.push(Token::new("@", TokenKind::Punctuation));

        if !field(4).unwrap().is_empty() {
            tokens.push(Token::new(field(4).unwrap(), TokenKind::Module));
        }

        tokens.push(Token::new(" ", TokenKind::Plain));
    }

    tokens.extend([
        Token::new(severity, severity_kind(severity)),
        Token::new(field(6).unwrap(), TokenKind::Punctuation),
    ]);

    if let Some(state) = field(7) {
        tokens.extend([
            Token::new(state, TokenKind::Status),
            Token::new(field(8).unwrap(), TokenKind::Punctuation),
        ]);
    }

    let message = field(9).unwrap();

    match severity {
        "STATEMENT" | "QUERY" if !message.is_empty() => {
            tokens.push(Token::new(message, TokenKind::Request))
        }
        _ => push_message(&mut tokens, message),
    }

    Some(ParsedLine::new(tokens))
}

/// Colors a message: a slow statement's duration and text, and the quoted
/// users and databases a message names.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    if let Some(caps) = DURATION.captures(message) {
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        tokens.extend([
            Token::new(field(1).unwrap(), TokenKind::Header),
            Token::new(field(2).unwrap(), TokenKind::Punctuation),
            Token::new(field(3).unwrap(), TokenKind::Duration),
        ]);

        if let Some(gap) = field(4) {
            tokens.extend([
                Token::new(gap, TokenKind::Plain),
                Token::new(field(5).unwrap(), TokenKind::Header),
                Token::new(field(6).unwrap(), TokenKind::Punctuation),
            ]);

            if !field(7).unwrap().is_empty() {
                tokens.push(Token::new(field(7).unwrap(), TokenKind::Request));
            }
        }

        return;
    }

    let mut cursor = 0;

    for caps in NAMED.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).unwrap().as_str();

        syslog::push_text(tokens, &message[cursor..whole.start()]);

        let kind = if field(1) == "database" {
            TokenKind::Module
        } else {
            TokenKind::UserId
        };

        tokens.extend([
            Token::new(field(1), TokenKind::Message),
            Token::new(field(2), TokenKind::Punctuation),
        ]);

        if !field(3).is_empty() {
            tokens.push(Token::new(field(3), kind));
        }

        tokens.push(Token::new(field(4), TokenKind::Punctuation));

        cursor = whole.end();
    }

    syslog::push_text(tokens, &message[cursor..]);
}
