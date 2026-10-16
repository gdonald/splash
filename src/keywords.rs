//! Keyword highlighting
//!
//! Words such as `ERROR`, `TCP`, `FAILED`, `RESTART`, and `SELECT` are styled
//! wherever they appear in text a parser left plain or treated as a message.
//! Keywords match whole words in any case. A config file adds keywords under
//! `[keywords]` and regular expressions under `[patterns]`, each keyed by
//! the color key they are styled with.
use crate::output::{ParsedLine, Token, TokenKind};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;
use std::fmt;

/// The keywords splash styles without being told to
pub const BUILT_IN: [(&str, TokenKind); 27] = [
    ("error", TokenKind::Failure),
    ("critical", TokenKind::Failure),
    ("fatal", TokenKind::Failure),
    ("warn", TokenKind::Warning),
    ("warning", TokenKind::Warning),
    ("info", TokenKind::Level),
    ("debug", TokenKind::Level),
    ("tcp", TokenKind::Protocol),
    ("udp", TokenKind::Protocol),
    ("http", TokenKind::Protocol),
    ("https", TokenKind::Protocol),
    ("ssl", TokenKind::Protocol),
    ("tls", TokenKind::Protocol),
    ("success", TokenKind::Success),
    ("ok", TokenKind::Success),
    ("failure", TokenKind::Failure),
    ("failed", TokenKind::Failure),
    ("start", TokenKind::Keyword),
    ("stop", TokenKind::Keyword),
    ("restart", TokenKind::Keyword),
    ("reload", TokenKind::Keyword),
    ("select", TokenKind::Method),
    ("insert", TokenKind::Method),
    ("update", TokenKind::Method),
    ("delete", TokenKind::Method),
    ("commit", TokenKind::Method),
    ("rollback", TokenKind::Method),
];

/// Error returned when configured keywords cannot be used
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordError {
    UnknownKind(String),
    BadPattern { pattern: String, message: String },
}

impl fmt::Display for KeywordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeywordError::UnknownKind(name) => write!(f, "Unknown keyword color '{}'", name),
            KeywordError::BadPattern { pattern, message } => {
                write!(f, "Invalid keyword pattern '{}': {}", pattern, message)
            }
        }
    }
}

impl std::error::Error for KeywordError {}

/// The keywords and patterns a run styles
#[derive(Debug, Clone)]
pub struct Keywords {
    /// Every keyword, matched as a whole word in any case
    words: Option<Regex>,
    /// The style for each keyword, by its lowercase text
    kinds: HashMap<String, TokenKind>,
    /// Configured patterns, each with the style for its matches
    patterns: Vec<(Regex, TokenKind)>,
}

impl PartialEq for Keywords {
    fn eq(&self, other: &Self) -> bool {
        let source = |keywords: &Keywords| {
            (
                keywords
                    .words
                    .as_ref()
                    .map(|words| words.as_str().to_string()),
                keywords
                    .patterns
                    .iter()
                    .map(|(pattern, kind)| (pattern.as_str().to_string(), *kind))
                    .collect::<Vec<_>>(),
            )
        };

        source(self) == source(other) && self.kinds == other.kinds
    }
}

impl Eq for Keywords {}

impl Default for Keywords {
    fn default() -> Self {
        Keywords::none()
    }
}

impl Keywords {
    /// Styles nothing
    pub fn none() -> Keywords {
        Keywords {
            words: None,
            kinds: HashMap::new(),
            patterns: Vec::new(),
        }
    }

