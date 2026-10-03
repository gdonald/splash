//! JSON line scanning
//!
//! Caddy, MongoDB, and Elasticsearch write one JSON object per line. The line
//! is scanned as JSON rather than matched against a pattern, so every key and
//! value is colored wherever it sits in the object, including nested objects.
//! Keys are colored as headers, and each plugin says how a value is colored
//! from the key that introduces it and the value itself.
use crate::output::{ParsedLine, Token, TokenKind};

/// How a plugin colors a value, from the key that introduces it and the
/// value's text. `None` leaves a string plain and a number as a number.
pub type ValueKind = fn(key: &str, value: &str) -> Option<TokenKind>;

/// Parses one JSON object, or returns `None` when the line is not a single
/// well formed object.
pub fn parse_object(line: &str, value_kind: ValueKind) -> Option<ParsedLine<'_>> {
    let mut scanner = Scanner::new(line, value_kind);

    scanner.skip_whitespace();

    if scanner.peek() != Some(b'{') {
        return None;
    }

    scanner.object()?;
    scanner.skip_whitespace();

    if !scanner.at_end() {
        return None;
    }

    Some(ParsedLine::new(scanner.tokens))
}

/// A walk over one JSON line that collects styled tokens as it goes
struct Scanner<'a> {
    text: &'a str,
    position: usize,
    tokens: Vec<Token<'a>>,
    value_kind: ValueKind,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str, value_kind: ValueKind) -> Self {
        Self {
            text,
            position: 0,
            tokens: Vec::new(),
            value_kind,
        }
    }

    fn at_end(&self) -> bool {
        self.position >= self.text.len()
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.position).copied()
    }

    /// Emits the run of spacing at the cursor, which JSON allows between any
    /// two pieces of syntax.
    fn skip_whitespace(&mut self) {
        let start = self.position;

        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.position += 1;
        }

        if self.position > start {
            self.push(start, TokenKind::Plain);
        }
    }

    /// Emits the one character at the cursor as punctuation, failing when it
    /// is not the character the grammar calls for.
    fn punctuation(&mut self, expected: u8) -> Option<()> {
        if self.peek() != Some(expected) {
            return None;
        }

        let start = self.position;
        self.position += 1;
        self.push(start, TokenKind::Punctuation);

        Some(())
    }

    /// Scans an object, coloring each key as a key and each value by the key
    /// that introduces it.
    fn object(&mut self) -> Option<()> {
        self.punctuation(b'{')?;

        while !self.at_end() {
            self.skip_whitespace();

            if self.punctuation(b'}').is_some() {
                return Some(());
            }

            let key = self.string(None)?;

            self.skip_whitespace();
            self.punctuation(b':')?;
            self.skip_whitespace();
            self.value(Some(key))?;
            self.skip_whitespace();

            if self.punctuation(b',').is_some() {
                continue;
            }

            return self.punctuation(b'}');
        }

        None
    }

    /// Scans an array, styling every element by the key that introduced the
    /// array.
    fn array(&mut self, key: Option<&'a str>) -> Option<()> {
        self.punctuation(b'[')?;

        while !self.at_end() {
            self.skip_whitespace();

            if self.punctuation(b']').is_some() {
                return Some(());
            }

            self.value(key)?;
            self.skip_whitespace();

            if self.punctuation(b',').is_some() {
                continue;
            }

            return self.punctuation(b']');
        }

        None
    }

    /// Scans one value of any type, introduced by `key`.
    fn value(&mut self, key: Option<&'a str>) -> Option<()> {
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(key),
            b'"' => self.string(key).map(|_| ()),
            _ => self.literal(key),
        }
    }

    /// The kind a value takes from the key that introduces it
    fn kind_of(&self, key: Option<&str>, value: &str) -> Option<TokenKind> {
        key.and_then(|key| (self.value_kind)(key, value))
    }

    /// Scans a quoted string, emitting its quotes apart from its text, and
    /// returns the text between them. A string with no key is a key itself.
    fn string(&mut self, key: Option<&'a str>) -> Option<&'a str> {
        self.punctuation(b'"')?;

        let start = self.position;

        while let Some(byte) = self.peek() {
            if byte == b'"' {
                let text = &self.text[start..self.position];

                if !text.is_empty() {
                    let kind = match key {
                        Some(_) => self.kind_of(key, text).unwrap_or(TokenKind::Plain),
                        None => TokenKind::Header,
                    };

                    self.push(start, kind);
                }

                self.punctuation(b'"')?;

                return Some(text);
            }

            self.position += if byte == b'\\' { 2 } else { 1 };
        }

        None
    }

    /// Scans a number or one of the three bare words JSON allows. A bare word
    /// keeps the plain style whatever key introduced it.
    fn literal(&mut self, key: Option<&'a str>) -> Option<()> {
        let start = self.position;

        while matches!(self.peek(), Some(byte) if is_literal_byte(byte)) {
            self.position += 1;
        }

        if self.position == start {
            return None;
        }

        let text = &self.text[start..self.position];

        let style = match text {
            "true" | "false" | "null" => TokenKind::Plain,
            _ => self.kind_of(key, text).unwrap_or(TokenKind::Number),
        };

        self.push(start, style);

        Some(())
    }

    fn push(&mut self, start: usize, kind: TokenKind) {
        self.tokens
            .push(Token::new(&self.text[start..self.position], kind));
    }
}

/// Whether a byte can appear in a number or in `true`, `false`, or `null`
fn is_literal_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'+' | b'.')
}
