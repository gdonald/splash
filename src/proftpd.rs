//! ProFTPD log parsing
//!
//! ProFTPD writes three kinds of line splash reads here. Its `SystemLog`
//! opens each line with a date carrying milliseconds, an optional host,
//! `proftpd[pid]`, and the server name with the client in parentheses, as in
//! `ftp.example.com (client.example.org[10.0.0.5]): `. Through syslog the
//! same server and client come after the syslog header, followed by ` - `.
//! Its `ExtendedLog`, in the default format, writes one Common Log Format
//! shaped line per command.
use crate::ftp;
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "proftpd";

/// The prefix of a `SystemLog` line, up to the message
static SYSTEM_LOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{4}-\d{2}-\d{2}\ \d{2}:\d{2}:\d{2},\d{3})   # date
        \ (?:(\S+)\ )?                                 # host
        (proftpd|daemon|session)                       # process label
        \[(\d+)\]                                      # process id
        (?:\ ([^\s(:]+)(?:(\ ?)\(([^\[]*)\[([^\]]*)\]\))?)?   # server and client
        :\x20
        ",
    )
    .unwrap()
});

/// The server and client that open the body of a syslog line
static SYSLOG_SERVER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        ([^\s(]+)                               # server
        (?:(\ ?)\(([^\[]*)\[([^\]]*)\]\))?      # client
        \ -\x20
        ",
    )
    .unwrap()
});

/// The user a `USER` or `ANON` message is about
static USER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(USER|ANON) ([^\s:]+)").unwrap());

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Login successful", TokenKind::Success),
        ("Authenticated without password", TokenKind::Success),
        ("Login failed", TokenKind::Failure),
        ("Incorrect password", TokenKind::Failure),
        ("No such user found", TokenKind::Failure),
        ("no such user found", TokenKind::Failure),
        ("Password expired", TokenKind::Failure),
        ("Account disabled", TokenKind::Failure),
        ("Limit access denies login", TokenKind::Failure),
    ])
});

/// The ProFTPD log plugin
pub struct ProftpdPlugin {
    metadata: PluginMetadata,
}

impl ProftpdPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "ProFTPD system and extended logs",
                "splash",
            ),
        }
    }
}

impl Default for ProftpdPlugin {
    fn default() -> Self {
        ProftpdPlugin::new()
    }
}

impl Plugin for ProftpdPlugin {
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

/// Parses one ProFTPD log line, from the `SystemLog`, syslog, or the
/// `ExtendedLog`, or returns `None` when the line is none of them.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_system_log_line(line)
        .or_else(|| parse_syslog_line(line))
        .or_else(|| ftp::parse_transfer_line(line))
}

/// Parses a line from ProFTPD's `SystemLog` file.
pub fn parse_system_log_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = SYSTEM_LOG.captures(line)?;
    let field = |index: usize| caps.get(index).map(|found| found.as_str());

    let mut tokens = vec![
        Token::new(field(1).unwrap(), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
    ];

    if let Some(host) = field(2) {
        tokens.extend([
            Token::new(host, TokenKind::Host),
            Token::new(" ", TokenKind::Plain),
        ]);
    }

    tokens.extend([
        Token::new(field(3).unwrap(), TokenKind::Tag),
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(4).unwrap(), TokenKind::Pid),
        Token::new("]", TokenKind::Punctuation),
    ]);

    if let Some(server) = field(5) {
        tokens.push(Token::new(" ", TokenKind::Plain));
        push_server(&mut tokens, server, client(&caps, 6));
    }

    tokens.extend([
        Token::new(":", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ]);

    push_message(&mut tokens, &line[caps.get(0).unwrap().end()..]);

    Some(ParsedLine::new(tokens))
}

/// Parses a line ProFTPD sent to syslog.
pub fn parse_syslog_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if header.program != NAME {
        return None;
    }

    let mut tokens = header.tokens;
    let mut body = header.body;

    if let Some(caps) = SYSLOG_SERVER.captures(body) {
        push_server(&mut tokens, caps.get(1).unwrap().as_str(), client(&caps, 2));

        tokens.extend([
            Token::new(" ", TokenKind::Plain),
            Token::new("-", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);

        body = &body[caps.get(0).unwrap().end()..];
    }

    push_message(&mut tokens, body);

    Some(ParsedLine::new(tokens))
}

/// The client in parentheses after a server name: the gap before it, its
/// name, and its address
struct Client<'a> {
    gap: &'a str,
    name: &'a str,
    address: &'a str,
}

/// Reads the client from the three groups starting at `first`, or returns
/// `None` when the line names no client.
fn client<'a>(caps: &regex::Captures<'a>, first: usize) -> Option<Client<'a>> {
    let gap = caps.get(first)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    Some(Client {
        gap: gap.as_str(),
        name: field(first + 1),
        address: field(first + 2),
    })
}

/// Colors the server name and, when there is one, the client's name and
/// address in parentheses after it.
fn push_server<'a>(tokens: &mut Vec<Token<'a>>, server: &'a str, client: Option<Client<'a>>) {
    tokens.push(Token::new(server, TokenKind::VirtualHost));

    let Some(client) = client else {
        return;
    };

    if !client.gap.is_empty() {
        tokens.push(Token::new(client.gap, TokenKind::Plain));
    }

    tokens.push(Token::new("(", TokenKind::Punctuation));

    if !client.name.is_empty() {
        tokens.push(Token::new(client.name, ftp::host_kind(client.name)));
    }

    tokens.push(Token::new("[", TokenKind::Punctuation));

    if !client.address.is_empty() {
        tokens.push(Token::new(client.address, ftp::host_kind(client.address)));
    }

    tokens.extend([
        Token::new("]", TokenKind::Punctuation),
        Token::new(")", TokenKind::Punctuation),
    ]);
}

/// Colors a message, with the user a `USER` or `ANON` message is about.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in USER.captures_iter(message) {
        let whole = caps.get(0).unwrap();

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        tokens.extend([
            Token::new(caps.get(1).unwrap().as_str(), TokenKind::Method),
            Token::new(" ", TokenKind::Plain),
            Token::new(caps.get(2).unwrap().as_str(), TokenKind::UserId),
        ]);

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