    /// The built-in keywords, plus the `(color key, comma-separated words)`
    /// and `(color key, pattern)` pairs a config file lists
    pub fn build(
        configured: &[(String, String)],
        patterns: &[(String, String)],
    ) -> Result<Keywords, KeywordError> {
        let mut kinds: HashMap<String, TokenKind> = BUILT_IN
            .iter()
            .map(|(word, kind)| (word.to_string(), *kind))
            .collect();

        for (key, words) in configured {
            let kind = kind_named(key)?;

            for word in words
                .split(',')
                .map(str::trim)
                .filter(|word| !word.is_empty())
            {
                kinds.insert(word.to_lowercase(), kind);
            }
        }

        let mut words: Vec<&String> = kinds.keys().collect();
        words.sort_by(|left, right| right.len().cmp(&left.len()).then(left.cmp(right)));

        let alternatives: Vec<String> = words
            .iter()
            .map(|word| {
                let escaped = regex::escape(word);
                let open = if word.starts_with(is_word) { r"\b" } else { "" };
                let close = if word.ends_with(is_word) { r"\b" } else { "" };

                format!("{}{}{}", open, escaped, close)
            })
            .collect();

        let words = RegexBuilder::new(&alternatives.join("|"))
            .case_insensitive(true)
            .build()
            .unwrap();

        let patterns = patterns
            .iter()
            .map(|(key, pattern)| {
                let regex = Regex::new(pattern).map_err(|error| KeywordError::BadPattern {
                    pattern: pattern.clone(),
                    message: error.to_string(),
                })?;

                Ok((regex, kind_named(key)?))
            })
            .collect::<Result<_, KeywordError>>()?;

        Ok(Keywords {
            words: Some(words),
            kinds,
            patterns,
        })
    }

    /// The built-in keywords alone
    pub fn built_in() -> Keywords {
        Keywords::build(&[], &[]).unwrap()
    }

    /// Restyles the keywords and pattern matches in the plain and message
    /// text of a line. A keyword or match can span the tokens of a run of
    /// plain and message text, and a pattern match wins over a keyword it
    /// overlaps.
    pub fn apply<'a>(&self, parsed: ParsedLine<'a>) -> ParsedLine<'a> {
        let Some(words) = &self.words else {
            return parsed;
        };

        let mut tokens = Vec::new();
        let mut run: Vec<Token<'a>> = Vec::new();

        for token in parsed.tokens {
            if matches!(token.kind, TokenKind::Plain | TokenKind::Message) {
                run.push(token);
                continue;
            }

            self.restyle_run(words, &mut run, &mut tokens);
            tokens.push(token);
        }

        self.restyle_run(words, &mut run, &mut tokens);

        ParsedLine::new(tokens)
    }

    fn restyle_run<'a>(
        &self,
        words: &Regex,
        run: &mut Vec<Token<'a>>,
        tokens: &mut Vec<Token<'a>>,
    ) {
        let text: String = run.iter().map(|token| token.text.as_ref()).collect();
        let mut marks: Vec<Option<TokenKind>> = vec![None; text.len()];

        for found in words.find_iter(&text) {
            let kind = self.kinds[&found.as_str().to_lowercase()];
            marks[found.range()].fill(Some(kind));
        }

        for (pattern, kind) in &self.patterns {
            for found in pattern.find_iter(&text) {
                marks[found.range()].fill(Some(*kind));
            }
        }

        let mut offset = 0;

        for token in run.drain(..) {
            let end = offset + token.text.len();
            tokens.extend(restyle(&token, &marks[offset..end]));
            offset = end;
        }
    }
}

fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn kind_named(key: &str) -> Result<TokenKind, KeywordError> {
    TokenKind::from_name(key).ok_or_else(|| KeywordError::UnknownKind(key.to_string()))
}

/// Splits a token into runs of one mark each, styling a marked run with its
/// mark and an unmarked run with the token's own style
pub fn restyle<'a>(token: &Token<'a>, marks: &[Option<TokenKind>]) -> Vec<Token<'a>> {
    let mut pieces = Vec::new();
    let mut start = 0;

    for end in 1..=token.text.len() {
        if end < token.text.len() && marks[end] == marks[start] {
            continue;
        }

        if token.text.is_char_boundary(end) {
            let mut piece = token.slice(start, end);
            piece.kind = marks[start].unwrap_or(token.kind);
            pieces.push(piece);
            start = end;
        }
    }

    pieces
}
