//! Apache and nginx log parsing
//!
//! One plugin covers the four shapes a web server log takes: the Combined and
//! vhost Combined access formats, the Common Log Format the `clf` mode also
//! reads, the Apache error log, and the nginx error log. Each line is tried
//! against them in that order and the first shape that fits wins.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::parser::{parse_clf_line, push_message};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "httpd";

/// The Combined Log Format: the Common Log Format followed by a quoted
/// referer and a quoted user agent, then any extended fields the server adds.
static COMBINED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^{}$", combined_pattern(""))).unwrap());

/// The vhost Combined format, which prefixes Combined with the server name
static VHOST_COMBINED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^{}$", combined_pattern(r"(\S+) "))).unwrap());

/// The Apache error log, whose leading fields are each bracketed
static APACHE_ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        \[([^\]]*)\]\                  # timestamp
        \[([^\]]*)\]                   # module and level
        (\ \[pid\ [^\]]*\])?           # pid and thread
        (\ \[client\ [^\]]*\])?        # client address
        (.*)                           # message, with its leading space
        $",
    )
    .unwrap()
});

/// The nginx error log, whose timestamp is unbracketed and slash separated
static NGINX_ERROR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^
        (\d{4}/\d{2}/\d{2}\ \d{2}:\d{2}:\d{2})  # timestamp
        \ \[([a-z]+)\]\                         # level
        (\d+\#\d+)                              # worker and thread
        :
        (.*)                                    # message, with its leading space
        $",
    )
    .unwrap()
});

/// A field made only of digits, which in an extended access log is the time
/// the server spent on the request
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());

fn combined_pattern(vhost: &str) -> String {
    format!(
        concat!(
            r"{}",
            r"(\d{{1,3}}(?:\.\d{{1,3}}){{3}}|[0-9a-fA-F:]+) ", // client
            r"(\S+) ",                                         // user identifier
            r"(\S+) ",                                         // userid
            r"(\[[^\]]*\]) ",                                  // timestamp
            r#""([A-Z]+) (\S+) (\S+)" "#,                      // method, request, protocol
            r"(\d{{3}}) ",                                     // status
            r"(\d+|-) ",                                       // size
            r#""([^"]*)" "#,                                   // referer
            r#""([^"]*)""#,                                    // user agent
            r"(.*)",                                           // extended fields
        ),
        vhost
    )
}

/// The Apache and nginx log plugin
pub struct HttpdPlugin {
    metadata: PluginMetadata,
}

impl HttpdPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Apache and nginx access and error logs",
                "splash",
            ),
        }
    }
}

impl Default for HttpdPlugin {
    fn default() -> Self {
        HttpdPlugin::new()
    }
}

impl Plugin for HttpdPlugin {
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

/// Parses one Apache or nginx log line, or returns `None` when the line is
/// none of the four formats.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    parse_combined_line(line)
        .or_else(|| parse_clf_line(line))
        .or_else(|| parse_apache_error_line(line))
        .or_else(|| parse_nginx_error_line(line))
}

/// Parses a Combined or vhost Combined access log line, including any
/// extended fields that follow the user agent.
pub fn parse_combined_line(line: &str) -> Option<ParsedLine<'_>> {
    let (caps, vhost) = match COMBINED.captures(line) {
        Some(caps) => (caps, false),
        None => (VHOST_COMBINED.captures(line)?, true),
    };

    let offset = usize::from(vhost);
    let field = |index: usize| caps.get(index + offset).unwrap().as_str();
    let mut tokens: Vec<Token> = Vec::new();

    if vhost {
        tokens.push(Token::new(
            caps.get(1).unwrap().as_str(),
            TokenKind::VirtualHost,
        ));
        tokens.push(Token::new(" ", TokenKind::Plain));
    }

    tokens.extend([
        Token::new(field(1), TokenKind::Client),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(2), TokenKind::UserIdentifier),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(3), TokenKind::UserId),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(4), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(field(5), TokenKind::Method),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(6), TokenKind::Request),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(7), TokenKind::Protocol),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(8), TokenKind::Status),
        Token::new(" ", TokenKind::Plain),
        Token::new(field(9), TokenKind::Size),
        Token::new(" ", TokenKind::Plain),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(field(10), TokenKind::Referer),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new("\"", TokenKind::Punctuation),
        Token::new(field(11), TokenKind::UserAgent),
        Token::new("\"", TokenKind::Punctuation),
    ]);

    push_extended_fields(&mut tokens, field(12));

    Some(ParsedLine::new(tokens))
}

