//! Terraform log parsing
//!
//! Reads three kinds of Terraform output. `TF_LOG` writes hclog lines,
//! `2023-10-03T12:00:01.123Z [INFO]  provider: message: key=value`, with the
//! level bracketed and padded. `-json` writes one object per line with
//! `@level`, `@message`, `@module`, `@timestamp`, and `type` keys. The CLI
//! prints each resource operation as `aws_instance.web: Creation complete
//! after 32s [id=i-0abc]`, the plan as `+`, `~`, and `-` lines, and closes
//! with a summary such as `Apply complete! Resources: 1 added, 0 changed, 0
//! destroyed.`.
use crate::json;
use crate::logfmt;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "terraform";

/// The time and level of an hclog line
static HCLOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}(?:Z|[-+]\d{4}))( )(\[)(TRACE|DEBUG|INFO|WARN|ERROR)(\])( +)").unwrap()
});

/// The logger name that opens an hclog message, such as `provider: `
static LOGGER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([\w.@/-]+)(: )").unwrap());

/// A resource operation, with how long it took and the resource's id, or
/// how long it has been running
static OPERATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\S+)(: )([A-Z][\w ]*?(?:\.\.\.|complete|errored))(?:( after )(\S+?))?(?:( \[)(\w+)(=)([^\]]*)(\]))?(?:( \[)(\S+)( elapsed)(\]))?$")
        .unwrap()
});

/// A summary line, such as `Plan: 1 to add, 0 to change, 0 to destroy.`
static SUMMARY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Apply complete!|Destroy complete!|Plan:|No changes\.)(.*)$").unwrap()
});

/// A plan line, with its action sign, or a comment naming what will happen
static PLAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(-/\+|\+/-|<=|[+~-]|#)( )(.*)$").unwrap());

/// A diagnostic, such as `Error: Invalid reference`, inside its box or not
static DIAGNOSTIC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(│ |╷|╵)?(?:(Error|Warning)(: ))?(.*)$").unwrap());

/// A count inside a summary line
static COUNT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d+\b").unwrap());

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("will be created", TokenKind::Success),
        ("will be updated in-place", TokenKind::Warning),
        ("must be replaced", TokenKind::Warning),
        ("will be destroyed", TokenKind::Failure),
        ("will be read during apply", TokenKind::Level),
    ])
});

/// The Terraform plugin
pub struct TerraformPlugin {
    metadata: PluginMetadata,
}

impl TerraformPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Terraform logs, JSON output, and CLI output",
                "splash",
            ),
        }
    }
}

impl Default for TerraformPlugin {
    fn default() -> Self {
        TerraformPlugin::new()
    }
}

impl Plugin for TerraformPlugin {
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

/// The kind a value of a `-json` line is colored in, by its key
pub fn json_value_kind(key: &str, value: &str) -> Option<TokenKind> {
    match key {
        "@level" => Some(logfmt::level_kind(value)),
        "@message" => Some(TokenKind::Message),
        "@module" => Some(TokenKind::Module),
        "@timestamp" => Some(TokenKind::Timestamp),
        "type" => Some(TokenKind::Tag),
        "addr" | "resource" | "resource_name" => Some(TokenKind::Module),
        "action" => Some(TokenKind::Method),
        "id_value" | "id_key" => Some(TokenKind::Transaction),
        "elapsed_seconds" => Some(TokenKind::Duration),
        "severity" => Some(logfmt::level_kind(value)),
        _ => None,
    }
}

/// The kind a value of an hclog line is colored in, by its key
fn hclog_value_kind(key: &str, _value: &str) -> Option<TokenKind> {
    match key {
        "tf_req_id" | "tf_rpc" => Some(TokenKind::Transaction),
        "tf_resource_type" | "tf_provider_addr" | "@module" => Some(TokenKind::Module),
        "@caller" => Some(TokenKind::Path),
        "timestamp" => Some(TokenKind::Timestamp),
        "error" | "err" => Some(TokenKind::Failure),
        _ => None,
    }
}

/// The kind an operation is colored in: a finished one is a success, an
/// errored one a failure, and one in progress a warning
fn operation_kind(operation: &str) -> TokenKind {
    if operation.ends_with("complete") {
        TokenKind::Success
    } else if operation.ends_with("errored") {
        TokenKind::Failure
    } else {
        TokenKind::Warning
    }
}

/// The kind a plan line's action is colored in
fn action_kind(sign: &str) -> TokenKind {
    match sign {
        "+" => TokenKind::Success,
        "-" | "-/+" | "+/-" => TokenKind::Failure,
        "~" => TokenKind::Warning,
        "#" => TokenKind::Punctuation,
        _ => TokenKind::Level,
    }
}

/// Parses one line of Terraform output, or returns `None` when the line is
/// in none of its formats.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    json::parse_object(line, json_value_kind)
        .or_else(|| parse_hclog_line(line))
        .or_else(|| parse_operation_line(line))
        .or_else(|| parse_summary_line(line))
        .or_else(|| parse_plan_line(line))
        .or_else(|| parse_diagnostic_line(line))
}

