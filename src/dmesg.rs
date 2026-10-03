//! dmesg parsing
//!
//! `dmesg` prints the kernel ring buffer one message per line. A line can
//! open with the raw priority `-r` prints, such as `<6>`, the facility and
//! level `-x` prints, such as `kern  :info  : `, and a timestamp: the seconds
//! since boot, `[    1.234567]`, the date `-T` prints, or the ISO date
//! `--time-format iso` prints. Kernel messages forwarded to syslog under the
//! `kernel` program are read too.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "dmesg";

/// The facility and level `dmesg -x` prints, each padded to six characters
static DECODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([a-z0-9]+)(\s*)(:)([a-z]+)(\s*)(:)( )").unwrap());

/// The timestamp in any of the forms `dmesg` prints
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?x)
        ^
        (
            \[\s*\d+\.\d+\]
            |
            \[{}\]
            |
            \d{{4}}-\d{{2}}-\d{{2}}T\d{{2}}:\d{{2}}:\d{{2}},\d+[-+]\d{{4}}
        )
        (\ ?)
        ",
        syslog::CTIME
    ))
    .unwrap()
});

/// The subsystem or device a message opens with, such as `usb 1-1: ` or
/// `EXT4-fs (sda1): `
static SUBSYSTEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^((?:\S+ ){0,2}\S+?): ").unwrap());

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Link is Up", TokenKind::Success),
        ("Link is Down", TokenKind::Warning),
        ("warning", TokenKind::Warning),
        ("Warning", TokenKind::Warning),
        ("WARNING", TokenKind::Warning),
        ("error", TokenKind::Failure),
        ("Error", TokenKind::Failure),
        ("I/O error", TokenKind::Failure),
        ("failed", TokenKind::Failure),
        ("Failed", TokenKind::Failure),
        ("segfault", TokenKind::Failure),
        ("Out of memory", TokenKind::Failure),
        ("Call Trace", TokenKind::Failure),
    ])
});

/// The dmesg plugin
pub struct DmesgPlugin {
    metadata: PluginMetadata,
}

impl DmesgPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Kernel ring buffer messages from dmesg",
                "splash",
            ),
        }
    }
}

impl Default for DmesgPlugin {
    fn default() -> Self {
        DmesgPlugin::new()
    }
}

impl Plugin for DmesgPlugin {
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

/// The kind a `dmesg -x` level is colored in
fn level_kind(level: &str) -> TokenKind {
    match level {
        "emerg" | "alert" | "crit" | "err" => TokenKind::Failure,
        "warn" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// Parses one dmesg line, or returns `None` when the line has none of the
/// prefixes dmesg prints and is not a kernel line from syslog.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, rest) = split_prefix(line)?;

    push_message(&mut tokens, rest);

    Some(ParsedLine::new(tokens))
}

/// Splits the prefixes dmesg prints, or the syslog header of a kernel line,
/// off a line, returning their tokens and the kernel message. Returns `None`
/// when the line has no prefix.
pub fn split_prefix(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let (mut tokens, mut rest, mut prefixed) = match syslog::split_header(line) {
        Some(header) if header.program == "kernel" => (header.tokens, header.body, true),
        _ => (Vec::new(), line, false),
    };

    if let Some((priority, after)) = syslog::split_priority(rest) {
        tokens.extend(priority);
        rest = after;
        prefixed = true;
    }

    if let Some(caps) = DECODE.captures(rest) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.push(Token::new(field(1), TokenKind::Module));
        push_padding(&mut tokens, field(2));
        tokens.extend([
            Token::new(field(3), TokenKind::Punctuation),
            Token::new(field(4), level_kind(field(4))),
        ]);
        push_padding(&mut tokens, field(5));
        tokens.extend([
            Token::new(field(6), TokenKind::Punctuation),
            Token::new(field(7), TokenKind::Plain),
        ]);

        rest = &rest[caps.get(0).unwrap().end()..];
        prefixed = true;
    }

    if let Some(caps) = TIMESTAMP.captures(rest) {
        tokens.push(Token::new(
            caps.get(1).unwrap().as_str(),
            TokenKind::Timestamp,
        ));
        push_padding(&mut tokens, caps.get(2).unwrap().as_str());

        rest = &rest[caps.get(0).unwrap().end()..];
        prefixed = true;
    }

    prefixed.then_some((tokens, rest))
}

fn push_padding<'a>(tokens: &mut Vec<Token<'a>>, padding: &'a str) {
    if !padding.is_empty() {
        tokens.push(Token::new(padding, TokenKind::Plain));
    }
}

/// Colors a kernel message: the subsystem it opens with, and the words that
/// report a problem. Text before the first colon that holds one of those
/// words, as in `Out of memory: `, is read as words rather than a subsystem.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut rest = message;

    if let Some(caps) = SUBSYSTEM
        .captures(message)
        .filter(|caps| !WORDS.is_match(caps.get(1).unwrap().as_str()))
    {
        tokens.extend([
            Token::new(caps.get(1).unwrap().as_str(), TokenKind::Module),
            Token::new(":", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);

        rest = &message[caps.get(0).unwrap().end()..];
    }

    mail::push_words(tokens, rest, &WORDS);
}
