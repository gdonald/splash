//! Apache error log parsing
//!
//! Apache 2.4's error log opens each line with bracketed fields: the date,
//! the module and level, the process and thread, and the client, as in
//! `[Wed Oct 11 14:32:52.123456 2023] [core:error] [pid 35708:tid 4328636416]
//! [client 72.15.99.187:1234] AH00128: File does not exist: /var/www/x`.
//! Apache 2.2 writes the level alone and no process. The httpd mode reads
//! these lines among access lines; this mode reads only error lines and
//! colors their fields one by one, along with the PHP messages mod_php and
//! proxy_fcgi pass through.
use crate::ftp;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::php;
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::net::IpAddr;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "apache-error";

static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        \[([^\]]*)\]                        # date
        \ \[(?:([^:\]]+):)?([a-z0-9]+)\]    # module and level
        (?:\ \[pid\ (\d+)(?::tid\ (\d+))?\])?   # process and thread
        (?:\ \[client\ ([^\]]*)\])?         # client
        (.*)                                # message, with its leading space
        $
        ",
    )
    .unwrap()
});

/// The error code a message opens with, such as `AH00128`
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(AH\d{5})(:)").unwrap());

/// The Apache error log plugin
pub struct ApacheErrorPlugin {
    metadata: PluginMetadata,
}

impl ApacheErrorPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Apache error logs",
                "splash",
            ),
        }
    }
}

impl Default for ApacheErrorPlugin {
    fn default() -> Self {
        ApacheErrorPlugin::new()
    }
}

impl Plugin for ApacheErrorPlugin {
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

/// The kind an Apache level is colored in
fn level_kind(level: &str) -> TokenKind {
    match level {
        "emerg" | "alert" | "crit" | "error" => TokenKind::Failure,
        "warn" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}

/// Parses one Apache error log line, or returns `None` when the line is not
/// one.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LINE.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());

    let mut tokens = vec![
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(1).unwrap(), TokenKind::Timestamp),
        Token::new("]", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new("[", TokenKind::Punctuation),
    ];

    if let Some(module) = field(2) {
        tokens.extend([
            Token::new(module, TokenKind::Module),
            Token::new(":", TokenKind::Punctuation),
        ]);
    }

    let level = field(3).unwrap();

    tokens.extend([
        Token::new(level, level_kind(level)),
        Token::new("]", TokenKind::Punctuation),
    ]);

    if let Some(pid) = field(4) {
        tokens.extend([
            Token::new(" ", TokenKind::Plain),
            Token::new("[", TokenKind::Punctuation),
            Token::new("pid", TokenKind::Header),
            Token::new(" ", TokenKind::Plain),
            Token::new(pid, TokenKind::Pid),
        ]);

        if let Some(thread) = field(5) {
            tokens.extend([
                Token::new(":", TokenKind::Punctuation),
                Token::new("tid", TokenKind::Header),
                Token::new(" ", TokenKind::Plain),
                Token::new(thread, TokenKind::Pid),
            ]);
        }

        tokens.push(Token::new("]", TokenKind::Punctuation));
    }

    if let Some(client) = field(6) {
        tokens.extend([
            Token::new(" ", TokenKind::Plain),
            Token::new("[", TokenKind::Punctuation),
            Token::new("client", TokenKind::Header),
            Token::new(" ", TokenKind::Plain),
        ]);
        push_client(&mut tokens, client);
        tokens.push(Token::new("]", TokenKind::Punctuation));
    }

    push_message(&mut tokens, field(7).unwrap());

    Some(ParsedLine::new(tokens))
}

/// Colors a client address and, when it has one, the port after its last
/// colon. An IPv6 address keeps its own colons.
fn push_client<'a>(tokens: &mut Vec<Token<'a>>, client: &'a str) {
    let with_port = client.rsplit_once(':').filter(|(address, port)| {
        address.parse::<IpAddr>().is_ok()
            && !port.is_empty()
            && port.bytes().all(|byte| byte.is_ascii_digit())
    });

    match with_port {
        Some((address, port)) => tokens.extend([
            Token::new(address, TokenKind::Ip),
            Token::new(":", TokenKind::Punctuation),
            Token::new(port, TokenKind::Number),
        ]),
        None if client.is_empty() => {}
        None => tokens.push(Token::new(client, ftp::host_kind(client))),
    }
}

/// Colors the message after the fields: its error code, then the text as a
/// PHP message, which colors files, lines, and any PHP error type in it.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, rest: &'a str) {
    let Some(message) = rest.strip_prefix(' ') else {
        php::push_message(tokens, rest);
        return;
    };

    tokens.push(Token::new(" ", TokenKind::Plain));

    match CODE.captures(message) {
        Some(caps) => {
            tokens.extend([
                Token::new(caps.get(1).unwrap().as_str(), TokenKind::Transaction),
                Token::new(caps.get(2).unwrap().as_str(), TokenKind::Punctuation),
            ]);
            php::push_message(tokens, &message[caps.get(0).unwrap().end()..]);
        }
        None => php::push_message(tokens, message),
    }
}