/// Parses a `TF_LOG` line.
pub fn parse_hclog_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = HCLOG.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), TokenKind::Punctuation),
        Token::new(field(4), logfmt::level_kind(field(4))),
        Token::new(field(5), TokenKind::Punctuation),
        Token::new(field(6), TokenKind::Plain),
    ];

    let mut message = &line[caps.get(0).unwrap().end()..];

    if let Some(logger) = LOGGER.captures(message) {
        tokens.extend([
            Token::new(logger.get(1).unwrap().as_str(), TokenKind::Module),
            Token::new(logger.get(2).unwrap().as_str(), TokenKind::Punctuation),
        ]);

        message = &message[logger.get(0).unwrap().end()..];
    }

    logfmt::push_pairs(&mut tokens, message, hclog_value_kind);

    Some(ParsedLine::new(tokens))
}

/// Parses a resource operation line of the CLI's output.
pub fn parse_operation_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = OPERATION.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());
    let operation = field(3).unwrap();

    let mut tokens = vec![
        Token::new(field(1).unwrap(), TokenKind::Module),
        Token::new(field(2).unwrap(), TokenKind::Punctuation),
        Token::new(operation, operation_kind(operation)),
    ];

    if let Some(after) = field(4) {
        tokens.extend([
            Token::new(after, TokenKind::Message),
            Token::new(field(5).unwrap(), TokenKind::Duration),
        ]);
    }

    if let Some(open) = field(6) {
        tokens.extend([
            Token::new(open, TokenKind::Punctuation),
            Token::new(field(7).unwrap(), TokenKind::Header),
            Token::new(field(8).unwrap(), TokenKind::Punctuation),
        ]);

        if !field(9).unwrap().is_empty() {
            tokens.push(Token::new(field(9).unwrap(), TokenKind::Transaction));
        }

        tokens.push(Token::new(field(10).unwrap(), TokenKind::Punctuation));
    }

    if let Some(open) = field(11) {
        tokens.extend([
            Token::new(open, TokenKind::Punctuation),
            Token::new(field(12).unwrap(), TokenKind::Duration),
            Token::new(field(13).unwrap(), TokenKind::Message),
            Token::new(field(14).unwrap(), TokenKind::Punctuation),
        ]);
    }

    Some(ParsedLine::new(tokens))
}

/// Parses a summary line, coloring its counts.
pub fn parse_summary_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = SUMMARY.captures(line)?;
    let heading = caps.get(1).unwrap().as_str();
    let rest = caps.get(2).unwrap().as_str();

    let kind = if heading == "Plan:" {
        TokenKind::Header
    } else {
        TokenKind::Success
    };

    let mut tokens = vec![Token::new(heading, kind)];
    let mut cursor = 0;

    for count in COUNT.find_iter(rest) {
        if count.start() > cursor {
            tokens.push(Token::new(&rest[cursor..count.start()], TokenKind::Message));
        }

        tokens.push(Token::new(count.as_str(), TokenKind::Number));
        cursor = count.end();
    }

    if cursor < rest.len() {
        tokens.push(Token::new(&rest[cursor..], TokenKind::Message));
    }

    Some(ParsedLine::new(tokens))
}

/// Parses a line of a plan: an attribute or resource with its action sign,
/// or a comment saying what will happen to a resource.
pub fn parse_plan_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = PLAN.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();
    let sign = field(2);

    let mut tokens = Vec::new();

    if !field(1).is_empty() {
        tokens.push(Token::new(field(1), TokenKind::Plain));
    }

    tokens.extend([
        Token::new(sign, action_kind(sign)),
        Token::new(field(3), TokenKind::Plain),
    ]);

    mail::push_words(&mut tokens, field(4), &WORDS);

    Some(ParsedLine::new(tokens))
}

/// Parses a diagnostic line, such as `Error: Invalid reference` or
/// `│ Error: Invalid reference` inside its box, or a line of its box.
pub fn parse_diagnostic_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = DIAGNOSTIC.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());

    if field(1).is_none() && field(2).is_none() {
        return None;
    }

    let mut tokens = Vec::new();

    if let Some(border) = field(1) {
        tokens.push(Token::new(border, TokenKind::Punctuation));
    }

    if let Some(severity) = field(2) {
        let kind = if severity == "Error" {
            TokenKind::Failure
        } else {
            TokenKind::Warning
        };

        tokens.extend([
            Token::new(severity, kind),
            Token::new(field(3).unwrap(), TokenKind::Punctuation),
        ]);
    }

    mail::push_words(&mut tokens, field(4).unwrap(), &WORDS);

    Some(ParsedLine::new(tokens))
}
