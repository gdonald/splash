//! Git log parsing
//!
//! `git daemon` logs each connection and request, as `Connection from
//! 10.0.0.5:52144` and `Request upload-pack for '/srv/git/app.git'`,
//! through syslog under `git-daemon`, or to standard error with the process
//! id in brackets before each message. Git's own trace output,
//! `GIT_TRACE=1`, opens each line with the time and the file and line that
//! wrote it, padded to forty characters, as
//! `12:00:01.123456 git.c:463               trace: built-in: git fetch`.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "git";

/// The process id `git daemon` writes before a message on standard error
static DAEMON_PID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\[)(\d+)(\])( )").unwrap());

/// A trace line's time, and the file and line that wrote it
static TRACE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{2}:\d{2}:\d{2}\.\d{6})( )([\w./-]+\.[ch])(:)(\d+)(\s+)").unwrap()
});

/// What a trace message says Git ran, as in `trace: built-in: git fetch`
static TRACED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(trace)(: )([\w -]+?)(: )(.*)$").unwrap());

/// The parts of a daemon message that carry a value: a client address and
/// port, a service and repository, and a quoted repository or value
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        (\d{1,3}(?:\.\d{1,3}){3}|\[[0-9a-fA-F:]+\])(:)(\d+)   # address and port
        |
        \b(Request)\ (\S+)\ (for)\b                          # service
        |
        (')([^']*)(')                                        # repository
        |
        (")([^"]*)(")                                        # quoted value
        "#,
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Ready to rumble", TokenKind::Success),
        ("Disconnected", TokenKind::Warning),
        ("not in directory list", TokenKind::Failure),
        ("does not appear to be a git repository", TokenKind::Failure),
        ("service not enabled", TokenKind::Failure),
        ("repository not exported", TokenKind::Failure),
        ("Protocol error", TokenKind::Failure),
        ("Non-absolute path denied", TokenKind::Failure),
        ("User-path not allowed", TokenKind::Failure),
        ("unable to fork", TokenKind::Failure),
        ("with error", TokenKind::Failure),
        ("fatal", TokenKind::Failure),
        ("error", TokenKind::Failure),
    ])
});

/// The Git plugin
pub struct GitPlugin {
    metadata: PluginMetadata,
}

impl GitPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "git daemon and Git trace logs",
                "splash",
            ),
        }
    }
}

impl Default for GitPlugin {
    fn default() -> Self {
        GitPlugin::new()
    }
}

impl Plugin for GitPlugin {
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

/// Parses one Git log line, from `git daemon` or from trace output, or
/// returns `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_daemon_line(line).or_else(|| parse_trace_line(line))
}

/// Parses a `git daemon` line, from syslog or from standard error.
pub fn parse_daemon_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = match syslog::split_header(line) {
        Some(header) if header.program == "git-daemon" => (header.tokens, header.body),
        Some(_) => return None,
        None => (Vec::new(), line),
    };

    let mut message = body;

    match DAEMON_PID.captures(body) {
        Some(caps) => {
            let field = |index: usize| caps.get(index).unwrap().as_str();

            tokens.extend([
                Token::new(field(1), TokenKind::Punctuation),
                Token::new(field(2), TokenKind::Pid),
                Token::new(field(3), TokenKind::Punctuation),
                Token::new(field(4), TokenKind::Plain),
            ]);

            message = &body[caps.get(0).unwrap().end()..];
        }
        None if tokens.is_empty() => return None,
        None => {}
    }

    push_message(&mut tokens, message);

    Some(ParsedLine::new(tokens))
}

/// Parses a line of Git's trace output.
pub fn parse_trace_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = TRACE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), TokenKind::Path),
        Token::new(field(4), TokenKind::Punctuation),
        Token::new(field(5), TokenKind::Number),
        Token::new(field(6), TokenKind::Plain),
    ];

    let message = &line[caps.get(0).unwrap().end()..];

    match TRACED.captures(message) {
        Some(traced) => {
            let part = |index: usize| traced.get(index).unwrap().as_str();

            tokens.extend([
                Token::new(part(1), TokenKind::Tag),
                Token::new(part(2), TokenKind::Punctuation),
                Token::new(part(3), TokenKind::Module),
                Token::new(part(4), TokenKind::Punctuation),
            ]);

            if !part(5).is_empty() {
                tokens.push(Token::new(part(5), TokenKind::Request));
            }
        }
        None => mail::push_words(&mut tokens, message, &WORDS),
    }

    Some(ParsedLine::new(tokens))
}

/// Colors a daemon message.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(address) = field(1) {
            tokens.extend([
                Token::new(address, TokenKind::Ip),
                Token::new(field(2).unwrap(), TokenKind::Punctuation),
                Token::new(field(3).unwrap(), TokenKind::Number),
            ]);
        } else if let Some(request) = field(4) {
            tokens.extend([
                Token::new(request, TokenKind::Message),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(5).unwrap(), TokenKind::Method),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(6).unwrap(), TokenKind::Message),
            ]);
        } else {
            let (open, inner, close, kind) = match field(7) {
                Some(open) => (open, field(8).unwrap(), field(9).unwrap(), TokenKind::Path),
                None => (
                    field(10).unwrap(),
                    field(11).unwrap(),
                    field(12).unwrap(),
                    TokenKind::Message,
                ),
            };

            tokens.push(Token::new(open, TokenKind::Punctuation));

            if !inner.is_empty() {
                tokens.push(Token::new(inner, kind));
            }

            tokens.push(Token::new(close, TokenKind::Punctuation));
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
