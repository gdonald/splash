//! Redis log parsing
//!
//! Redis writes each line as `pid:role date mark message`, as in
//! `1234:M 03 Oct 2023 12:00:01.123 * Ready to accept connections tcp`. The
//! role is `M` for a master, `S` for a replica, `C` for a child process
//! writing an RDB or AOF file, and `X` for Sentinel. The mark gives the
//! level: `.` debug, `-` verbose, `*` notice, and `#` warning.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "redis";

static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d+)(:)([XCSM])                                    # process id and role
        \ (\d{2}\ [A-Z][a-z]{2}\ \d{4}\ \d{2}:\d{2}:\d{2}\.\d{3})   # date
        \ ([.*\#-])                                         # level mark
        (.*)                                                # message, with its leading space
        $
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Ready to accept connections", TokenKind::Success),
        ("DB saved on disk", TokenKind::Success),
        (
            "Background saving terminated with success",
            TokenKind::Success,
        ),
        (
            "MASTER <-> REPLICA sync: Finished with success",
            TokenKind::Success,
        ),
        ("WARNING", TokenKind::Warning),
        ("Error", TokenKind::Failure),
        ("error", TokenKind::Failure),
        ("failed", TokenKind::Failure),
        ("Failed", TokenKind::Failure),
        ("MISCONF", TokenKind::Failure),
        ("Out Of Memory", TokenKind::Failure),
    ])
});

/// The Redis log plugin
pub struct RedisPlugin {
    metadata: PluginMetadata,
}

impl RedisPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Redis server logs",
                "splash",
            ),
        }
    }
}

impl Default for RedisPlugin {
    fn default() -> Self {
        RedisPlugin::new()
    }
}

impl Plugin for RedisPlugin {
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

/// The kind a level mark is colored in: `#` is a warning, and the rest are
/// levels
fn mark_kind(mark: &str) -> TokenKind {
    if mark == "#" {
        TokenKind::Warning
    } else {
        TokenKind::Level
    }
}

/// Parses one Redis log line, or returns `None` when the line is not one.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LINE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Pid),
        Token::new(field(2), TokenKind::Punctuation),
        Token::new(field(3), TokenKind::Module),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(5), mark_kind(field(5))),
    ];

    mail::push_words(&mut tokens, field(6), &WORDS);

    Some(ParsedLine::new(tokens))
}
