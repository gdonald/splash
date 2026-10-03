//! nginx error log parsing
//!
//! nginx opens each error log line with the date, the level, and the process
//! and thread ids, followed by the connection number when the error belongs
//! to a request, as in `2023/10/03 12:00:01 [error] 1234#5678: *42 `. The
//! message often names a system call and the file it failed on, as
//! `open() "/usr/share/nginx/html/favicon.ico" failed (2: No such file or
//! directory)`, and ends with the request's context: `, client: 10.0.0.5,
//! server: example.com, request: "GET /favicon.ico HTTP/1.1", host:
//! "example.com"`.
use crate::ftp;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "nginx-error";

static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2})( )(\[)([a-z]+)(\])( )(\d+)(#)(\d+)(: )(?:(\*)(\d+)( ))?").unwrap()
});

/// Where the request's context starts
static CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r", (?:client|server|request|upstream|host|referrer|subrequest|login): ").unwrap()
});

/// One `name: value` pair of the request's context
static PAIR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(, )?([a-z]+)(: )("[^"]*"|[^,]*)"#).unwrap());

/// The parts of a message that carry a value: a system call, a quoted file
/// or value, and an error number with its text
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        \b(\w+\(\))                    # system call
        |
        (")([^"]*)(")                  # quoted file or value
        |
        (\()(\d+)(:\ )([^)]*)(\))      # error number and text
        "#,
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("failed", TokenKind::Failure),
        ("upstream timed out", TokenKind::Failure),
        ("no live upstreams", TokenKind::Failure),
        ("is forbidden", TokenKind::Failure),
        ("limiting requests", TokenKind::Warning),
        ("limiting connections", TokenKind::Warning),
        (
            "an upstream response is buffered to a temporary file",
            TokenKind::Warning,
        ),
    ])
});

/// The nginx error log plugin
pub struct NginxErrorPlugin {
    metadata: PluginMetadata,
}

impl NginxErrorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "nginx error logs",
                "splash",
            ),
        }
    }
}

impl Default for NginxErrorPlugin {
    fn default() -> Self {
        NginxErrorPlugin::new()
    }
}

impl Plugin for NginxErrorPlugin {
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

/// The kind a level is colored in
fn level_kind(level: &str) -> TokenKind {
    match level {
        "emerg" | "alert" | "crit" | "error" => TokenKind::Failure,
        "warn" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// The kind a context value is colored in, by its name
fn context_kind(name: &str, value: &str) -> TokenKind {
    match name {
        "client" => ftp::host_kind(value),
        "server" => TokenKind::VirtualHost,
        "request" | "subrequest" | "upstream" => TokenKind::Request,
        "host" => TokenKind::Host,
        "referrer" => TokenKind::Referer,
        _ => TokenKind::Message,
    }
}

/// Parses one nginx error log line, or returns `None` when the line is not
/// one.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LINE.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());
    let level = field(4).unwrap();

    let mut tokens = vec![
        Token::new(field(1).unwrap(), TokenKind::Timestamp),
        Token::new(field(2).unwrap(), TokenKind::Plain),
        Token::new(field(3).unwrap(), TokenKind::Punctuation),
        Token::new(level, level_kind(level)),
        Token::new(field(5).unwrap(), TokenKind::Punctuation),
        Token::new(field(6).unwrap(), TokenKind::Plain),
        Token::new(field(7).unwrap(), TokenKind::Pid),
        Token::new(field(8).unwrap(), TokenKind::Punctuation),
        Token::new(field(9).unwrap(), TokenKind::Pid),
        Token::new(field(10).unwrap(), TokenKind::Punctuation),
    ];

    if let Some(star) = field(11) {
        tokens.extend([
            Token::new(star, TokenKind::Punctuation),
            Token::new(field(12).unwrap(), TokenKind::Transaction),
            Token::new(field(13).unwrap(), TokenKind::Plain),
        ]);
    }

    let rest = &line[caps.get(0).unwrap().end()..];
    let context_start = CONTEXT.find(rest).map_or(rest.len(), |found| found.start());

    push_message(&mut tokens, &rest[..context_start]);
    push_context(&mut tokens, &rest[context_start..]);

    Some(ParsedLine::new(tokens))
}

/// Colors the message before the request's context.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(call) = field(1) {
            tokens.push(Token::new(call, TokenKind::Module));
        } else if let Some(quoted) = field(3) {
            let kind = if quoted.starts_with('/') {
                TokenKind::Path
            } else {
                TokenKind::Message
            };

            tokens.push(Token::new(field(2).unwrap(), TokenKind::Punctuation));

            if !quoted.is_empty() {
                tokens.push(Token::new(quoted, kind));
            }

            tokens.push(Token::new(field(4).unwrap(), TokenKind::Punctuation));
        } else {
            tokens.extend([
                Token::new(field(5).unwrap(), TokenKind::Punctuation),
                Token::new(field(6).unwrap(), TokenKind::Number),
                Token::new(field(7).unwrap(), TokenKind::Punctuation),
                Token::new(field(8).unwrap(), TokenKind::Failure),
                Token::new(field(9).unwrap(), TokenKind::Punctuation),
            ]);
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}

/// Colors the request's context, each value by its name.
fn push_context<'a>(tokens: &mut Vec<Token<'a>>, context: &'a str) {
    let mut cursor = 0;

    for caps in PAIR.captures_iter(context) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());
        let name = field(2).unwrap();
        let value = field(4).unwrap();

        mail::push_words(tokens, &context[cursor..whole.start()], &WORDS);

        if let Some(separator) = field(1) {
            tokens.push(Token::new(separator, TokenKind::Punctuation));
        }

        tokens.extend([
            Token::new(name, TokenKind::Header),
            Token::new(field(3).unwrap(), TokenKind::Punctuation),
        ]);

        let inner = value
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'));

        match inner {
            Some(inner) => {
                tokens.push(Token::new("\"", TokenKind::Punctuation));

                if !inner.is_empty() {
                    tokens.push(Token::new(inner, context_kind(name, inner)));
                }

                tokens.push(Token::new("\"", TokenKind::Punctuation));
            }
            None if value.is_empty() => {}
            None => tokens.push(Token::new(value, context_kind(name, value))),
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &context[cursor..], &WORDS);
}
