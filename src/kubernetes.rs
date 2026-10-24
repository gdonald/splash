//! Kubernetes log parsing
//!
//! Kubernetes components log through klog, whose header reads
//! `I1003 12:00:01.123456    1234 controller.go:123] `: the severity
//! (`I`, `W`, `E`, or `F`), the date and time, the thread id padded to seven
//! characters, and the file and line. A structured message follows as a
//! quoted message and `key=value` pairs. The kubelet writes each line a
//! container printed to `/var/log/containers` in the CRI format,
//! `2023-10-03T12:00:01.123456789Z stdout F message`, where `F` marks a full
//! line and `P` a partial one. `kubectl logs` prints the container's lines
//! as they are, after an RFC 3339 time with `--timestamps` and after a
//! `[pod/NAME/CONTAINER]` tag with `--prefix`.
use crate::container;
use crate::logfmt;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "kubernetes";

/// A klog header
static KLOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([IWEF])(\d{4} \d{2}:\d{2}:\d{2}\.\d{6})(\s+)(\d+)( )([^\s:]+)(:)(\d+)(\])")
        .unwrap()
});

/// The quoted message a structured klog line opens with
static STRUCTURED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^( )(")((?:[^"\\]|\\.)*)(")"#).unwrap());

/// A CRI container log line
static CRI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[-+]\d{2}:\d{2}))( )(stdout|stderr)( )([PF](?::[PF])*)(?:( )(.*))?$")
        .unwrap()
});

/// The tag `kubectl logs --prefix` puts before each line
static PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\[[a-z]+/[^\]\s]+\])( )(.*)$").unwrap());

/// The Kubernetes plugin
pub struct KubernetesPlugin {
    metadata: PluginMetadata,
}

impl KubernetesPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Kubernetes component and container logs",
                "splash",
            ),
        }
    }
}

impl Default for KubernetesPlugin {
    fn default() -> Self {
        KubernetesPlugin::new()
    }
}

impl Plugin for KubernetesPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a> {
        match parse_line(line) {
            Some(parsed) => ParseResult::Parsed(parsed),
            None => ParseResult::NoMatch,
        }
    }

    fn parse_display_line<'a>(&self, line: &'a str) -> ParseResult<'a> {
        ParseResult::Parsed(parse_display_line(line))
    }
}

/// The kind a klog severity is colored in
pub fn severity_kind(severity: &str) -> TokenKind {
    match severity {
        "E" | "F" => TokenKind::Failure,
        "W" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// The kind a value of a structured klog line is colored in, by its key
pub fn value_kind(key: &str, _value: &str) -> Option<TokenKind> {
    match key {
        "pod" | "node" | "namespace" | "deployment" | "replicaset" | "service" | "job" => {
            Some(TokenKind::Module)
        }
        "podUID" | "uid" | "containerID" => Some(TokenKind::Transaction),
        "err" | "error" => Some(TokenKind::Failure),
        "duration" | "latency" => Some(TokenKind::Duration),
        _ => None,
    }
}

/// Parses one Kubernetes log line, a klog line on its own or behind a syslog
/// header, a CRI container log line, or a line `kubectl logs --prefix`
/// tagged, or returns `None` when the line is none of them.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_klog_line(line)
        .or_else(|| parse_syslog_line(line))
        .or_else(|| parse_cri_line(line))
        .or_else(|| parse_prefixed_line(line))
}

/// Parses a line for rendering in kubernetes mode: a line in none of the
/// formats above is a line `kubectl logs` printed, and a container's line is
/// colored in its own format.
pub fn parse_display_line(line: &str) -> ParsedLine<'_> {
    parse_klog_line(line)
        .or_else(|| parse_syslog_line(line))
        .or_else(|| cri_line(line, push_klog_or_output))
        .or_else(|| prefixed_line(line, push_container_line))
        .unwrap_or_else(|| {
            let mut tokens = Vec::new();

            push_container_line(&mut tokens, line);

            ParsedLine::new(tokens)
        })
}

/// Parses a line `kubectl logs --prefix` tagged with its pod and container.
pub fn parse_prefixed_line(line: &str) -> Option<ParsedLine<'_>> {
    prefixed_line(line, push_klog_or_text)
}

