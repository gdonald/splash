//! CI log parsing
//!
//! Reads three kinds of CI log. A Jenkins console log is a build's output,
//! with Jenkins' own lines among it: `Started by user alice`, the
//! `[Pipeline]` steps, the `+ make test` lines a shell step echoes, and
//! `Finished: SUCCESS`. The Timestamper plugin can put the time before each
//! line. Jenkins' server log, `jenkins.log`, writes
//! `2023-10-03 12:00:01.123+0000 [id=42]<TAB>INFO<TAB>hudson.model.Run#execute: message`.
//! A GitHub Actions log puts the time before each line and marks groups,
//! errors, and warnings with `##[group]`, `##[error]`, and the like, or with
//! the `::error::` workflow commands a step prints.
//!
//! Every line of a build's output belongs to the log, so this mode keeps
//! each line, coloring the ones it knows and the words that report a
//! problem in the rest.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "ci";

/// A line of Jenkins' server log
static SERVER_LOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}[-+]\d{4})( )(\[)(id)(=)(\d+)(\])(\t)([A-Z]+)(\t)(\S+?)(: )(.*)$")
        .unwrap()
});

/// The time a GitHub Actions log or Jenkins' Timestamper plugin puts before
/// a line
static TIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)( )|(\[)(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)(\] )|(\d{2}:\d{2}:\d{2})( ))")
        .unwrap()
});

/// A GitHub Actions log marker, such as `##[error]`
static MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(##\[)(group|endgroup|section|command|error|warning|notice|debug)(\])(.*)$")
        .unwrap()
});

/// A GitHub Actions workflow command, such as `::error file=app.js::message`
static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(::)(error|warning|notice|debug|group|endgroup|add-mask)( [^:]*)?(::)(.*)$")
        .unwrap()
});

/// A Jenkins pipeline step, such as `[Pipeline] sh`
static PIPELINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\[)(Pipeline)(\])( )(.*)$").unwrap());

/// How a Jenkins build finished
static FINISHED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Finished)(: )(SUCCESS|UNSTABLE|FAILURE|NOT_BUILT|ABORTED)$").unwrap()
});

/// A command a shell step echoes
static SHELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\++)( )(.+)$").unwrap());

/// A line Jenkins writes when a build starts
static STARTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Started by|Running as|Running on|Building in workspace|Obtained \S+ from)(.*)$")
        .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("BUILD SUCCESS", TokenKind::Success),
        ("BUILD SUCCESSFUL", TokenKind::Success),
        ("BUILD FAILURE", TokenKind::Failure),
        ("BUILD FAILED", TokenKind::Failure),
        ("SUCCESS", TokenKind::Success),
        ("PASSED", TokenKind::Success),
        ("passed", TokenKind::Success),
        ("FAILED", TokenKind::Failure),
        ("FAILURE", TokenKind::Failure),
        ("failed", TokenKind::Failure),
        ("ERROR", TokenKind::Failure),
        ("Error", TokenKind::Failure),
        ("error", TokenKind::Failure),
        ("Exception", TokenKind::Failure),
        ("WARNING", TokenKind::Warning),
        ("Warning", TokenKind::Warning),
        ("warning", TokenKind::Warning),
        ("UNSTABLE", TokenKind::Warning),
        ("ABORTED", TokenKind::Warning),
    ])
});

/// The CI log plugin
pub struct CiPlugin {
    metadata: PluginMetadata,
}

impl CiPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Jenkins and GitHub Actions build logs",
                "splash",
            ),
        }
    }
}

impl Default for CiPlugin {
    fn default() -> Self {
        CiPlugin::new()
    }
}

impl Plugin for CiPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a> {
        ParseResult::Parsed(parse_line(line))
    }

    /// Every line parses, so confidence comes from the lines that carry a
    /// CI marker instead.
    fn detect_format(&self, sample_lines: &[&str]) -> f32 {
        if sample_lines.is_empty() {
            return 0.0;
        }

        let marked = sample_lines.iter().filter(|line| is_marked(line)).count();

        marked as f32 / sample_lines.len() as f32
    }
}

/// Whether a line carries a marker only a CI log writes
pub fn is_marked(line: &str) -> bool {
    let (_, rest) = split_time(line);

    SERVER_LOG.is_match(line)
        || MARKER.is_match(rest)
        || COMMAND.is_match(rest)
        || PIPELINE.is_match(rest)
        || FINISHED.is_match(rest)
        || STARTED.is_match(rest)
}

