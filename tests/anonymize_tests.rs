use splash::anonymize::Anonymizer;
use splash::output::{ParsedLine, Token, TokenKind};
use std::borrow::Cow;

fn mask_shape(text: &str, prefix: &str) -> bool {
    let regex = regex::Regex::new(&format!("^{}-[0-9a-f]{{8}}$", prefix)).unwrap();

    regex.is_match(text)
}

fn masked(tokens: Vec<Token<'static>>) -> Vec<Token<'static>> {
    Anonymizer::with_key(7)
        .apply(ParsedLine::new(tokens))
        .tokens
}

#[test]
fn a_value_gets_the_same_mask_each_time() {
    let anonymizer = Anonymizer::with_key(7);

    assert_eq!(
        anonymizer.mask("ip", "10.0.0.5"),
        anonymizer.mask("ip", "10.0.0.5")
    );
}

#[test]
fn different_values_get_different_masks() {
    let anonymizer = Anonymizer::with_key(7);

    assert_ne!(
        anonymizer.mask("ip", "10.0.0.5"),
        anonymizer.mask("ip", "10.0.0.6")
    );
}

#[test]
fn a_different_key_gives_a_value_a_different_mask() {
    assert_ne!(
        Anonymizer::with_key(7).mask("user", "alice"),
        Anonymizer::with_key(8).mask("user", "alice")
    );
}

#[test]
fn a_mask_is_a_prefix_and_eight_hex_digits() {
    assert!(mask_shape(
        &Anonymizer::default().mask("user", "alice"),
        "user"
    ));
}

#[test]
fn addresses_and_user_names_are_masked_by_kind() {
    let tokens = masked(vec![
        Token::new("10.0.0.5", TokenKind::Ip),
        Token::new("10.0.0.6", TokenKind::Client),
        Token::new("alice@example.com", TokenKind::Email),
        Token::new("alice", TokenKind::UserId),
        Token::new("mallory", TokenKind::UnknownUser),
    ]);

    let prefixes = ["ip", "ip", "email", "user", "user"];

    for (token, prefix) in tokens.iter().zip(prefixes) {
        assert!(mask_shape(&token.text, prefix), "{:?}", token);
    }
}

#[test]
fn a_masked_token_keeps_its_kind() {
    let tokens = masked(vec![Token::new("alice", TokenKind::UserId)]);

    assert_eq!(tokens[0].kind, TokenKind::UserId);
}

#[test]
fn a_missing_user_is_kept() {
    let tokens = masked(vec![Token::new("-", TokenKind::UserId)]);

    assert_eq!(tokens[0].text, "-");
}

#[test]
fn addresses_inside_other_text_are_masked() {
    let anonymizer = Anonymizer::with_key(7);
    let tokens = masked(vec![Token::new(
        "mail to bob@example.com from 10.0.0.5 failed",
        TokenKind::Message,
    )]);

    assert_eq!(
        tokens[0].text,
        format!(
            "mail to {} from {} failed",
            anonymizer.mask("email", "bob@example.com"),
            anonymizer.mask("ip", "10.0.0.5")
        )
    );
}

#[test]
fn text_holding_no_address_is_left_borrowed() {
    let tokens = masked(vec![Token::new("service started", TokenKind::Message)]);

    assert!(matches!(tokens[0].text, Cow::Borrowed("service started")));
}
