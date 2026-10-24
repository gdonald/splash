/// Log line parsing into styled tokens
///
/// Parsing is separated from rendering so that a single parse of a log line
/// can be emitted as ANSI, HTML, JSON, or plain text.
use crate::detect;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::ParseResult;
use crate::registry;
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

/// A date and time as application loggers write them, such as
/// `2023-10-03 12:00:01,123` or `2023-10-03T12:00:01.123Z`
static ISO_TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[-+]\d{2}:?\d{2})?")
        .unwrap()
});

/// A run of whitespace, or a run of anything else
static RUNS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+|\S+").unwrap());

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

/// An IPv4 address anywhere inside a free text message
static MESSAGE_IP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap());

/// Colors any addresses inside a free text message, leaving the rest of the
/// message in the message style.
pub fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for found in MESSAGE_IP.find_iter(message) {
        if found.start() > cursor {
            tokens.push(Token::new(
                &message[cursor..found.start()],
                TokenKind::Message,
            ));
        }

        tokens.push(Token::new(found.as_str(), TokenKind::Ip));
        cursor = found.end();
    }

    if cursor < message.len() {
        tokens.push(Token::new(&message[cursor..], TokenKind::Message));
    }
}

/// Parses one line according to the given mode.
///
/// Returns `None` when the mode has nothing to emit for the line, such as a
/// blank line or a line that does not match the Common Log Format.
pub fn parse_line<'a>(line: &'a str, mode: &str) -> Option<ParsedLine<'a>> {
    if line.is_empty() {
        return None;
    }

    if detect::is_auto(mode) {
        return detect::parse_auto_line(line, mode);
    }

    match mode {
        "clf" => parse_clf_line(line),
        _ => match registry::builtins().get(mode) {
            Ok(plugin) => match plugin.parse_display_line(line) {
                ParseResult::Parsed(parsed) => Some(parsed),
                ParseResult::NoMatch | ParseResult::Error(_) => None,
            },
            Err(_) => Some(parse_adhoc_line(line)),
        },
    }
}

/// Parses a line with the general purpose pattern matchers, keeping its
/// whitespace as it is, so the indentation of a stack trace survives.
pub fn parse_adhoc_line(line: &str) -> ParsedLine<'_> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut cursor = 0;

    for timestamp in ISO_TIMESTAMP.find_iter(line) {
        push_words(&mut tokens, &line[cursor..timestamp.start()]);
        tokens.push(Token::new(timestamp.as_str(), TokenKind::Timestamp));
        cursor = timestamp.end();
    }

    push_words(&mut tokens, &line[cursor..]);

    ParsedLine::new(tokens)
}

fn push_words<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    for run in RUNS.find_iter(text) {
        match run.as_str().starts_with(char::is_whitespace) {
            true => tokens.push(Token::new(run.as_str(), TokenKind::Plain)),
            false => push_word(tokens, run.as_str()),
        }
    }
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
    let kinds = [
        TokenKind::Client,
        TokenKind::UserIdentifier,
        TokenKind::UserId,
        TokenKind::Timestamp,
        TokenKind::Method,
        TokenKind::Request,
        TokenKind::Protocol,
        TokenKind::Status,
        TokenKind::Size,
    ];
    let mut tokens = Vec::new();
    let mut cursor = 0;

    for (index, kind) in kinds.into_iter().enumerate() {
        let field = caps.get(index + 1).unwrap();

        push_between(&mut tokens, &line[cursor..field.start()]);
        tokens.push(Token::new(field.as_str(), kind));
        cursor = field.end();
    }

    push_between(&mut tokens, &line[cursor..]);

    Some(ParsedLine::new(tokens))
}

/// Pushes the text between fields: each quote as punctuation and the rest
/// as plain text
fn push_between<'a>(tokens: &mut Vec<Token<'a>>, text: &'a str) {
    for piece in text.split_inclusive('"') {
        match piece.strip_suffix('"') {
            Some(before) => {
                if !before.is_empty() {
                    tokens.push(Token::new(before, TokenKind::Plain));
                }

                tokens.push(Token::new("\"", TokenKind::Punctuation));
            }
            None => tokens.push(Token::new(piece, TokenKind::Plain)),
        }
    }
}
