//! Lines programs printed inside a container
//!
//! `docker logs` and `kubectl logs` print each line the container wrote,
//! in whatever format the program in it uses. Such a line is colored with
//! the mode that reads it on its own, and otherwise as free text.
use crate::detect;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The RFC 3339 time `--timestamps` puts before each line
static TIMESTAMPED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[-+]\d{2}:\d{2}))( )(.*)$")
        .unwrap()
});

/// Parses a line a container printed
pub fn parse_output(line: &str) -> ParsedLine<'_> {
    let mut tokens = Vec::new();

    push_output(&mut tokens, line);

    ParsedLine::new(tokens)
}

/// Colors a line a container printed with the mode that reads it on its
/// own, or as free text when no mode does
pub fn push_output<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    match detect::detect(&[text]).and_then(|detection| detect::parse_as(&detection.mode, text)) {
        Some(parsed) => tokens.extend(parsed.tokens),
        None => syslog::push_text(tokens, text),
    }
}

/// Colors the time `--timestamps` put before a line, then the line with
/// `push_rest`, or returns `None`, adding nothing, when the text does not
/// open with such a time
pub fn push_timestamped<'a>(
    tokens: &mut Vec<Token<'a>>,
    text: &'a str,
    push_rest: impl Fn(&mut Vec<Token<'a>>, &'a str),
) -> Option<()> {
    let caps = TIMESTAMPED.captures(text)?;

    tokens.push(Token::new(
        caps.get(1).unwrap().as_str(),
        TokenKind::Timestamp,
    ));
    tokens.push(Token::new(caps.get(2).unwrap().as_str(), TokenKind::Plain));
    push_rest(tokens, caps.get(3).unwrap().as_str());

    Some(())
}
