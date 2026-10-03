//! Pieces shared by the FTP server plugins
//!
//! ProFTPD's `ExtendedLog` and pure-ftpd's `clf` transfer log both write one
//! line per request in the Common Log Format shape
//! `host - user [date] "request" status size`. That shape is read here, along
//! with the choice between coloring a remote host as a name or an address.
use crate::output::{ParsedLine, Token, TokenKind};
use regex::Regex;
use std::net::IpAddr;
use std::sync::LazyLock;

/// A transfer log line in the Common Log Format shape, such as
/// `10.0.0.5 - alice [03/Oct/2023:12:00:01 +0000] "RETR report.pdf" 226 4096`
static TRANSFER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        ^
        (\S+)               # remote host
        \ (\S+)             # remote login name
        \ (\S+)             # user
        \ (\[[^\]]*\])      # date
        \ "(\S+)(?:\ ([^"]*))?"   # command and its argument
        \ (\d{3}|-)         # status
        \ (\d+|-)           # size
        $
        "#,
    )
    .unwrap()
});

/// The kind a remote host is colored in: `ip` for an IPv4 or IPv6 address,
/// `host` for anything else
pub fn host_kind(host: &str) -> TokenKind {
    if host.parse::<IpAddr>().is_ok() {
        TokenKind::Ip
    } else {
        TokenKind::Host
    }
}

/// Parses a transfer log line in the Common Log Format shape, or returns
/// `None` when the line has a different shape.
pub fn parse_transfer_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = TRANSFER.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let mut tokens = vec![
        Token::new(field(1), host_kind(field(1))),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::UserIdentifier),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::UserId),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(field(5), TokenKind::Method),
    ];

    if let Some(argument) = caps.get(6) {
        tokens.push(Token::new(" ", TokenKind::Plain));

        if !argument.as_str().is_empty() {
            tokens.push(Token::new(argument.as_str(), TokenKind::Request));
        }
    }

    tokens.extend([
        Token::new("\"", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(7), TokenKind::Status),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(8), TokenKind::Size),
    ]);

    Some(ParsedLine::new(tokens))
}
