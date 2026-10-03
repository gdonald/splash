use splash::json;
use splash::output::{Token, TokenKind};

/// Colors `level` by its value and leaves every other key to the defaults
fn level_by_value(key: &str, value: &str) -> Option<TokenKind> {
    match (key, value) {
        ("level", "error") => Some(TokenKind::Failure),
        ("level", _) => Some(TokenKind::Level),
        _ => None,
    }
}

#[test]
fn a_value_is_colored_by_its_key_and_its_text() {
    assert_eq!(
        json::parse_object(r#"{"level":"error"}"#, level_by_value)
            .unwrap()
            .tokens,
        vec![
            Token::new("{", TokenKind::Punctuation),
            Token::new("\"", TokenKind::Punctuation),
            Token::new("level", TokenKind::Header),
            Token::new("\"", TokenKind::Punctuation),
            Token::new(":", TokenKind::Punctuation),
            Token::new("\"", TokenKind::Punctuation),
            Token::new("error", TokenKind::Failure),
            Token::new("\"", TokenKind::Punctuation),
            Token::new("}", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_string_with_no_style_for_its_key_is_plain() {
    let parsed = json::parse_object(r#"{"note":"queued"}"#, level_by_value).unwrap();

    assert!(parsed
        .tokens
        .contains(&Token::new("queued", TokenKind::Plain)));
}

#[test]
fn a_number_with_no_style_for_its_key_is_a_number() {
    let parsed = json::parse_object(r#"{"count":3}"#, level_by_value).unwrap();

    assert!(parsed.tokens.contains(&Token::new("3", TokenKind::Number)));
}

#[test]
fn a_number_is_colored_by_its_key_and_its_text() {
    let parsed = json::parse_object(r#"{"level":7}"#, level_by_value).unwrap();

    assert!(parsed.tokens.contains(&Token::new("7", TokenKind::Level)));
}

#[test]
fn each_element_of_an_array_is_colored_by_the_key_of_the_array() {
    let parsed = json::parse_object(r#"{"level":["error","info"]}"#, level_by_value).unwrap();

    assert!(parsed
        .tokens
        .contains(&Token::new("error", TokenKind::Failure)));
    assert!(parsed
        .tokens
        .contains(&Token::new("info", TokenKind::Level)));
}

#[test]
fn a_line_that_is_not_an_object_is_not_parsed() {
    assert!(json::parse_object("[1, 2]", level_by_value).is_none());
}
