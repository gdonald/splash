//! Procmail log parsing
//!
//! Procmail's log file holds an abstract of each delivered mail on three
//! lines: `From` with the envelope sender and date, ` Subject:`, and
//! `  Folder:` with the mailbox or program the mail went to and its size.
//! Diagnostics, and the recipe trace `VERBOSE=on` turns on, are written
//! between them with a `procmail: ` prefix.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "procmail";

/// The first line of an abstract: the envelope sender and the date
static FROM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^(From) (\S+)(\s+)({})$", syslog::CTIME)).unwrap());

/// The second line of an abstract: the subject
static SUBJECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s+)(Subject):(.*)$").unwrap());

/// The third line of an abstract: where the mail went and its size
static FOLDER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s+)(Folder):(\s+)(\S.*?)(\s+)(\d+)$").unwrap());

/// A diagnostic, with the process id `VERBOSE=on` adds
static DIAGNOSTIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(procmail): (?:\[(\d+)\] )?").unwrap());

/// A diagnostic that is only the date, written when `VERBOSE=on` starts
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^{}$", syslog::CTIME)).unwrap());

/// A quoted file, recipe condition, or assignment inside a diagnostic
static QUOTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]*)""#).unwrap());

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Match on", TokenKind::Success),
        ("No match on", TokenKind::Warning),
        ("Skipped", TokenKind::Warning),
        ("Timeout", TokenKind::Warning),
        ("Couldn't", TokenKind::Failure),
        ("Error", TokenKind::Failure),
        ("Unable", TokenKind::Failure),
        ("Lock failure", TokenKind::Failure),
        ("Bad substitution", TokenKind::Failure),
    ])
});

/// The Procmail log plugin
pub struct ProcmailPlugin {
    metadata: PluginMetadata,
}

impl ProcmailPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Procmail mail filtering and delivery logs",
                "splash",
            ),
        }
    }
}

impl Default for ProcmailPlugin {
    fn default() -> Self {
        ProcmailPlugin::new()
    }
}

impl Plugin for ProcmailPlugin {
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

/// Parses one procmail log line, or returns `None` when the line is neither
/// part of an abstract nor a diagnostic.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_from_line(line)
        .or_else(|| parse_subject_line(line))
        .or_else(|| parse_folder_line(line))
        .or_else(|| parse_diagnostic(line))
}

/// Parses the `From` line that opens an abstract.
pub fn parse_from_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = FROM.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    Some(ParsedLine::new(vec![
        Token::new(field(1), TokenKind::Header),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::Email),
        Token::new(field(3), TokenKind::Plain),
        Token::new(field(4), TokenKind::Timestamp),
    ]))
}

/// Parses the ` Subject:` line of an abstract.
pub fn parse_subject_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = SUBJECT.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Plain),
        Token::new(field(2), TokenKind::Header),
        Token::new(":", TokenKind::Punctuation),
    ];

    if !field(3).is_empty() {
        tokens.push(Token::new(field(3), TokenKind::Message));
    }

    Some(ParsedLine::new(tokens))
}

/// Parses the `  Folder:` line that closes an abstract.
pub fn parse_folder_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = FOLDER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    Some(ParsedLine::new(vec![
        Token::new(field(1), TokenKind::Plain),
        Token::new(field(2), TokenKind::Header),
        Token::new(":", TokenKind::Punctuation),
        Token::new(field(3), TokenKind::Plain),
        Token::new(field(4), TokenKind::Path),
        Token::new(field(5), TokenKind::Plain),
        Token::new(field(6), TokenKind::Size),
    ]))
}

/// Parses a `procmail: ` diagnostic.
pub fn parse_diagnostic(line: &str) -> Option<ParsedLine<'_>> {
    let caps = DIAGNOSTIC.captures(line)?;

    let mut tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Tag),
        Token::new(":", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ];

    if let Some(pid) = caps.get(2) {
        tokens.extend([
            Token::new("[", TokenKind::Punctuation),
            Token::new(pid.as_str(), TokenKind::Pid),
            Token::new("]", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);
    }

    let message = &line[caps.get(0).unwrap().end()..];

    if DATE.is_match(message) {
        tokens.push(Token::new(message, TokenKind::Timestamp));
    } else {
        push_message(&mut tokens, message);
    }

    Some(ParsedLine::new(tokens))
}

/// Colors a diagnostic's message. A quoted path is colored as a path, and any
/// other quoted text is read for `key=value` assignments.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in QUOTED.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let quoted = caps.get(1).unwrap().as_str();

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        tokens.push(Token::new("\"", TokenKind::Punctuation));

        if quoted.starts_with('/') {
            tokens.push(Token::new(quoted, TokenKind::Path));
        } else {
            mail::push_fields(tokens, quoted, &WORDS);
        }

        tokens.push(Token::new("\"", TokenKind::Punctuation));

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
