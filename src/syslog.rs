//! Syslog parsing
//!
//! Most of the logs splash reads arrive behind a syslog header: a timestamp,
//! a host, and the program that logged the line, as in
//! `Oct  3 12:00:01 mail postfix/smtpd[1234]: `. The header is read here for
//! every plugin, in the forms classic syslog daemons and `journalctl` write.
//!
//! The syslog plugin reads any program's lines in those forms, plus the
//! `<PRI>` prefix of a message read off the wire and the RFC 5424 format.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "syslog";

/// A date in the form `ctime` writes, such as `Tue Oct  3 12:00:01 2023`. The
/// spaces are escaped so the pattern reads the same with or without `(?x)`.
pub const CTIME: &str = r"[A-Z][a-z]{2}\ [A-Z][a-z]{2}\ [\ \d]\d\ \d{2}:\d{2}:\d{2}\ \d{4}";

/// The timestamps a syslog header opens with: the traditional
/// `Oct  3 12:00:01`, with microseconds under `journalctl -o short-precise`;
/// RFC 3339, with or without a colon in the offset; `journalctl -o
/// short-full`'s `Tue 2023-10-03 12:00:01 UTC`; the monotonic
/// `[ 1234.567890]`; and the Unix time `1696334401.123456`.
const TIMESTAMP: &str = r"(?x:
    [A-Z][a-z]{2}\s+\d{1,2}\ \d{2}:\d{2}:\d{2}(?:\.\d+)?
    |
    \d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[-+]\d{2}:?\d{2})
    |
    [A-Z][a-z]{2}\ \d{4}-\d{2}-\d{2}\ \d{2}:\d{2}:\d{2}\ \S+
    |
    \[\s*\d+\.\d+\]
    |
    \d+\.\d{6}
)";

/// A syslog header with the program that logged the line
static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?x)
        ^
        ({TIMESTAMP})
        \ (\S+)             # host
        \ ([^\s\[:]+)       # program
        (?:\[(\d+)\])?      # process id
        :\x20
        "
    ))
    .unwrap()
});

/// A syslog header with no program, as on `last message repeated 3 times`
static BARE_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^({TIMESTAMP}) (\S+) ")).unwrap());

/// The priority a message read off the wire opens with, such as `<34>`
static PRIORITY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^<(\d{1,3})>").unwrap());

/// The header of an RFC 5424 message, after its priority
static RFC_5424: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{1,2})           # version
        \ (\S+)             # timestamp
        \ (\S+)             # host
        \ (\S+)             # program
        \ (\S+)             # process id
        \ (\S+)             # message id
        \x20
        ",
    )
    .unwrap()
});

/// One element of RFC 5424 structured data, such as
/// `[exampleSDID@32473 iut="3" eventSource="Application"]`
static ELEMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\[([^\s\]=]+)((?:\ [^\s\]=]+="(?:[^"\\]|\\.)*")*)\]"#).unwrap()
});

/// One parameter of a structured data element
static PARAMETER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\ ([^\s\]=]+)="((?:[^"\\]|\\.)*)""#).unwrap());

/// Words that say a message reports a problem
static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("error", TokenKind::Failure),
        ("Error", TokenKind::Failure),
        ("ERROR", TokenKind::Failure),
        ("failed", TokenKind::Failure),
        ("Failed", TokenKind::Failure),
        ("failure", TokenKind::Failure),
        ("warning", TokenKind::Warning),
        ("Warning", TokenKind::Warning),
        ("WARNING", TokenKind::Warning),
    ])
});

/// The syslog header of a line, split into tokens
pub struct SyslogHeader<'a> {
    pub tokens: Vec<Token<'a>>,
    pub program: &'a str,
    pub body: &'a str,
}

/// Splits the syslog header off a line, or returns `None` when the line does
/// not start with one.
pub fn split_header(line: &str) -> Option<SyslogHeader<'_>> {
    let caps = HEADER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::Host),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::Tag),
    ];

    if let Some(pid) = caps.get(4) {
        tokens.extend([
            Token::new("[", TokenKind::Punctuation),
            Token::new(pid.as_str(), TokenKind::Pid),
            Token::new("]", TokenKind::Punctuation),
        ]);
    }

    tokens.extend([
        Token::new(":", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ]);

    Some(SyslogHeader {
        tokens,
        program: field(3),
        body: &line[caps.get(0).unwrap().end()..],
    })
}

