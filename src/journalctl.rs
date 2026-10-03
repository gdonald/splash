//! journalctl parsing
//!
//! `journalctl`'s `short` output modes print each journal entry as a syslog
//! line: a timestamp, the host, the program, and the message. The timestamp
//! form depends on the mode, and every form the shared syslog header reads is
//! accepted here. Between entries journalctl prints markers such as
//! `-- Boot 1a2b... --` and `-- No entries --`. systemd's messages about its
//! units are colored by what happened to the unit.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "journalctl";

/// A boot marker, with the boot id
static BOOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(-- )(Boot)( )([0-9a-f]{32})( --)$").unwrap());

/// Any other marker, such as `-- No entries --`
static MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(-- )(.*)( --)$").unwrap());

/// A unit name, or a `key=value` field such as `code=exited`
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ([\w@\\:.-]+\.(?:service|socket|target|timer|mount|automount|path|slice|scope|device|swap))\b
        |
        \b([a-z_]+)=([^\s,]+)
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Started", TokenKind::Success),
        ("Finished", TokenKind::Success),
        ("Reached target", TokenKind::Success),
        ("Listening on", TokenKind::Success),
        ("Stopping", TokenKind::Warning),
        ("Stopped", TokenKind::Warning),
        ("Main process exited", TokenKind::Warning),
        ("Failed to start", TokenKind::Failure),
        ("Failed", TokenKind::Failure),
        ("failed", TokenKind::Failure),
        ("error", TokenKind::Failure),
    ])
});

/// The journalctl plugin
pub struct JournalctlPlugin {
    metadata: PluginMetadata,
}

impl JournalctlPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "systemd journal output from journalctl",
                "splash",
            ),
        }
    }
}

impl Default for JournalctlPlugin {
    fn default() -> Self {
        JournalctlPlugin::new()
    }
}

impl Plugin for JournalctlPlugin {
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

/// Parses one line of journalctl output, or returns `None` when the line is
/// neither an entry nor a marker.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_marker(line).or_else(|| parse_entry(line))
}

/// Parses a marker journalctl prints between entries.
pub fn parse_marker(line: &str) -> Option<ParsedLine<'_>> {
    if let Some(caps) = BOOT.captures(line) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        return Some(ParsedLine::new(vec![
            Token::new(field(1), TokenKind::Punctuation),
            Token::new(field(2), TokenKind::Header),
            Token::new(field(3), TokenKind::Plain),
            Token::new(field(4), TokenKind::Transaction),
            Token::new(field(5), TokenKind::Punctuation),
        ]));
    }

    let caps = MARKER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![Token::new(field(1), TokenKind::Punctuation)];

    if !field(2).is_empty() {
        tokens.push(Token::new(field(2), TokenKind::Message));
    }

    tokens.push(Token::new(field(3), TokenKind::Punctuation));

    Some(ParsedLine::new(tokens))
}

/// Parses a journal entry.
pub fn parse_entry(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;
    let mut tokens = header.tokens;

    push_message(&mut tokens, header.body);

    Some(ParsedLine::new(tokens))
}

/// Colors an entry's message: unit names, `key=value` fields, and the words
/// that say what happened to a unit.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(unit) = caps.get(1) {
            tokens.push(Token::new(unit.as_str(), TokenKind::Module));
        } else {
            tokens.extend([
                Token::new(caps.get(2).unwrap().as_str(), TokenKind::Header),
                Token::new("=", TokenKind::Punctuation),
            ]);
            mail::push_words(tokens, caps.get(3).unwrap().as_str(), &WORDS);
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
