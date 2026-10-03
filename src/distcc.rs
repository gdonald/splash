//! distcc parsing
//!
//! distccd writes each message to its log file as `distccd[pid] (function)
//! message`, and through syslog as `(function) message` under the
//! `distccd` program. An error or warning opens its message with `ERROR: `
//! or `Warning: `. When a job finishes, distccd logs a summary such as
//! `client: 10.0.0.5:52144 COMPILE_OK exit:0 sig:0 core:0 ret:0
//! time:1234ms gcc main.c`.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "distcc";

/// The program and process id that open a log file line
static LOG_FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(distccd)(\[)(\d+)(\])( )").unwrap());

/// The function a message was logged from
static FUNCTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\()([^)]+)(\))( )").unwrap());

/// The parts of a job summary that carry a value: a client address and
/// port, and a `name:value` field such as `exit:0` or `time:1234ms`
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (\d{1,3}(?:\.\d{1,3}){3})(:)(\d+)          # address and port
        |
        \b(exit|sig|core|ret|time)(:)(\d+)(ms)?\b   # field
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("COMPILE_OK", TokenKind::Success),
        ("COMPILE_ERROR", TokenKind::Failure),
        ("COMPILE_TIMEOUT", TokenKind::Failure),
        ("CLI_DISCONN", TokenKind::Warning),
        ("REJ_OVERLOAD", TokenKind::Warning),
        ("REJ_BAD_REQ", TokenKind::Failure),
        ("ERROR", TokenKind::Failure),
        ("Warning", TokenKind::Warning),
        ("CRITICAL", TokenKind::Failure),
        ("ALERT", TokenKind::Failure),
        ("EMERGENCY", TokenKind::Failure),
    ])
});

/// The distcc plugin
pub struct DistccPlugin {
    metadata: PluginMetadata,
}

impl DistccPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "distccd distributed compilation logs",
                "splash",
            ),
        }
    }
}

impl Default for DistccPlugin {
    fn default() -> Self {
        DistccPlugin::new()
    }
}

impl Plugin for DistccPlugin {
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

/// Parses one distccd line, from its log file or from syslog, or returns
/// `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = split_log_file_prefix(line).or_else(|| split_syslog_header(line))?;

    push_body(&mut tokens, body);

    Some(ParsedLine::new(tokens))
}

/// Splits the program and process id off a line from distccd's log file.
fn split_log_file_prefix(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let caps = LOG_FILE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let tokens = vec![
        Token::new(field(1), TokenKind::Tag),
        Token::new(field(2), TokenKind::Punctuation),
        Token::new(field(3), TokenKind::Pid),
        Token::new(field(4), TokenKind::Punctuation),
        Token::new(field(5), TokenKind::Plain),
    ];

    Some((tokens, &line[caps.get(0).unwrap().end()..]))
}

/// Splits the syslog header off a line distccd sent to syslog.
fn split_syslog_header(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let header = syslog::split_header(line)?;

    (header.program == "distccd").then_some((header.tokens, header.body))
}

/// Colors the function a message was logged from, then the message.
fn push_body<'a>(tokens: &mut Vec<Token<'a>>, body: &'a str) {
    let mut message = body;

    if let Some(caps) = FUNCTION.captures(body) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(field(1), TokenKind::Punctuation),
            Token::new(field(2), TokenKind::Module),
            Token::new(field(3), TokenKind::Punctuation),
            Token::new(field(4), TokenKind::Plain),
        ]);

        message = &body[caps.get(0).unwrap().end()..];
    }

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
        } else {
            let name = field(4).unwrap();
            let kind = if name == "time" {
                TokenKind::Duration
            } else {
                TokenKind::Number
            };

            tokens.extend([
                Token::new(name, TokenKind::Header),
                Token::new(field(5).unwrap(), TokenKind::Punctuation),
                Token::new(field(6).unwrap(), kind),
            ]);

            if let Some(unit) = field(7) {
                tokens.push(Token::new(unit, kind));
            }
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
