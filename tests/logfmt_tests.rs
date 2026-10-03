use splash::logfmt;
use splash::output::{Token, TokenKind};

/// Colors `level` as a level and leaves every other key as message text
fn level_only(key: &str, value: &str) -> Option<TokenKind> {
    (key == "level").then(|| logfmt::level_kind(value))
}

/// The tokens the pair scanner splits a text into
fn pairs(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    logfmt::push_pairs(&mut tokens, text, level_only);
    tokens
}

#[test]
fn a_bare_value_is_colored_by_its_key() {
    assert_eq!(
        pairs("level=warn"),
        vec![
            Token::new("level", TokenKind::Header),
            Token::new("=", TokenKind::Punctuation),
            Token::new("warn", TokenKind::Warning),
        ]
    );
}

#[test]
fn a_quoted_value_keeps_its_quotes_as_punctuation() {
    assert_eq!(
        pairs(r#"msg="Starting up""#),
        vec![
            Token::new("msg", TokenKind::Header),
            Token::new("=", TokenKind::Punctuation),
            Token::new("\"", TokenKind::Punctuation),
            Token::new("Starting up", TokenKind::Message),
            Token::new("\"", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn an_empty_value_is_only_its_key() {
    assert_eq!(
        pairs("out="),
        vec![
            Token::new("out", TokenKind::Header),
            Token::new("=", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn an_empty_quoted_value_is_only_its_quotes() {
    assert_eq!(pairs(r#"msg="""#).len(), 4);
}

#[test]
fn the_text_between_pairs_is_message_text() {
    assert_eq!(
        pairs("done level=info now")[0],
        Token::new("done ", TokenKind::Message)
    );
}

#[test]
fn go_logger_levels_are_colored_by_how_severe_they_are() {
    assert_eq!(logfmt::level_kind("ERROR"), TokenKind::Failure);
    assert_eq!(logfmt::level_kind("fatal"), TokenKind::Failure);
    assert_eq!(logfmt::level_kind("Warning"), TokenKind::Warning);
    assert_eq!(logfmt::level_kind("debug"), TokenKind::Level);
}

#[test]
fn a_key_opening_with_an_at_sign_is_read() {
    assert_eq!(
        pairs("@module=aws")[0],
        Token::new("@module", TokenKind::Header)
    );
}