fn prefixed_line<'a>(
    line: &'a str,
    push_content: fn(&mut Vec<Token<'a>>, &'a str),
) -> Option<ParsedLine<'a>> {
    let caps = PREFIX.captures(line)?;

    let mut tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Module),
        Token::new(caps.get(2).unwrap().as_str(), TokenKind::Plain),
    ];

    push_content(&mut tokens, caps.get(3).unwrap().as_str());

    Some(ParsedLine::new(tokens))
}

/// Colors a line `kubectl logs` printed, after its `--timestamps` time when
/// it has one
fn push_container_line<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    if container::push_timestamped(tokens, text, push_klog_or_output).is_none() {
        push_klog_or_output(tokens, text);
    }
}

fn push_klog_or_text<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    if push_klog(tokens, text).is_none() {
        syslog::push_text(tokens, text);
    }
}

fn push_klog_or_output<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    if push_klog(tokens, text).is_none() {
        container::push_output(tokens, text);
    }
}

/// Parses a line that opens with a klog header.
pub fn parse_klog_line(line: &str) -> Option<ParsedLine<'_>> {
    let mut tokens = Vec::new();

    push_klog(&mut tokens, line)?;

    Some(ParsedLine::new(tokens))
}

/// Parses a klog line a component sent to syslog or the journal.
pub fn parse_syslog_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;
    let mut tokens = header.tokens;

    push_klog(&mut tokens, header.body)?;

    Some(ParsedLine::new(tokens))
}

/// Parses a CRI container log line. A container line that is itself a klog
/// line is colored as one.
pub fn parse_cri_line(line: &str) -> Option<ParsedLine<'_>> {
    cri_line(line, push_klog_or_text)
}

fn cri_line<'a>(
    line: &'a str,
    push_content: fn(&mut Vec<Token<'a>>, &'a str),
) -> Option<ParsedLine<'a>> {
    let caps = CRI.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());
    let stream = field(3).unwrap();

    let stream_kind = if stream == "stderr" {
        TokenKind::Warning
    } else {
        TokenKind::Level
    };

    let mut tokens = vec![
        Token::new(field(1).unwrap(), TokenKind::Timestamp),
        Token::new(field(2).unwrap(), TokenKind::Plain),
        Token::new(stream, stream_kind),
        Token::new(field(4).unwrap(), TokenKind::Plain),
        Token::new(field(5).unwrap(), TokenKind::Tag),
    ];

    if let Some(gap) = field(6) {
        tokens.push(Token::new(gap, TokenKind::Plain));

        push_content(&mut tokens, field(7).unwrap());
    }

    Some(ParsedLine::new(tokens))
}

/// Colors a klog header and the message after it, or returns `None`, adding
/// nothing, when the text does not open with a klog header.
fn push_klog<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) -> Option<()> {
    let caps = KLOG.captures(text)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    tokens.extend([
        Token::new(field(1), severity_kind(field(1))),
        Token::new(field(2), TokenKind::Timestamp),
        Token::new(field(3), TokenKind::Plain),
        Token::new(field(4), TokenKind::Pid),
        Token::new(field(5), TokenKind::Plain),
        Token::new(field(6), TokenKind::Path),
        Token::new(field(7), TokenKind::Punctuation),
        Token::new(field(8), TokenKind::Number),
        Token::new(field(9), TokenKind::Punctuation),
    ]);

    let mut message = &text[caps.get(0).unwrap().end()..];

    if let Some(structured) = STRUCTURED.captures(message) {
        let part = |index: usize| structured.get(index).unwrap().as_str();

        tokens.extend([
            Token::new(part(1), TokenKind::Plain),
            Token::new(part(2), TokenKind::Punctuation),
        ]);

        if !part(3).is_empty() {
            tokens.push(Token::new(part(3), TokenKind::Message));
        }

        tokens.push(Token::new(part(4), TokenKind::Punctuation));
        message = &message[structured.get(0).unwrap().end()..];
    }

    logfmt::push_pairs(tokens, message, value_kind);

    Some(())
}
