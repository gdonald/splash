/// Log line parsing into styled tokens
///
/// Parsing is separated from rendering so that a single parse of a log line
/// can be emitted as ANSI, HTML, JSON, or plain text.
use crate::output::{ParsedLine, Token, TokenKind};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static MATCHERS: LazyLock<HashMap<&'static str, Regex>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(
        "ip_addr",
        Regex::new(r".*(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}).*").unwrap(),
    );
    m.insert(
        "http_verb",
        Regex::new(r"(.*)(GET|POST|PUT|PATCH|DELETE|HEAD|CONNECT|OPTIONS|TRACE)(.*)").unwrap(),
    );
    m.insert("http_version", Regex::new(r"HTTP/1.0").unwrap());
    m.insert("number", Regex::new(r"^\d+$").unwrap());
    m.insert(
        "datetime",
        Regex::new(r"\d{2}/[[:alpha:]]{3}/\d{4}:\d{2}:\d{2}:\d{2}").unwrap(),
    );
    m.insert("tz_offset", Regex::new(r"^[-]?\d{4}$").unwrap());

    m
});

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

fn matcher(name: &str) -> &Regex {
    MATCHERS.get(name).unwrap()
}

/// Parses one line according to the given mode.
///
/// Returns `None` when the mode has nothing to emit for the line, such as a
/// blank line or a line that does not match the Common Log Format.
pub fn parse_line(line: &str, mode: &str) -> Option<ParsedLine> {
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
pub fn parse_adhoc_line(line: &str) -> ParsedLine {
    let mut tokens: Vec<Token> = Vec::new();

    for word in line.split_whitespace() {
        if !tokens.is_empty() {
            tokens.push(Token::new(" ", TokenKind::Plain));
        }

        push_word(&mut tokens, word);
    }

    ParsedLine::new(tokens)
}

fn push_word(tokens: &mut Vec<Token>, word: &str) {
    let mut core = String::new();

    for character in word.chars() {
        if PUNCTUATION.contains(&character) {
            push_core(tokens, &core);
            core.clear();
            tokens.push(Token::new(&character.to_string(), TokenKind::Punctuation));
        } else {
            core.push(character);
        }
    }

    push_core(tokens, &core);
}

fn push_core(tokens: &mut Vec<Token>, core: &str) {
    if core.is_empty() {
        return;
    }

    if let Some(kind) = whole_word_kind(core) {
        tokens.push(Token::new(core, kind));
        return;
    }

    let verb = matcher("http_verb");
    if let Some(caps) = verb.captures(core) {
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
    if matcher("number").is_match(core) {
        return Some(TokenKind::Number);
    }
    if matcher("ip_addr").is_match(core) {
        return Some(TokenKind::Ip);
    }
    if matcher("datetime").is_match(core) {
        return Some(TokenKind::DateTime);
    }
    if matcher("tz_offset").is_match(core) {
        return Some(TokenKind::TimezoneOffset);
    }
    if matcher("http_version").is_match(core) {
        return Some(TokenKind::HttpVersion);
    }

    None
}

/// Parses a Common Log Format line into its named fields.
pub fn parse_clf_line(line: &str) -> Option<ParsedLine> {
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
