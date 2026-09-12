/// Log line parsing into styled tokens
///
/// Parsing is separated from rendering so that a single parse of a log line
/// can be emitted as ANSI, HTML, JSON, or plain text.
use crate::output::{ParsedLine, Token, TokenKind};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

/// Patterns are compiled the first time they are used and kept for the rest of
/// the run, so a pattern a log never exercises costs nothing.
static IP_ADDR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r".*(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}).*").unwrap());

static HTTP_VERB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(.*)(GET|POST|PUT|PATCH|DELETE|HEAD|CONNECT|OPTIONS|TRACE)(.*)").unwrap()
});

static HTTP_VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"HTTP/1.0").unwrap());

static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());

static DATETIME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{2}/[[:alpha:]]{3}/\d{4}:\d{2}:\d{2}:\d{2}").unwrap());

static TZ_OFFSET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[-]?\d{4}$").unwrap());

/// Patterns compiled at run time, such as ones read from a config file
static PATTERN_CACHE: LazyLock<Mutex<HashMap<String, Arc<Regex>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Compiles a pattern the first time it is asked for and hands back the same
/// compiled pattern on every later call.
pub fn cached_pattern(pattern: &str) -> Result<Arc<Regex>, regex::Error> {
    let mut cache = PATTERN_CACHE.lock().unwrap();

    if let Some(compiled) = cache.get(pattern) {
        return Ok(Arc::clone(compiled));
    }

    let compiled = Arc::new(Regex::new(pattern)?);
    cache.insert(pattern.to_string(), Arc::clone(&compiled));

    Ok(compiled)
}

/// The number of patterns compiled at run time and held in the cache
pub fn cached_pattern_count() -> usize {
    PATTERN_CACHE.lock().unwrap().len()
}

static CLF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?x)
        ([\d]{1,3}\.[\d]{1,3}\.[\d]{1,3}\.[\d]{1,3}) # client
        \s
        (\S+)                                        # user_identifier
        \s
        (\S+)                                        # userid
        \s
        (?:(\[.*?\]))                                # datetime
        \s
        "([A-Z]+)\s(\S+)\s(\S+)"                     # method, request, protocol
        \s
        (\d{3})                                      # status
        \s
        (\d+|-)                                      # size
        "#,
    )
    .unwrap()
});

const PUNCTUATION: [char; 3] = ['"', '[', ']'];

/// Parses one line according to the given mode.
///
/// Returns `None` when the mode has nothing to emit for the line, such as a
/// blank line or a line that does not match the Common Log Format.
pub fn parse_line<'a>(line: &'a str, mode: &str) -> Option<ParsedLine<'a>> {
    if line.is_empty() {
        return None;
    }

    match mode {
        "clf" => parse_clf_line(line),
        _ => Some(parse_adhoc_line(line)),
    }
}

/// Parses a line with the general purpose pattern matchers.
///
/// Runs of whitespace collapse to a single space, matching the ad-hoc output
/// splash has always produced.
pub fn parse_adhoc_line(line: &str) -> ParsedLine<'_> {
    let mut tokens: Vec<Token> = Vec::new();

    for word in line.split_whitespace() {
        if !tokens.is_empty() {
            tokens.push(Token::new(" ", TokenKind::Plain));
        }

        push_word(&mut tokens, word);
    }

    ParsedLine::new(tokens)
}

fn push_word<'a>(tokens: &mut Vec<Token<'a>>, word: &'a str) {
    let mut start = 0;

    for (index, character) in word.char_indices() {
        if PUNCTUATION.contains(&character) {
            push_core(tokens, &word[start..index]);
            start = index + character.len_utf8();
            tokens.push(Token::new(&word[index..start], TokenKind::Punctuation));
        }
    }

    push_core(tokens, &word[start..]);
}

fn push_core<'a>(tokens: &mut Vec<Token<'a>>, core: &'a str) {
    if core.is_empty() {
        return;
    }

    if let Some(kind) = whole_word_kind(core) {
        tokens.push(Token::new(core, kind));
        return;
    }

    if let Some(caps) = HTTP_VERB.captures(core) {
        let before = caps.get(1).unwrap().as_str();
        let matched = caps.get(2).unwrap().as_str();
        let after = caps.get(3).unwrap().as_str();

        if !before.is_empty() {
            tokens.push(Token::new(before, TokenKind::Plain));
        }
        tokens.push(Token::new(matched, TokenKind::HttpVerb));
        if !after.is_empty() {
            tokens.push(Token::new(after, TokenKind::Plain));
        }

        return;
    }

    tokens.push(Token::new(core, TokenKind::Plain));
}

fn whole_word_kind(core: &str) -> Option<TokenKind> {
    if NUMBER.is_match(core) {
        return Some(TokenKind::Number);
    }
    if IP_ADDR.is_match(core) {
        return Some(TokenKind::Ip);
    }
    if DATETIME.is_match(core) {
        return Some(TokenKind::DateTime);
    }
    if TZ_OFFSET.is_match(core) {
        return Some(TokenKind::TimezoneOffset);
    }
    if HTTP_VERSION.is_match(core) {
        return Some(TokenKind::HttpVersion);
    }

    None
}

/// Parses a Common Log Format line into its named fields.
pub fn parse_clf_line(line: &str) -> Option<ParsedLine<'_>> {
    let caps = CLF.captures(line)?;
    let field = |index: usize| caps.get(index).unwrap().as_str();

    let tokens = vec![
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
    ];

    Some(ParsedLine::new(tokens))
}