/// Splits a timestamp and host off a line that names no program, returning
/// their tokens and the rest of the line.
pub fn split_bare_header(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let caps = BARE_HEADER.captures(line)?;

    let tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(caps.get(2).unwrap().as_str(), TokenKind::Host),
        Token::new(" ", TokenKind::Plain),
    ];

    Some((tokens, &line[caps.get(0).unwrap().end()..]))
}

/// The kind a syslog severity is colored in: `emerg` through `err` (0 to 3)
/// are failures, `warning` (4) is a warning, and the rest are levels.
pub fn severity_kind(severity: u32) -> TokenKind {
    match severity {
        0..=3 => TokenKind::Failure,
        4 => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// Splits a `<PRI>` prefix off a line, coloring the number by the severity it
/// holds, or returns `None` when the line has none.
pub fn split_priority(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let caps = PRIORITY.captures(line)?;
    let number = caps.get(1).unwrap().as_str();
    let severity = number.parse::<u32>().unwrap() % 8;

    let tokens = vec![
        Token::new("<", TokenKind::Punctuation),
        Token::new(number, severity_kind(severity)),
        Token::new(">", TokenKind::Punctuation),
    ];

    Some((tokens, &line[caps.get(0).unwrap().end()..]))
}

/// Colors a free text message: words that report a problem, and any IP
/// addresses.
pub fn push_text<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    mail::push_words(tokens, text, &WORDS);
}

/// The syslog plugin
pub struct SyslogPlugin {
    metadata: PluginMetadata,
}

impl SyslogPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Syslog lines from any program, including RFC 5424",
                "splash",
            ),
        }
    }
}

impl Default for SyslogPlugin {
    fn default() -> Self {
        SyslogPlugin::new()
    }
}

impl Plugin for SyslogPlugin {
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

/// Parses one syslog line, or returns `None` when the line has no syslog
/// header.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, rest) = split_priority(line).unwrap_or((Vec::new(), line));
    let rfc_5424 = (!tokens.is_empty())
        .then(|| RFC_5424.captures(rest))
        .flatten();

    if let Some(caps) = rfc_5424 {
        push_rfc_5424(&mut tokens, rest, &caps);
    } else if let Some(header) = split_header(rest) {
        tokens.extend(header.tokens);
        push_text(&mut tokens, header.body);
    } else {
        let (header, body) = split_bare_header(rest)?;
        tokens.extend(header);
        push_text(&mut tokens, body);
    }

    Some(ParsedLine::new(tokens))
}

/// Colors an RFC 5424 message after its priority: the header fields, any
/// structured data, and the message.
fn push_rfc_5424<'a>(tokens: &mut Vec<Token<'a>>, rest: &'a str, caps: &regex::Captures<'a>) {
    let kinds = [
        TokenKind::Number,
        TokenKind::Timestamp,
        TokenKind::Host,
        TokenKind::Tag,
        TokenKind::Pid,
        TokenKind::Transaction,
    ];

    for (index, kind) in kinds.into_iter().enumerate() {
        let text = caps.get(index + 1).unwrap().as_str();
        let kind = if text == "-" {
            TokenKind::Punctuation
        } else {
            kind
        };

        tokens.push(Token::new(text, kind));
        tokens.push(Token::new(" ", TokenKind::Plain));
    }

    let mut message = &rest[caps.get(0).unwrap().end()..];

    if let Some(after) = message.strip_prefix('-') {
        tokens.push(Token::new("-", TokenKind::Punctuation));
        message = after;
    } else {
        while let Some(element) = ELEMENT.captures(message) {
            push_element(tokens, &element);
            message = &message[element.get(0).unwrap().end()..];
        }
    }

    push_text(tokens, message);
}

/// Colors one structured data element: its id, and each parameter's name and
/// value.
fn push_element<'a>(tokens: &mut Vec<Token<'a>>, element: &regex::Captures<'a>) {
    tokens.extend([
        Token::new("[", TokenKind::Punctuation),
        Token::new(element.get(1).unwrap().as_str(), TokenKind::Module),
    ]);

    for parameter in PARAMETER.captures_iter(element.get(2).unwrap().as_str()) {
        let value = parameter.get(2).unwrap().as_str();

        tokens.extend([
            Token::new(" ", TokenKind::Plain),
            Token::new(parameter.get(1).unwrap().as_str(), TokenKind::Header),
            Token::new("=", TokenKind::Punctuation),
            Token::new("\"", TokenKind::Punctuation),
        ]);

        if !value.is_empty() {
            tokens.push(Token::new(value, TokenKind::Message));
        }

        tokens.push(Token::new("\"", TokenKind::Punctuation));
    }

    tokens.push(Token::new("]", TokenKind::Punctuation));
}
