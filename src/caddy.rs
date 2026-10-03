//! Caddy structured log parsing
//!
//! Caddy's JSON encoder writes one object per line, read by the shared JSON
//! scanner. A value takes its color from the key that introduces it, in the
//! nested `request` and `headers` objects too, and a key Caddy alone knows
//! about still reads as a key.
use crate::json;
use crate::output::{ParsedLine, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "caddy";

/// The Caddy structured log plugin
pub struct CaddyPlugin {
    metadata: PluginMetadata,
}

impl CaddyPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Caddy structured JSON logs",
                "splash",
            ),
        }
    }
}

impl Default for CaddyPlugin {
    fn default() -> Self {
        CaddyPlugin::new()
    }
}

impl Plugin for CaddyPlugin {
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

/// Parses one JSON object, or returns `None` when the line is not a single
/// well formed object.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    json::parse_object(line, |key, _| value_kind(key))
}

/// The style a value takes from the key that introduces it. A key splash has
/// no field for leaves its value in the default style for that value's type.
pub fn value_kind(key: &str) -> Option<TokenKind> {
    match key {
        "level" => Some(TokenKind::Level),
        "ts" | "time" => Some(TokenKind::Timestamp),
        "logger" => Some(TokenKind::Tag),
        "msg" | "message" | "error" => Some(TokenKind::Message),
        "remote_ip" | "client_ip" => Some(TokenKind::Ip),
        "proto" => Some(TokenKind::Protocol),
        "method" => Some(TokenKind::Method),
        "host" | "server_name" => Some(TokenKind::Host),
        "uri" => Some(TokenKind::Request),
        "status" => Some(TokenKind::Status),
        "size" | "bytes_read" => Some(TokenKind::Size),
        "duration" => Some(TokenKind::Duration),
        "user_id" => Some(TokenKind::UserId),
        _ => None,
    }
}
