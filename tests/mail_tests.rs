use splash::mail::{self, FieldValue, Words};
use splash::output::{Token, TokenKind};
use std::sync::LazyLock;

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("flushed", TokenKind::Success),
        ("not flushed", TokenKind::Warning),
    ])
});

/// The tokens the shared field scanner splits the text into
fn fields(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    mail::push_fields(&mut tokens, text, &WORDS);
    tokens
}

/// The text of the first token of the given kind
fn field(text: &str, kind: TokenKind) -> String {
    fields(text)
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), text))
        .text
        .to_string()
}

/// The text the scanner kept, joined back together
fn joined(text: &str) -> String {
    fields(text).iter().map(|token| token.text).collect()
}

#[test]
fn a_field_colors_its_key_as_a_header() {
    assert_eq!(field("size=4512", TokenKind::Header), "size");
}

#[test]
fn a_field_colors_its_value_by_its_key() {
    assert_eq!(field("size=4512", TokenKind::Size), "4512");
}

#[test]
fn a_field_value_in_angle_brackets_keeps_the_brackets_as_punctuation() {
    let tokens = fields("from=<alice@example.com>");

    assert_eq!(
        tokens,
        vec![
            Token::new("from", TokenKind::Header),
            Token::new("=", TokenKind::Punctuation),
            Token::new("<", TokenKind::Punctuation),
            Token::new("alice@example.com", TokenKind::Email),
            Token::new(">", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn an_empty_field_value_in_angle_brackets_is_only_punctuation() {
    assert_eq!(
        fields("from=<>"),
        vec![
            Token::new("from", TokenKind::Header),
            Token::new("=", TokenKind::Punctuation),
            Token::new("<", TokenKind::Punctuation),
            Token::new(">", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_quoted_field_value_keeps_its_quotes_as_punctuation() {
    let tokens = fields(r#"C="250 OK""#);

    assert_eq!(tokens[2], Token::new("\"", TokenKind::Punctuation));
    assert_eq!(tokens[4], Token::new("\"", TokenKind::Punctuation));
}

#[test]
fn a_quoted_field_value_is_colored_inside_its_quotes() {
    assert_eq!(field(r#"C="250 OK""#, TokenKind::Message), "250 OK");
}

#[test]
fn a_field_with_no_value_colors_only_its_key() {
    assert_eq!(
        fields("relay="),
        vec![
            Token::new("relay", TokenKind::Header),
            Token::new("=", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_one_character_value_that_opens_a_quote_is_kept_as_it_is() {
    assert_eq!(field(r#"note=""#, TokenKind::Message), "\"");
}

#[test]
fn a_field_value_ends_at_a_comma() {
    assert_eq!(field("size=4512, nrcpt=1", TokenKind::Size), "4512");
}

#[test]
fn a_host_value_colors_the_host_name_and_its_bracketed_address_separately() {
    let text = "relay=mx.example.org[93.184.216.34]:25";

    assert_eq!(field(text, TokenKind::Host), "mx.example.org");
    assert_eq!(field(text, TokenKind::Ip), "93.184.216.34");
}

#[test]
fn a_host_value_keeps_the_port_after_its_address() {
    assert_eq!(
        joined("relay=mx.example.org[93.184.216.34]:25"),
        "relay=mx.example.org[93.184.216.34]:25"
    );
}

#[test]
fn a_host_value_with_only_a_bracketed_address_has_no_host_name() {
    assert!(!fields("H=[10.0.0.5]")
        .iter()
        .any(|token| token.kind == TokenKind::Host));
}

#[test]
fn a_host_value_with_no_address_is_colored_as_a_host() {
    assert_eq!(field("relay=none", TokenKind::Host), "none");
}

#[test]
fn a_status_value_is_colored_by_whether_the_mail_went_through() {
    assert_eq!(field("status=sent", TokenKind::Success), "sent");
}

#[test]
fn a_field_splash_has_no_style_for_keeps_its_value_as_message_text() {
    assert_eq!(field("commands=5", TokenKind::Message), "5");
}

#[test]
fn a_field_value_splash_has_no_style_for_colors_an_address_inside_it() {
    assert_eq!(field("note=10.0.0.9", TokenKind::Ip), "10.0.0.9");
}

#[test]
fn an_address_in_angle_brackets_is_colored_as_an_email() {
    assert_eq!(
        field("554 5.7.1 <erin@example.org>: denied", TokenKind::Email),
        "erin@example.org"
    );
}

#[test]
fn a_bare_address_is_colored_as_an_email() {
    assert_eq!(
        field("<= alice@example.com H=x", TokenKind::Email),
        "alice@example.com"
    );
}

#[test]
fn the_text_between_fields_is_message_text() {
    assert_eq!(
        field("connect from unknown", TokenKind::Message),
        "connect from unknown"
    );
}

#[test]
fn an_address_in_the_text_between_fields_is_colored() {
    assert_eq!(
        field("connect from unknown[10.0.0.5]", TokenKind::Ip),
        "10.0.0.5"
    );
}

#[test]
fn a_plugin_word_in_the_text_is_colored_by_what_it_says() {
    assert_eq!(field("message 1 flushed", TokenKind::Success), "flushed");
}

#[test]
fn a_longer_plugin_word_wins_over_a_shorter_one_inside_it() {
    assert_eq!(
        field("message 1 not flushed", TokenKind::Warning),
        "not flushed"
    );
}

#[test]
fn a_plugin_word_inside_a_longer_word_is_not_colored() {
    assert!(!fields("unflushedness")
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_scanner_keeps_every_character_of_the_text() {
    let text = "to=<bob@example.org>, relay=mx.example.org[93.184.216.34]:25, status=sent (250 OK)";

    assert_eq!(joined(text), text);
}

#[test]
fn plugin_words_alone_are_colored_without_reading_fields() {
    let mut tokens = Vec::new();
    mail::push_words(&mut tokens, "size=1 flushed", &WORDS);

    assert_eq!(
        tokens,
        vec![
            Token::new("size=1 ", TokenKind::Message),
            Token::new("flushed", TokenKind::Success),
        ]
    );
}

#[test]
fn address_fields_are_colored_as_email() {
    for key in ["from", "to", "orig_to", "F", "sender", "recipient"] {
        assert_eq!(
            mail::field_value(key),
            FieldValue::Kind(TokenKind::Email),
            "{}",
            key
        );
    }
}

#[test]
fn host_fields_are_colored_as_hosts() {
    for key in ["relay", "client", "helo", "H", "host"] {
        assert_eq!(mail::field_value(key), FieldValue::Host, "{}", key);
    }
}

#[test]
fn the_status_field_is_colored_by_its_outcome() {
    assert_eq!(mail::field_value("status"), FieldValue::Outcome);
}

#[test]
fn fields_holding_one_kind_of_value_are_colored_as_that_kind() {
    let expected = [
        ("size", TokenKind::Size),
        ("S", TokenKind::Size),
        ("in", TokenKind::Size),
        ("out", TokenKind::Size),
        ("delay", TokenKind::Duration),
        ("DT", TokenKind::Duration),
        ("QT", TokenKind::Duration),
        ("delays", TokenKind::Timers),
        ("dsn", TokenKind::Status),
        ("proto", TokenKind::Protocol),
        ("P", TokenKind::Protocol),
        ("X", TokenKind::Protocol),
        ("method", TokenKind::Protocol),
        ("sasl_method", TokenKind::Protocol),
        ("user", TokenKind::UserId),
        ("U", TokenKind::UserId),
        ("uid", TokenKind::UserId),
        ("sasl_username", TokenKind::UserId),
        ("message-id", TokenKind::Transaction),
        ("msgid", TokenKind::Transaction),
        ("id", TokenKind::Transaction),
        ("session", TokenKind::Transaction),
        ("pid", TokenKind::Pid),
        ("mpid", TokenKind::Pid),
        ("rip", TokenKind::Ip),
        ("lip", TokenKind::Ip),
        ("R", TokenKind::Module),
        ("T", TokenKind::Module),
        ("nrcpt", TokenKind::Number),
    ];

    for (key, kind) in expected {
        assert_eq!(mail::field_value(key), FieldValue::Kind(kind), "{}", key);
    }
}

#[test]
fn an_unknown_field_is_colored_as_message_text() {
    assert_eq!(mail::field_value("commands"), FieldValue::Message);
}

#[test]
fn delivered_mail_is_a_success() {
    for status in ["sent", "delivered", "deliverable"] {
        assert_eq!(mail::outcome_kind(status), TokenKind::Success, "{}", status);
    }
}

#[test]
fn deferred_mail_is_a_warning() {
    assert_eq!(mail::outcome_kind("deferred"), TokenKind::Warning);
}

#[test]
fn mail_that_will_not_be_delivered_is_a_failure() {
    for status in ["bounced", "expired", "undeliverable", "failed"] {
        assert_eq!(mail::outcome_kind(status), TokenKind::Failure, "{}", status);
    }
}

#[test]
fn an_unknown_status_is_message_text() {
    assert_eq!(mail::outcome_kind("3"), TokenKind::Message);
}

#[test]
fn a_word_list_reports_whether_text_holds_one_of_its_words() {
    assert!(WORDS.is_match("all flushed"));
    assert!(!WORDS.is_match("all done"));
}
