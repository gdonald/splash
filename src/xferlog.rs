//! xferlog parsing
//!
//! The xferlog format began with wu-ftpd and is written by vsftpd, ProFTPD,
//! and pure-ftpd as well. Each transfer is one line of space separated
//! fields: the date, the transfer time in seconds, the remote host, the size,
//! the file name, the transfer type, a special action flag, the direction,
//! the access mode, the user, the service, the authentication method, the
//! authenticated user id, and whether the transfer completed.
use crate::ftp;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "xferlog";

/// One transfer. The file name is the only field that may hold a space, so
/// it is read as whatever lies between the fixed fields on either side.
static TRANSFER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?x)
        ^
        ({})            # date
        \ (\d+)         # transfer time
        \ (\S+)         # remote host
        \ (\d+)         # size
        \ (.+)          # file name
        \ ([ab])        # transfer type
        \ (\S+)         # special action flag
        \ ([oid])       # direction
        \ ([agr])       # access mode
        \ (\S+)         # user
        \ (\S+)         # service
        \ ([01])        # authentication method
        \ (\S+)         # authenticated user id
        \ ([ci])        # completion status
        $
        ",
        syslog::CTIME
    ))
    .unwrap()
});

/// The xferlog plugin
pub struct XferlogPlugin {
    metadata: PluginMetadata,
}

impl XferlogPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Generic xferlog FTP transfer logs",
                "splash",
            ),
        }
    }
}

impl Default for XferlogPlugin {
    fn default() -> Self {
        XferlogPlugin::new()
    }
}

impl Plugin for XferlogPlugin {
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

/// The kind a completion status is colored in: `c` finished and `i` did not
fn completion_kind(status: &str) -> TokenKind {
    if status == "c" {
        TokenKind::Success
    } else {
        TokenKind::Failure
    }
}

/// Parses one xferlog line, or returns `None` when the line has a different
/// shape.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = TRANSFER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let kinds = [
        TokenKind::Timestamp,
        TokenKind::Duration,
        ftp::host_kind(field(3)),
        TokenKind::Size,
        TokenKind::Path,
        TokenKind::Protocol,
        TokenKind::Tag,
        TokenKind::Method,
        TokenKind::Level,
        TokenKind::UserId,
        TokenKind::Module,
        TokenKind::Number,
        TokenKind::UserIdentifier,
        completion_kind(field(14)),
    ];

    let mut tokens = Vec::new();

    for (index, kind) in kinds.into_iter().enumerate() {
        if index > 0 {
            tokens.push(Token::new(" ", TokenKind::Plain));
        }

        tokens.push(Token::new(field(index + 1), kind));
    }

    Some(ParsedLine::new(tokens))
}
