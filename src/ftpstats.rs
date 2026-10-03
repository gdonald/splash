//! ftpstats parsing
//!
//! The ftpstats format is pure-ftpd's `stats` transfer log. Each transfer is
//! one line: the Unix time, a session id made of the session start time and
//! process id in hexadecimal, the user, the remote host, `U` for an upload or
//! `D` for a download, the size, the transfer time in seconds, and the file.
use crate::ftp;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "ftpstats";

static TRANSFER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{9,10})              # Unix time
        \ ([\da-f]+\.[\da-f]+)  # session id
        \ (\S+)                 # user
        \ (\S+)                 # remote host
        \ ([UD])                # direction
        \ (\d+)                 # size
        \ (\d+)                 # transfer time
        \ (.*)                  # file
        $
        ",
    )
    .unwrap()
});

/// The ftpstats plugin
pub struct FtpstatsPlugin {
    metadata: PluginMetadata,
}

impl FtpstatsPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "pure-ftpd ftpstats transfer logs",
                "splash",
            ),
        }
    }
}

impl Default for FtpstatsPlugin {
    fn default() -> Self {
        FtpstatsPlugin::new()
    }
}

impl Plugin for FtpstatsPlugin {
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

/// Parses one ftpstats line, or returns `None` when the line has a different
/// shape.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = TRANSFER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let kinds = [
        TokenKind::Timestamp,
        TokenKind::Transaction,
        TokenKind::UserId,
        ftp::host_kind(field(4)),
        TokenKind::Method,
        TokenKind::Size,
        TokenKind::Duration,
        TokenKind::Path,
    ];

    let mut tokens = Vec::new();

    for (index, kind) in kinds.into_iter().enumerate() {
        if index > 0 {
            tokens.push(Token::new(" ", TokenKind::Plain));
        }

        let text = field(index + 1);

        if !text.is_empty() {
            tokens.push(Token::new(text, kind));
        }
    }

    Some(ParsedLine::new(tokens))
}
