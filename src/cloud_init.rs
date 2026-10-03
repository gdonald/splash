//! cloud-init log parsing
//!
//! cloud-init writes `/var/log/cloud-init.log` with the Python logging
//! format `%(asctime)s - %(filename)s[%(levelname)s]: %(message)s`, as in
//! `2023-10-03 12:00:01,123 - util.py[DEBUG]: Cloud-init v. 23.3.1 running
//! 'init-local' at ...`, and sends the same messages to syslog as
//! `[CLOUDINIT] util.py[DEBUG]: ...`. Its stages report each module as
//! `start:` and `finish:` events with `SUCCESS` or `FAIL`.
//! `/var/log/cloud-init-output.log` holds the stage banners and the
//! `ci-info:` tables of network and key information.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "cloud-init";

/// The date that opens a line of `cloud-init.log`
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2},\d{3})( - )").unwrap());

/// The `[CLOUDINIT] ` tag of a syslog line
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\[)(CLOUDINIT)(\])( )").unwrap());

/// The file and level a message was logged from
static SOURCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\w.-]+)(\[)([A-Z]+)(\])(: )").unwrap());

/// A line of `cloud-init-output.log`: a stage banner or a `ci-info:` line
static OUTPUT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:Cloud-init v\. |ci-info: )").unwrap());

/// The parts of a message that carry a value: the cloud-init version, a
/// stage or module in quotes, a stage event, and the seconds since boot
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (Cloud-init\ v\.\ )(\S+)                      # version
        |
        (')([^']*)(')                                 # stage or module
        |
        \b(start|finish)(:\ )([\w/-]+)(:)             # event
        |
        \b(Up\ )(\d+(?:\.\d+)?)(\ seconds)            # seconds since boot
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("SUCCESS", TokenKind::Success),
        ("ran successfully", TokenKind::Success),
        ("finished at", TokenKind::Success),
        ("FAIL", TokenKind::Failure),
        ("failed", TokenKind::Failure),
        ("Failed", TokenKind::Failure),
        ("Traceback", TokenKind::Failure),
        ("Exception", TokenKind::Failure),
        ("WARNING", TokenKind::Warning),
        ("Skipping", TokenKind::Warning),
    ])
});

/// The cloud-init plugin
pub struct CloudInitPlugin {
    metadata: PluginMetadata,
}

impl CloudInitPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "cloud-init instance initialization logs",
                "splash",
            ),
        }
    }
}

impl Default for CloudInitPlugin {
    fn default() -> Self {
        CloudInitPlugin::new()
    }
}

impl Plugin for CloudInitPlugin {
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

/// The kind a Python logging level is colored in
fn level_kind(level: &str) -> TokenKind {
    match level {
        "ERROR" | "CRITICAL" => TokenKind::Failure,
        "WARNING" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// Parses one cloud-init log line, from `cloud-init.log`, syslog, or
/// `cloud-init-output.log`, or returns `None` when the line is in none of
/// them.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_log_line(line)
        .or_else(|| parse_syslog_line(line))
        .or_else(|| parse_output_line(line))
}

/// Parses a line of `cloud-init.log`.
pub fn parse_log_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = DATE.captures(line)?;

    let mut tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Timestamp),
        Token::new(caps.get(2).unwrap().as_str(), TokenKind::Punctuation),
    ];

    push_source_and_message(&mut tokens, &line[caps.get(0).unwrap().end()..])?;

    Some(ParsedLine::new(tokens))
}

/// Parses a `[CLOUDINIT]` line from syslog.
pub fn parse_syslog_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;
    let caps = TAG.captures(header.body)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = header.tokens;

    tokens.extend([
        Token::new(field(1), TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Tag),
        Token::new(field(3), TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Plain),
    ]);

    push_source_and_message(&mut tokens, &header.body[caps.get(0).unwrap().end()..])?;

    Some(ParsedLine::new(tokens))
}

/// Parses a stage banner or a `ci-info:` line of `cloud-init-output.log`.
pub fn parse_output_line(line: &str) -> Option<ParsedLine<'_>> {
    if !OUTPUT.is_match(line) {
        return None;
    }

    let mut tokens = Vec::new();

    push_message(&mut tokens, line);

    Some(ParsedLine::new(tokens))
}

/// Colors the file and level a message was logged from, then the message,
/// or returns `None` when the text does not open with them.
fn push_source_and_message<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) -> Option<()> {
    let caps = SOURCE.captures(text)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    tokens.extend([
        Token::new(field(1), TokenKind::Path),
        Token::new(field(2), TokenKind::Punctuation),
        Token::new(field(3), level_kind(field(3))),
        Token::new(field(4), TokenKind::Punctuation),
        Token::new(field(5), TokenKind::Punctuation),
    ]);

    push_message(tokens, &text[caps.get(0).unwrap().end()..]);

    Some(())
}

/// Colors a message.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(version) = field(2) {
            tokens.extend([
                Token::new(field(1).unwrap(), TokenKind::Tag),
                Token::new(version, TokenKind::Number),
            ]);
        } else if let Some(quoted) = field(4) {
            tokens.push(Token::new(field(3).unwrap(), TokenKind::Punctuation));

            if !quoted.is_empty() {
                tokens.push(Token::new(quoted, TokenKind::Module));
            }

            tokens.push(Token::new(field(5).unwrap(), TokenKind::Punctuation));
        } else if let Some(event) = field(6) {
            tokens.extend([
                Token::new(event, TokenKind::Header),
                Token::new(field(7).unwrap(), TokenKind::Punctuation),
                Token::new(field(8).unwrap(), TokenKind::Module),
                Token::new(field(9).unwrap(), TokenKind::Punctuation),
            ]);
        } else {
            tokens.extend([
                Token::new(field(10).unwrap(), TokenKind::Message),
                Token::new(field(11).unwrap(), TokenKind::Duration),
                Token::new(field(12).unwrap(), TokenKind::Message),
            ]);
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
