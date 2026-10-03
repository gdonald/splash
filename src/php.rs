//! PHP log parsing
//!
//! PHP's `error_log` file holds one line per error, as
//! `[03-Oct-2023 12:00:01 UTC] PHP Warning:  message in /var/www/index.php
//! on line 12`. An uncaught exception continues on the lines after it with
//! its stack trace, `#0 /var/www/index.php(20): run()`, and closes with
//! `  thrown in ... on line 12`. PHP-FPM's own log opens each line with the
//! date and a level, as `[03-Oct-2023 12:00:01] WARNING: `.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "php";

/// The bracketed date an error log or PHP-FPM line opens with. The error log
/// adds the time zone, and PHP-FPM adds microseconds at its debug level.
static DATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\[)(\d{2}-[A-Z][a-z]{2}-\d{4} \d{2}:\d{2}:\d{2}(?:\.\d+)?(?: [\w/+-]+)?)(\]) ")
        .unwrap()
});

/// The level PHP-FPM writes after the date
static FPM_LEVEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(DEBUG|NOTICE|WARNING|ERROR|ALERT)(: )").unwrap());

/// A stack trace line, such as `#0 /var/www/index.php(20): run()` or
/// `#1 {main}`
static FRAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#)(\d+)( )(.*)$").unwrap());

/// The lines that open and close a stack trace
static TRACE_EDGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:PHP )?Stack trace:$|^\s+thrown in ").unwrap());

/// The parts of a PHP message that carry a value: an error type, a file and
/// line, a PHP-FPM pool, and a child process
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        \b(PHP)\ (Fatal\ error|Recoverable\ fatal\ error|Parse\ error|Warning|Notice
            |Deprecated|Unknown\ error):                                   # error type
        |
        \b(in)\ (/\S+)\ (on\ line)\ (\d+)                                  # file and line
        |
        (/[^\s:()]+)(?:(:)(\d+)|(\()(\d+)(\)))                             # file:line or file(line)
        |
        (\[)(pool)\ ([^\]]+)(\])                                           # pool
        |
        \b(child)\ (\d+)                                                   # child process
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Uncaught", TokenKind::Failure),
        ("thrown", TokenKind::Failure),
        ("exited on signal", TokenKind::Failure),
        ("exited with code", TokenKind::Warning),
        ("server reached pm.max_children setting", TokenKind::Warning),
        ("seems busy", TokenKind::Warning),
        ("ready to handle connections", TokenKind::Success),
        ("fpm is running", TokenKind::Success),
        ("started", TokenKind::Success),
    ])
});

/// The PHP log plugin
pub struct PhpPlugin {
    metadata: PluginMetadata,
}

impl PhpPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "PHP error logs and PHP-FPM logs",
                "splash",
            ),
        }
    }
}

impl Default for PhpPlugin {
    fn default() -> Self {
        PhpPlugin::new()
    }
}

impl Plugin for PhpPlugin {
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

/// Parses one PHP log line, or returns `None` when the line is neither a
/// dated line nor part of a stack trace.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_dated_line(line).or_else(|| parse_trace_line(line))
}

/// Parses a line that opens with a bracketed date.
pub fn parse_dated_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = DATE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Timestamp),
        Token::new(field(3), TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ];

    let mut message = &line[caps.get(0).unwrap().end()..];

    if let Some(level) = FPM_LEVEL.captures(message) {
        let name = level.get(1).unwrap().as_str();

        tokens.extend([
            Token::new(name, fpm_level_kind(name)),
            Token::new(level.get(2).unwrap().as_str(), TokenKind::Punctuation),
        ]);

        message = &message[level.get(0).unwrap().end()..];
    }

    push_message(&mut tokens, message);

    Some(ParsedLine::new(tokens))
}

/// Parses a line of the stack trace that follows an uncaught exception.
pub fn parse_trace_line(line: &str) -> Option<ParsedLine<'_>> {
    let mut tokens = Vec::new();

    if let Some(caps) = FRAME.captures(line) {
        tokens.extend([
            Token::new(caps.get(1).unwrap().as_str(), TokenKind::Punctuation),
            Token::new(caps.get(2).unwrap().as_str(), TokenKind::Number),
            Token::new(caps.get(3).unwrap().as_str(), TokenKind::Plain),
        ]);
        push_message(&mut tokens, caps.get(4).unwrap().as_str());
    } else if TRACE_EDGE.is_match(line) {
        push_message(&mut tokens, line);
    } else {
        return None;
    }

    Some(ParsedLine::new(tokens))
}

/// The kind a PHP-FPM level is colored in
fn fpm_level_kind(level: &str) -> TokenKind {
    match level {
        "ERROR" | "ALERT" => TokenKind::Failure,
        "WARNING" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// The kind a PHP error type is colored in
pub fn error_kind(error: &str) -> TokenKind {
    match error {
        "Warning" => TokenKind::Warning,
        "Notice" | "Deprecated" => TokenKind::Level,
        _ => TokenKind::Failure,
    }
}

/// Colors a PHP message: its error type, the files and lines it names, and a
/// PHP-FPM pool and child process. Apache's error log carries PHP messages
/// too, and the apache-error mode colors them with this.
pub fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(error) = field(2) {
            tokens.extend([
                Token::new(field(1).unwrap(), TokenKind::Tag),
                Token::new(" ", TokenKind::Plain),
                Token::new(error, error_kind(error)),
                Token::new(":", TokenKind::Punctuation),
            ]);
        } else if let Some(path) = field(4) {
            tokens.extend([
                Token::new(field(3).unwrap(), TokenKind::Message),
                Token::new(" ", TokenKind::Plain),
                Token::new(path, TokenKind::Path),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(5).unwrap(), TokenKind::Message),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(6).unwrap(), TokenKind::Number),
            ]);
        } else if let Some(path) = field(7) {
            tokens.push(Token::new(path, TokenKind::Path));

            match field(8) {
                Some(colon) => tokens.extend([
                    Token::new(colon, TokenKind::Punctuation),
                    Token::new(field(9).unwrap(), TokenKind::Number),
                ]),
                None => tokens.extend([
                    Token::new(field(10).unwrap(), TokenKind::Punctuation),
                    Token::new(field(11).unwrap(), TokenKind::Number),
                    Token::new(field(12).unwrap(), TokenKind::Punctuation),
                ]),
            }
        } else if let Some(pool) = field(15) {
            tokens.extend([
                Token::new(field(13).unwrap(), TokenKind::Punctuation),
                Token::new(field(14).unwrap(), TokenKind::Header),
                Token::new(" ", TokenKind::Plain),
                Token::new(pool, TokenKind::Module),
                Token::new(field(16).unwrap(), TokenKind::Punctuation),
            ]);
        } else {
            tokens.extend([
                Token::new(field(17).unwrap(), TokenKind::Message),
                Token::new(" ", TokenKind::Plain),
                Token::new(field(18).unwrap(), TokenKind::Pid),
            ]);
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
