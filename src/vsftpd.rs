//! vsftpd log parsing
//!
//! vsftpd's own log format, written to `vsftpd.log` or to syslog, reports
//! each event as `[user] OK DOWNLOAD: Client "10.0.0.5"` followed by the
//! file, the bytes moved, and the rate. In its log file each line opens with
//! the date and `[pid N]`, and through syslog the syslog header takes their
//! place.
use crate::ftp;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::parser::push_message;
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "vsftpd";

/// The date and process id that open a line of vsftpd's log file
static FILE_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^({}) (\[)(pid) (\d+)(\]) ", syslog::CTIME)).unwrap());

/// An event: the user, whether it succeeded, what it was, and the client
static EVENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        ^
        (?:\[([^\]]*)\]\ )?     # user
        (?:(OK|FAIL)\ )?        # outcome
        (DOWNLOAD|UPLOAD|MKDIR|LOGIN|FTP\ command|FTP\ response|CONNECT|DELETE|RENAME|RMDIR|CHMOD|DEBUG)
        :\ (Client)\ "([^"]*)"  # client
        (.*)
        $
        "#,
    )
    .unwrap()
});

/// The details after the client: an anonymous password, a quoted file or
/// command, a byte count, or a rate
static DETAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        ,\x20
        (?:
            (anon\ password)\ "([^"]*)"
            |
            "([^"]*)"
            |
            (\d+)(\ bytes)
            |
            (\d+\.\d+)(Kbyte/sec)
        )
        "#,
    )
    .unwrap()
});

/// A reply code at the start of an `FTP response`
static REPLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d{3})(.*)$").unwrap());

/// The vsftpd log plugin
pub struct VsftpdPlugin {
    metadata: PluginMetadata,
}

impl VsftpdPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "vsftpd FTP server logs",
                "splash",
            ),
        }
    }
}

impl Default for VsftpdPlugin {
    fn default() -> Self {
        VsftpdPlugin::new()
    }
}

impl Plugin for VsftpdPlugin {
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

/// Parses one vsftpd log line, from its log file or from syslog, or returns
/// `None` when the line is neither.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let (mut tokens, body) = split_file_prefix(line).or_else(|| split_syslog_header(line))?;

    push_body(&mut tokens, body);

    Some(ParsedLine::new(tokens))
}

/// Splits the date and process id off a line from vsftpd's log file.
fn split_file_prefix(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let caps = FILE_PREFIX.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let tokens = vec![
        Token::new(field(1), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::Punctuation),
        Token::new(field(3), TokenKind::Header),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4), TokenKind::Pid),
        Token::new(field(5), TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
    ];

    Some((tokens, &line[caps.get(0).unwrap().end()..]))
}

/// Splits the syslog header off a line vsftpd sent to syslog.
fn split_syslog_header(line: &str) -> Option<(Vec<Token<'_>>, &str)> {
    let header = syslog::split_header(line)?;

    if header.program != NAME {
        return None;
    }

    Some((header.tokens, header.body))
}

/// Colors an event, or colors the text as a message when it is not one.
fn push_body<'a>(tokens: &mut Vec<Token<'a>>, body: &'a str) {
    let Some(caps) = EVENT.captures(body) else {
        push_message(tokens, body);
        return;
    };

    let field = |index: usize| caps.get(index).map(|found| found.as_str());

    if let Some(user) = field(1) {
        tokens.push(Token::new("[", TokenKind::Punctuation));

        if !user.is_empty() {
            tokens.push(Token::new(user, TokenKind::UserId));
        }

        tokens.extend([
            Token::new("]", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);
    }

    if let Some(outcome) = field(2) {
        let kind = if outcome == "OK" {
            TokenKind::Success
        } else {
            TokenKind::Failure
        };

        tokens.extend([Token::new(outcome, kind), Token::new(" ", TokenKind::Plain)]);
    }

    let action = field(3).unwrap();
    let client = field(5).unwrap();

    tokens.extend([
        Token::new(action, TokenKind::Method),
        Token::new(":", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4).unwrap(), TokenKind::Header),
        Token::new(" ", TokenKind::Plain),
        Token::new("\"", TokenKind::Punctuation),
    ]);

    if !client.is_empty() {
        tokens.push(Token::new(client, ftp::host_kind(client)));
    }

    tokens.push(Token::new("\"", TokenKind::Punctuation));

    push_details(tokens, field(6).unwrap(), action);
}

/// Colors the details after the client. A quoted value is read by what the
/// event was: a command, a reply, a file, or a message.
fn push_details<'a>(tokens: &mut Vec<Token<'a>>, details: &'a str, action: &str) {
    let mut cursor = 0;

    for caps in DETAIL.captures_iter(details) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        push_message(tokens, &details[cursor..whole.start()]);

        tokens.extend([
            Token::new(",", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]);

        if let Some(label) = field(1) {
            tokens.extend([
                Token::new(label, TokenKind::Header),
                Token::new(" ", TokenKind::Plain),
            ]);
            push_quoted(tokens, field(2).unwrap(), |tokens, text| {
                tokens.push(Token::new(text, TokenKind::UserIdentifier))
            });
        } else if let Some(quoted) = field(3) {
            push_quoted(tokens, quoted, |tokens, text| {
                push_quoted_value(tokens, text, action)
            });
        } else if let Some(bytes) = field(4) {
            tokens.extend([
                Token::new(bytes, TokenKind::Size),
                Token::new(field(5).unwrap(), TokenKind::Message),
            ]);
        } else {
            tokens.extend([
                Token::new(field(6).unwrap(), TokenKind::Number),
                Token::new(field(7).unwrap(), TokenKind::Message),
            ]);
        }

        cursor = whole.end();
    }

    push_message(tokens, &details[cursor..]);
}

/// Pushes a quoted value between quote marks, leaving an empty one as only
/// the quote marks.
fn push_quoted<'a>(
    tokens: &mut Vec<Token<'a>>,
    text: &'a str,
    push_inner: impl FnOnce(&mut Vec<Token<'a>>, &'a str),
) {
    tokens.push(Token::new("\"", TokenKind::Punctuation));

    if !text.is_empty() {
        push_inner(tokens, text);
    }

    tokens.push(Token::new("\"", TokenKind::Punctuation));
}

fn push_quoted_value<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str, action: &str) {
    match action {
        "FTP command" => tokens.push(Token::new(text, TokenKind::Request)),
        "FTP response" => push_reply(tokens, text),
        "LOGIN" | "CONNECT" | "DEBUG" => push_message(tokens, text),
        _ => tokens.push(Token::new(text, TokenKind::Path)),
    }
}

/// Colors a reply as its code and its text.
fn push_reply<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    let Some(caps) = REPLY.captures(text) else {
        push_message(tokens, text);
        return;
    };

    tokens.push(Token::new(caps.get(1).unwrap().as_str(), TokenKind::Status));
    push_message(tokens, caps.get(2).unwrap().as_str());
}
