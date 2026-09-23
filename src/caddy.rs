//! Caddy structured log parsing
//!
//! Caddy's JSON encoder writes one object per line. The line is scanned as
//! JSON rather than matched against a pattern, so every key and value is
//! colored wherever it sits in the object, including the nested `request` and
//! `headers` objects. A value takes its color from the key that introduces
//! it, and a key Caddy alone knows about still reads as a key.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "caddy";

/// The Caddy structured log plugin
pub struct CaddyPlugin {
    metadata: PluginMetadata,
}

impl CaddyPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "Caddy structured JSON logs",
                "splash",
            ),
        }
    }
}

impl Default for CaddyPlugin {
    fn default() -> Self {
        CaddyPlugin::new()
    }
}

impl Plugin for CaddyPlugin {
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

/// Parses one JSON object, or returns `None` when the line is not a single
/// well formed object.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let mut scanner = Scanner::new(line);

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

/// The style a value takes from the key that introduces it. A key splash has
/// no field for leaves its value in the default style for that value's type.
pub fn value_kind(key: &str) -> Option<TokenKind> {
    match key {
        "level" => Some(TokenKind::Level),
        "ts" | "time" => Some(TokenKind::Timestamp),
        "logger" => Some(TokenKind::Tag),
        "msg" | "message" | "error" => Some(TokenKind::Message),
        "remote_ip" | "client_ip" => Some(TokenKind::Ip),
        "proto" => Some(TokenKind::Protocol),
        "method" => Some(TokenKind::Method),
        "host" | "server_name" => Some(TokenKind::Host),
        "uri" => Some(TokenKind::Request),
        "status" => Some(TokenKind::Status),
        "size" | "bytes_read" => Some(TokenKind::Size),
        "duration" => Some(TokenKind::Duration),
        "user_id" => Some(TokenKind::UserId),
        _ => None,
    }
}

/// A walk over one JSON line that collects styled tokens as it goes
struct Scanner<'a> {
    text: &'a str,
    position: usize,
    tokens: Vec<Token<'a>>,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            position: 0,
            tokens: Vec::new(),
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

            let key = self.string(TokenKind::Header)?;

            self.skip_whitespace();
            self.punctuation(b':')?;
            self.skip_whitespace();
            self.value(value_kind(key))?;
            self.skip_whitespace();

            if self.punctuation(b',').is_some() {
                continue;
            }

            return self.punctuation(b'}');
        }

        None
    }

    /// Scans an array, giving every element the style of the key that
    /// introduced the array.
    fn array(&mut self, kind: Option<TokenKind>) -> Option<()> {
        self.punctuation(b'[')?;

        while !self.at_end() {
            self.skip_whitespace();

            if self.punctuation(b']').is_some() {
                return Some(());
            }

            self.value(kind)?;
            self.skip_whitespace();

            if self.punctuation(b',').is_some() {
                continue;
            }

            return self.punctuation(b']');
        }

        None
    }

    /// Scans one value of any type.
    fn value(&mut self, kind: Option<TokenKind>) -> Option<()> {
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(kind),
            b'"' => self.string(kind.unwrap_or(TokenKind::Plain)).map(|_| ()),
            _ => self.literal(kind),
        }
    }

    /// Scans a quoted string, emitting its quotes apart from its text, and
    /// returns the text between them.
    fn string(&mut self, kind: TokenKind) -> Option<&'a str> {
        self.punctuation(b'"')?;

        let start = self.position;

        while let Some(byte) = self.peek() {
            if byte == b'"' {
                let text = &self.text[start..self.position];

                if !text.is_empty() {
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
    fn literal(&mut self, kind: Option<TokenKind>) -> Option<()> {
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
            _ => kind.unwrap_or(TokenKind::Number),
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
