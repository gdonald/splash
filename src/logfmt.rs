//! logfmt pair scanning
//!
//! dockerd, containerd, Kubernetes components, and Terraform write
//! `key=value` pairs, with a value that holds spaces wrapped in double
//! quotes, as in `time="2023-10-03T12:00:01Z" level=info msg="Starting up"`.
//! The pairs are read here, and each plugin says how a value is colored from
//! its key and its text.
use crate::output::{Token, TokenKind};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// How a plugin colors a value, from its key and its text. `None` leaves the
/// value as message text.
pub type ValueKind = fn(key: &str, value: &str) -> Option<TokenKind>;

/// One `key=value` pair, with a quoted or a bare value
static PAIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(@?[A-Za-z_][\w.@-]*)(=)(?:(")((?:[^"\\]|\\.)*)(")|([^\s"]*))"#).unwrap()
});

/// Colors the `key=value` pairs in a text, coloring the text between them as
/// a message.
pub fn push_pairs<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str, value_kind: ValueKind) {
    let mut cursor = 0;

    for caps in PAIR.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());
        let key = field(1).unwrap();

        syslog::push_text(tokens, &text[cursor..whole.start()]);

        tokens.extend([
            Token::new(key, TokenKind::Header),
            Token::new(field(2).unwrap(), TokenKind::Punctuation),
        ]);

        let value = field(4).or(field(6)).unwrap();
        let kind = value_kind(key, value).unwrap_or(TokenKind::Message);

        if let Some(quote) = field(3) {
            tokens.push(Token::new(quote, TokenKind::Punctuation));
        }

        if !value.is_empty() {
            tokens.push(Token::new(value, kind));
        }

        if let Some(quote) = field(5) {
            tokens.push(Token::new(quote, TokenKind::Punctuation));
        }

        cursor = whole.end();
    }

    syslog::push_text(tokens, &text[cursor..]);
}

/// The `key=value` pairs in a text, each value without its quotes. A pair
/// must open the text or follow whitespace, so a query string such as
/// `/search?q=1` holds none.
pub fn pairs(text: &str) -> Vec<(&str, &str)> {
    PAIR.captures_iter(text)
        .filter(|caps| {
            let start = caps.get(0).unwrap().start();

            text[..start]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
        })
        .map(|caps| {
            let value = caps.get(4).or(caps.get(6)).unwrap().as_str();

            (caps.get(1).unwrap().as_str(), value)
        })
        .collect()
}

/// The kind a log level is colored in, for the level names Go loggers use:
/// `error`, `fatal`, and `panic` are failures, `warn` and `warning` are
/// warnings, and the rest are levels. Case does not matter.
pub fn level_kind(level: &str) -> TokenKind {
    match level.to_ascii_lowercase().as_str() {
        "error" | "fatal" | "panic" | "err" | "crit" | "critical" => TokenKind::Failure,
        "warn" | "warning" => TokenKind::Warning,
        _ => TokenKind::Level,
    }
}