/// Parses an Apache error log line.
pub fn parse_apache_error_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = APACHE_ERROR.captures(line)?;
    let mut tokens = vec![
        Token::new("[", TokenKind::Punctuation),
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Timestamp),
        Token::new("]", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new("[", TokenKind::Punctuation),
    ];

    push_module_and_level(&mut tokens, caps.get(2).unwrap().as_str());
    tokens.push(Token::new("]", TokenKind::Punctuation));

    if let Some(pid) = caps.get(3) {
        tokens.push(Token::new(" ", TokenKind::Plain));
        tokens.push(Token::new(pid.as_str().trim_start(), TokenKind::Pid));
    }

    if let Some(client) = caps.get(4) {
        tokens.push(Token::new(" ", TokenKind::Plain));
        tokens.push(Token::new(client.as_str().trim_start(), TokenKind::Client));
    }

    push_trailing_message(&mut tokens, caps.get(5).unwrap().as_str());

    Some(ParsedLine::new(tokens))
}

/// Parses an nginx error log line.
pub fn parse_nginx_error_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = NGINX_ERROR.captures(line)?;
    let mut tokens = vec![
        Token::new(caps.get(1).unwrap().as_str(), TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new("[", TokenKind::Punctuation),
        Token::new(caps.get(2).unwrap().as_str(), TokenKind::Level),
        Token::new("]", TokenKind::Punctuation),
        Token::new(" ", TokenKind::Plain),
        Token::new(caps.get(3).unwrap().as_str(), TokenKind::Pid),
        Token::new(":", TokenKind::Punctuation),
    ];

    push_trailing_message(&mut tokens, caps.get(4).unwrap().as_str());

    Some(ParsedLine::new(tokens))
}

/// Colors the message that closes an error log line, keeping the space that
/// separates it from the fields before it.
fn push_trailing_message<'a>(tokens: &mut Vec<Token<'a>>, rest: &'a str) {
    match rest.strip_prefix(' ') {
        Some(message) => {
            tokens.push(Token::new(&rest[..1], TokenKind::Plain));
            push_message(tokens, message);
        }
        None => push_message(tokens, rest),
    }
}

/// Splits an Apache `module:level` field, which older releases write as a
/// bare level with no module.
fn push_module_and_level<'a>(tokens: &mut Vec<Token<'a>>, field: &'a str) {
    match field.rfind(':') {
        Some(split) => {
            tokens.push(Token::new(&field[..split], TokenKind::Module));
            tokens.push(Token::new(":", TokenKind::Punctuation));
            tokens.push(Token::new(&field[split + 1..], TokenKind::Level));
        }
        None => tokens.push(Token::new(field, TokenKind::Level)),
    }
}

/// Colors the fields an extended access log adds after the user agent. A
/// field of digits alone is the time the server spent on the request.
fn push_extended_fields<'a>(tokens: &mut Vec<Token<'a>>, extra: &'a str) {
    let mut rest = extra;

    while !rest.is_empty() {
        let spaces = rest.len() - rest.trim_start().len();

        if spaces > 0 {
            tokens.push(Token::new(&rest[..spaces], TokenKind::Plain));
            rest = &rest[spaces..];
            continue;
        }

        let end = rest.find(' ').unwrap_or(rest.len());
        let field = &rest[..end];

        let kind = if DIGITS.is_match(field) {
            TokenKind::Duration
        } else {
            TokenKind::Plain
        };

        tokens.push(Token::new(field, kind));
        rest = &rest[end..];
    }
}
