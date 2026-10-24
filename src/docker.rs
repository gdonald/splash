//! Docker log parsing
//!
//! The json-file logging driver writes each line a container printed as
//! `{"log":"listening on :8080\n","stream":"stdout","time":"2023-10-03T12:00:01.123456789Z"}`.
//! `docker logs --timestamps` prints the same line behind its time. dockerd
//! and containerd log their own messages as logfmt pairs, as in
//! `time="2023-10-03T12:00:01.123456789Z" level=info msg="Starting up"`,
//! to a file or through syslog and the journal. `docker logs` without
//! `--timestamps` prints the container's lines as they are, and
//! `docker compose logs` puts each one after the name of the service's
//! container and a bar, as in `web-1  | listening on :8080`.
use crate::container;
use crate::json;
use crate::logfmt;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "docker";

/// The programs whose syslog lines this plugin reads
const PROGRAMS: [&str; 3] = ["dockerd", "containerd", "docker"];

/// The time and level that open a daemon's logfmt line
static DAEMON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^time="[^"]*" level="#).unwrap());

/// The container name and bar `docker compose logs` puts before each line.
/// Compose names a service's containers with a replica number, as `web-1`
/// or `project_web_1`.
static COMPOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([\w.-]+[-_]\d+)( +\| )(.*)$").unwrap());

/// The Docker plugin
pub struct DockerPlugin {
    metadata: PluginMetadata,
}

impl DockerPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Docker container and daemon logs",
                "splash",
            ),
        }
    }
}

impl Default for DockerPlugin {
    fn default() -> Self {
        DockerPlugin::new()
    }
}

impl Plugin for DockerPlugin {
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

/// The kind a value of a json-file line is colored in, by its key
pub fn json_value_kind(key: &str, value: &str) -> Option<TokenKind> {
    match key {
        "log" => Some(TokenKind::Message),
        "stream" if value == "stderr" => Some(TokenKind::Warning),
        "stream" => Some(TokenKind::Level),
        "time" => Some(TokenKind::Timestamp),
        _ => None,
    }
}

/// The kind a value of a daemon's logfmt line is colored in, by its key
pub fn logfmt_value_kind(key: &str, value: &str) -> Option<TokenKind> {
    match key {
        "time" => Some(TokenKind::Timestamp),
        "level" => Some(logfmt::level_kind(value)),
        "msg" => Some(TokenKind::Message),
        "error" | "err" => Some(TokenKind::Failure),
        "container" | "id" | "container_id" | "task_id" => Some(TokenKind::Transaction),
        "module" | "type" | "namespace" => Some(TokenKind::Module),
        _ => None,
    }
}

/// Parses one Docker log line, or returns `None` when the line is in none of
/// its formats.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    json::parse_object(line, json_value_kind)
        .or_else(|| parse_daemon_line(line))
        .or_else(|| parse_timestamped_line(line))
        .or_else(|| parse_compose_line(line))
}

/// Parses a line from `docker compose logs`, with or without `--timestamps`.
pub fn parse_compose_line(line: &str) -> Option<ParsedLine<'_>> {
    compose_line(line, |tokens, text| {
        if container::push_timestamped(tokens, text, syslog::push_text).is_none() {
            syslog::push_text(tokens, text);
        }
    })
}

fn compose_line<'a>(
    line: &'a str,
    push_content: fn(&mut Vec<Token<'a>>, &'a str),
) -> Option<ParsedLine<'a>> {
    let caps = COMPOSE.captures(line)?;

    let mut tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Module),
        Token::new(caps.get(2).unwrap().as_str(), TokenKind::Punctuation),
    ];

    push_content(&mut tokens, caps.get(3).unwrap().as_str());

    Some(ParsedLine::new(tokens))
}

/// Parses a daemon's logfmt line, from its log file or from syslog.
pub fn parse_daemon_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = match syslog::split_header(line) {
        Some(header) if PROGRAMS.contains(&header.program) => (header.tokens, header.body),
        Some(_) => return None,
        None => (Vec::new(), line),
    };

    if !DAEMON.is_match(body) {
        return None;
    }

    logfmt::push_pairs(&mut tokens, body, logfmt_value_kind);

    Some(ParsedLine::new(tokens))
}

/// Parses a line from `docker logs --timestamps`.
pub fn parse_timestamped_line(line: &str) -> Option<ParsedLine<'_>> {
    let mut tokens = Vec::new();

    container::push_timestamped(&mut tokens, line, syslog::push_text)?;

    Some(ParsedLine::new(tokens))
}

/// Parses a line for rendering in docker mode: a line in none of Docker's
/// formats is a line a container printed, and a container's line, alone or
/// after its `--timestamps` time, is colored in its own format.
pub fn parse_display_line(line: &str) -> ParsedLine<'_> {
    compose_line(line, push_container_line).unwrap_or_else(|| {
        let mut tokens = Vec::new();

        if container::push_timestamped(&mut tokens, line, container::push_output).is_some() {
            return ParsedLine::new(tokens);
        }

        json::parse_object(line, json_value_kind)
            .or_else(|| parse_daemon_line(line))
            .unwrap_or_else(|| container::parse_output(line))
    })
}

/// Colors a line a container printed, after its `--timestamps` time when it
/// has one
fn push_container_line<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    if container::push_timestamped(tokens, text, container::push_output).is_none() {
        container::push_output(tokens, text);
    }
}
