//! Pieces shared by the mail server plugins
//!
//! Postfix, Exim, Dovecot, and Fetchmail all write their messages as free
//! text sprinkled with `key=value` fields and mail addresses. The fields and
//! the addresses are read here, and each plugin supplies the words that say
//! whether its mail went through.
use crate::output::{Token, TokenKind};
use crate::parser::push_message;
use regex::Regex;
use std::sync::LazyLock;

/// A `key=value` field, whose value may be quoted or wrapped in angle
/// brackets, or a mail address standing on its own
static FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        ([A-Za-z][\w-]*)=("[^"]*"|<[^>]*>|[^\s,]*)   # key and value
        |
        <([^<>\s]*)>                                # address in angle brackets
        |
        ([\w.+-]+@[\w-]+(?:\.[\w-]+)*)              # bare address
        "#,
    )
    .unwrap()
});

/// A host name followed by its address in brackets, such as
/// `mx.example.org[93.184.216.34]:25`
static HOST_ADDRESS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([^\[]*)\[([^\]]*)\](.*)$").unwrap());

/// Words a plugin colors by what they say about the mail, such as `flushed`
/// or `auth failed`
pub struct Words {
    pattern: Regex,
    kinds: &'static [(&'static str, TokenKind)],
}

impl Words {
    /// Builds a matcher for the given words. A longer word is tried before a
    /// shorter one it contains, so `not flushed` wins over `flushed`.
    pub fn new(kinds: &'static [(&'static str, TokenKind)]) -> Self {
        let mut words: Vec<&str> = kinds.iter().map(|(word, _)| *word).collect();
        words.sort_by_key(|word| std::cmp::Reverse(word.len()));

        let alternatives: Vec<String> = words.iter().map(|word| regex::escape(word)).collect();

        Self {
            pattern: Regex::new(&format!(r"\b(?:{})\b", alternatives.join("|"))).unwrap(),
            kinds,
        }
    }

    /// Whether any of the words appear in the text
    pub fn is_match(&self, text: &str) -> bool {
        self.pattern.is_match(text)
    }

    /// The kind given to a matched word
    fn kind(&self, word: &str) -> TokenKind {
        self.kinds
            .iter()
            .find(|(known, _)| *known == word)
            .map(|(_, kind)| *kind)
            .unwrap()
    }
}

/// Colors free text holding `key=value` fields and mail addresses. The text
/// between them is message text, with the plugin's words and any addresses
/// colored.
pub fn push_fields<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str, words: &Words) {
    let mut cursor = 0;

    for caps in FIELD.captures_iter(text) {
        let whole = caps.get(0).unwrap();

        push_words(tokens, &text[cursor..whole.start()], words);

        if let Some(key) = caps.get(1) {
            tokens.push(Token::new(key.as_str(), TokenKind::Header));
            tokens.push(Token::new("=", TokenKind::Punctuation));
            push_value(tokens, key.as_str(), caps.get(2).unwrap().as_str());
        } else if let Some(address) = caps.get(3) {
            push_wrapped(
                tokens,
                "<",
                address.as_str(),
                ">",
                FieldValue::Kind(TokenKind::Email),
            );
        } else {
            tokens.push(Token::new(whole.as_str(), TokenKind::Email));
        }

        cursor = whole.end();
    }

    push_words(tokens, &text[cursor..], words);
}

/// Colors the plugin's words inside message text, leaving the rest to
/// `push_message`.
pub fn push_words<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str, words: &Words) {
    let mut cursor = 0;

    for found in words.pattern.find_iter(text) {
        push_message(tokens, &text[cursor..found.start()]);
        tokens.push(Token::new(found.as_str(), words.kind(found.as_str())));
        cursor = found.end();
    }

    push_message(tokens, &text[cursor..]);
}

/// How the value of a `key=value` field is colored
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldValue {
    /// One token of this kind
    Kind(TokenKind),
    /// A host name, with its bracketed address colored separately
    Host,
    /// A delivery status, colored by whether the mail went through
    Outcome,
    /// Message text, with any addresses in it colored
    Message,
}

