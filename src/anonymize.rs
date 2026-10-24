//! Masking IP addresses, email addresses, and user names
//!
//! Each value is replaced by a pseudonym made from a keyed hash of it, so the
//! same address gets the same mask throughout a run and lines can still be
//! compared, while the key is chosen at random for each run so a mask cannot
//! be matched to its value from another run.
use crate::output::{ParsedLine, Token, TokenKind};
use regex::Regex;
use std::borrow::Cow;
use std::collections::hash_map::{DefaultHasher, RandomState};
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::LazyLock;

/// An IPv4 address or an email address inside other text
static SENSITIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        (?P<email>[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,})
        |
        (?P<ip>\b(?:\d{1,3}\.){3}\d{1,3}\b)
        ",
    )
    .unwrap()
});

/// The value logs write for a missing user or address, which is kept
const MISSING: &str = "-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anonymizer {
    key: u64,
}

impl Default for Anonymizer {
    fn default() -> Self {
        Anonymizer::new()
    }
}

impl Anonymizer {
    /// An anonymizer with a key chosen at random
    pub fn new() -> Anonymizer {
        Anonymizer::with_key(RandomState::new().hash_one(0u8))
    }

    /// An anonymizer with a given key, whose masks repeat from run to run
    pub fn with_key(key: u64) -> Anonymizer {
        Anonymizer { key }
    }

    /// The line with every address and user name masked
    pub fn apply<'a>(&self, parsed: ParsedLine<'a>) -> ParsedLine<'a> {
        let tokens = parsed
            .tokens
            .into_iter()
            .map(|token| self.mask_token(token))
            .collect();

        ParsedLine::new(tokens)
    }

    fn mask_token<'a>(&self, token: Token<'a>) -> Token<'a> {
        let prefix = match token.kind {
            TokenKind::Ip | TokenKind::Client => "ip",
            TokenKind::Email => "email",
            TokenKind::UserId | TokenKind::UnknownUser => "user",
            _ => return self.mask_inside(token),
        };

        if token.text == MISSING {
            return token;
        }

        Token::owned(self.mask(prefix, &token.text), token.kind)
    }

    /// Masks the IPv4 and email addresses inside a token's text
    fn mask_inside<'a>(&self, token: Token<'a>) -> Token<'a> {
        let masked = SENSITIVE.replace_all(&token.text, |caps: &regex::Captures| {
            match caps.name("email") {
                Some(email) => self.mask("email", email.as_str()),
                None => self.mask("ip", &caps["ip"]),
            }
        });

        match masked {
            Cow::Borrowed(_) => token,
            Cow::Owned(text) => Token::owned(text, token.kind),
        }
    }

    /// The pseudonym for a value, as `PREFIX-` and eight hex digits
    pub fn mask(&self, prefix: &str, value: &str) -> String {
        let mut hasher = DefaultHasher::new();

        self.key.hash(&mut hasher);
        value.hash(&mut hasher);

        format!("{}-{:08x}", prefix, hasher.finish() as u32)
    }
}