/// The kind a Jenkins server log level is colored in
fn level_kind(level: &str) -> TokenKind {
    match level {
        "SEVERE" => TokenKind::Failure,
        "WARNING" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// The kind a GitHub Actions marker or command is colored in
fn marker_kind(marker: &str) -> TokenKind {
    match marker {
        "error" => TokenKind::Failure,
        "warning" => TokenKind::Warning,
        "group" | "endgroup" | "section" => TokenKind::Header,
        _ => TokenKind::Level,
    }
}

/// The kind a Jenkins build result is colored in
fn result_kind(result: &str) -> TokenKind {
    match result {
        "SUCCESS" => TokenKind::Success,
        "FAILURE" => TokenKind::Failure,
        _ => TokenKind::Warning,
    }
}

/// Parses one line of a CI log. Every line parses.
pub fn parse_line(line: &str) -> ParsedLine<'_> {
    if let Some(parsed) = parse_server_log_line(line) {
        return parsed;
    }

    let (mut tokens, rest) = split_time(line);

    push_console_line(&mut tokens, rest);

    ParsedLine::new(tokens)
}

/// Parses a line of Jenkins' server log.
pub fn parse_server_log_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = SERVER_LOG.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), TokenKind::Punctuation),
        Token::new(field(4), TokenKind::Header),
        Token::new(field(5), TokenKind::Punctuation),
        Token::new(field(6), TokenKind::Pid),
        Token::new(field(7), TokenKind::Punctuation),
        Token::new(field(8), TokenKind::Plain),
        Token::new(field(9), level_kind(field(9))),
        Token::new(field(10), TokenKind::Plain),
        Token::new(field(11), TokenKind::Module),
        Token::new(field(12), TokenKind::Punctuation),
    ];

    mail::push_words(&mut tokens, field(13), &WORDS);

    Some(ParsedLine::new(tokens))
}

/// Splits the time off a line, returning its tokens and the rest of the
/// line.
fn split_time(line: &str) -> (Vec<Token<'_>>, &str) {
    let Some(caps) = TIME.captures(line) else {
        return (Vec::new(), line);
    };

    let tokens = (1..=7)
        .filter_map(|index| caps.get(index).map(|found| (index, found.as_str())))
        .map(|(index, text)| {
            let kind = match index {
                1 | 4 | 6 => TokenKind::Timestamp,
                3 | 5 => TokenKind::Punctuation,
                _ => TokenKind::Plain,
            };

            Token::new(text, kind)
        })
        .collect();

    (tokens, &line[caps.get(0).unwrap().end()..])
}

/// Colors a console line: a marker, a workflow command, a pipeline step, a
/// build result, a shell command, a build start, or build output.
fn push_console_line<'a>(tokens: &mut Vec<Token<'a>>, line: &'a str) {
    if let Some(caps) = MARKER.captures(line) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(field(1), TokenKind::Punctuation),
            Token::new(field(2), marker_kind(field(2))),
            Token::new(field(3), TokenKind::Punctuation),
        ]);
        mail::push_words(tokens, field(4), &WORDS);
    } else if let Some(caps) = COMMAND.captures(line) {
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        tokens.extend([
            Token::new(field(1).unwrap(), TokenKind::Punctuation),
            Token::new(field(2).unwrap(), marker_kind(field(2).unwrap())),
        ]);

        if let Some(parameters) = field(3) {
            tokens.push(Token::new(parameters, TokenKind::Path));
        }

        tokens.push(Token::new(field(4).unwrap(), TokenKind::Punctuation));
        mail::push_words(tokens, field(5).unwrap(), &WORDS);
    } else if let Some(caps) = PIPELINE.captures(line) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(field(1), TokenKind::Punctuation),
            Token::new(field(2), TokenKind::Tag),
            Token::new(field(3), TokenKind::Punctuation),
            Token::new(field(4), TokenKind::Plain),
            Token::new(field(5), TokenKind::Module),
        ]);
    } else if let Some(caps) = FINISHED.captures(line) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(field(1), TokenKind::Header),
            Token::new(field(2), TokenKind::Punctuation),
            Token::new(field(3), result_kind(field(3))),
        ]);
    } else if let Some(caps) = SHELL.captures(line) {
        let field = |index: usize| caps.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(field(1), TokenKind::Punctuation),
            Token::new(field(2), TokenKind::Plain),
            Token::new(field(3), TokenKind::Request),
        ]);
    } else if let Some(caps) = STARTED.captures(line) {
        tokens.push(Token::new(caps.get(1).unwrap().as_str(), TokenKind::Header));
        mail::push_words(tokens, caps.get(2).unwrap().as_str(), &WORDS);
    } else {
        mail::push_words(tokens, line, &WORDS);
    }
}