/// How the value of the field named `key` is colored. Postfix, Exim, and
/// Dovecot name their fields differently, and none of the names clash.
pub fn field_value(key: &str) -> FieldValue {
    match key {
        "from" | "to" | "orig_to" | "F" | "sender" | "recipient" => {
            FieldValue::Kind(TokenKind::Email)
        }
        "relay" | "client" | "helo" | "H" | "host" => FieldValue::Host,
        "status" => FieldValue::Outcome,
        "size" | "S" | "in" | "out" => FieldValue::Kind(TokenKind::Size),
        "delay" | "DT" | "QT" => FieldValue::Kind(TokenKind::Duration),
        "delays" => FieldValue::Kind(TokenKind::Timers),
        "dsn" => FieldValue::Kind(TokenKind::Status),
        "proto" | "P" | "X" | "method" | "sasl_method" => FieldValue::Kind(TokenKind::Protocol),
        "user" | "U" | "uid" | "sasl_username" => FieldValue::Kind(TokenKind::UserId),
        "message-id" | "msgid" | "id" | "session" => FieldValue::Kind(TokenKind::Transaction),
        "pid" | "mpid" => FieldValue::Kind(TokenKind::Pid),
        "rip" | "lip" => FieldValue::Kind(TokenKind::Ip),
        "R" | "T" => FieldValue::Kind(TokenKind::Module),
        "nrcpt" => FieldValue::Kind(TokenKind::Number),
        _ => FieldValue::Message,
    }
}

/// The kind a delivery status is colored in: `sent` went through, `deferred`
/// will be retried, and `bounced` will not be.
pub fn outcome_kind(status: &str) -> TokenKind {
    match status {
        "sent" | "delivered" | "deliverable" => TokenKind::Success,
        "deferred" => TokenKind::Warning,
        "bounced" | "expired" | "undeliverable" | "failed" => TokenKind::Failure,
        _ => TokenKind::Message,
    }
}

/// Colors the value of a `key=value` field, keeping any quotes or angle
/// brackets around it as punctuation.
fn push_value<'a>(tokens: &mut Vec<Token<'a>>, key: &str, value: &'a str) {
    let style = field_value(key);

    for (open, close) in [("\"", "\""), ("<", ">")] {
        if value.len() >= 2 && value.starts_with(open) && value.ends_with(close) {
            push_wrapped(tokens, open, &value[1..value.len() - 1], close, style);
            return;
        }
    }

    push_bare_value(tokens, value, style);
}

fn push_wrapped<'a>(
    tokens: &mut Vec<Token<'a>>,
    open: &'static str,
    inner: &'a str,
    close: &'static str,
    style: FieldValue,
) {
    tokens.push(Token::new(open, TokenKind::Punctuation));
    push_bare_value(tokens, inner, style);
    tokens.push(Token::new(close, TokenKind::Punctuation));
}

fn push_bare_value<'a>(tokens: &mut Vec<Token<'a>>, value: &'a str, style: FieldValue) {
    if value.is_empty() {
        return;
    }

    match style {
        FieldValue::Kind(kind) => tokens.push(Token::new(value, kind)),
        FieldValue::Host => push_host(tokens, value),
        FieldValue::Outcome => tokens.push(Token::new(value, outcome_kind(value))),
        FieldValue::Message => push_message(tokens, value),
    }
}

/// Colors a host name and the bracketed address after it, as in
/// `unknown[10.0.0.5]` or `mx.example.org[93.184.216.34]:25`.
fn push_host<'a>(tokens: &mut Vec<Token<'a>>, value: &'a str) {
    let Some(caps) = HOST_ADDRESS.captures(value) else {
        tokens.push(Token::new(value, TokenKind::Host));
        return;
    };

    let field = |index: usize| caps.get(index).unwrap().as_str();

    if !field(1).is_empty() {
        tokens.push(Token::new(field(1), TokenKind::Host));
    }

    tokens.extend([
        Token::new("[", TokenKind::Punctuation),
        Token::new(field(2), TokenKind::Ip),
        Token::new("]", TokenKind::Punctuation),
    ]);

    if !field(3).is_empty() {
        tokens.push(Token::new(field(3), TokenKind::Plain));
    }
}
