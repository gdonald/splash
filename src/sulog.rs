//! sulog parsing
//!
//! Solaris and other System V systems record each use of `su` in
//! `/var/adm/sulog` as `SU 10/03 12:00 + pts/1 alice-root`: the date, `+`
//! for a successful switch or `-` for a failed one, the terminal, and the
//! user who ran su joined by a dash to the user they became.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "sulog";

static LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(SU)( )(\d{2}/\d{2} \d{2}:\d{2})( )([+-])( )(\S+)( )([^-]+)(-)(.*)$").unwrap()
});

/// The sulog plugin
pub struct SulogPlugin {
    metadata: PluginMetadata,
}

impl SulogPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "System V su command logs",
                "splash",
            ),
        }
    }
}

impl Default for SulogPlugin {
    fn default() -> Self {
        SulogPlugin::new()
    }
}

impl Plugin for SulogPlugin {
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

/// Parses one sulog line, or returns `None` when the line is not one.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = LINE.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let outcome = if field(5) == "+" {
        TokenKind::Success
    } else {
        TokenKind::Failure
    };

    let terminal = if field(7) == "?" {
        TokenKind::Plain
    } else {
        TokenKind::Path
    };

    let mut tokens = vec![
        Token::new(field(1), TokenKind::Tag),
        Token::new(field(2), TokenKind::Plain),
        Token::new(field(3), TokenKind::Timestamp),
        Token::new(field(4), TokenKind::Plain),
        Token::new(field(5), outcome),
        Token::new(field(6), TokenKind::Plain),
        Token::new(field(7), terminal),
        Token::new(field(8), TokenKind::Plain),
        Token::new(field(9), TokenKind::UserId),
        Token::new(field(10), TokenKind::Punctuation),
    ];

    if !field(11).is_empty() {
        tokens.push(Token::new(field(11), TokenKind::UserId));
    }

    Some(ParsedLine::new(tokens))
}
