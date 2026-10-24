use splash::container;
use splash::output::{Token, TokenKind};
use splash::syslog;

const ACCESS: &str = r#"10.0.0.5 - - [03/Oct/2023:12:00:01 +0000] "GET /index.html HTTP/1.1" 200 612 "-" "curl/8.4.0""#;

fn kinds(tokens: &[Token]) -> Vec<TokenKind> {
    tokens.iter().map(|token| token.kind).collect()
}

#[test]
fn a_container_line_in_a_known_format_is_colored_in_that_format() {
    let parsed = container::parse_output(ACCESS);

    assert_eq!(parsed.text(), ACCESS);
    assert_eq!(parsed.tokens[0].kind, TokenKind::Client);
}

#[test]
fn a_container_line_in_no_known_format_is_kept_as_free_text() {
    let parsed = container::parse_output("the maintenance window moves to 02:00");

    assert_eq!(parsed.text(), "the maintenance window moves to 02:00");
    assert_eq!(kinds(&parsed.tokens), [TokenKind::Message]);
}

#[test]
fn a_blank_container_line_is_kept() {
    assert_eq!(container::parse_output("   ").text(), "   ");
}

#[test]
fn a_timestamped_line_colors_the_time_then_the_rest() {
    let mut tokens = Vec::new();
    let line = "2023-10-03T12:00:01.123456789Z listening on :8080";

    let pushed = container::push_timestamped(&mut tokens, line, syslog::push_text);

    assert!(pushed.is_some());
    assert_eq!(
        kinds(&tokens),
        [TokenKind::Timestamp, TokenKind::Plain, TokenKind::Message]
    );
    assert_eq!(tokens[0].text, "2023-10-03T12:00:01.123456789Z");
}

#[test]
fn a_time_with_an_offset_is_read_as_a_timestamp() {
    let mut tokens = Vec::new();

    container::push_timestamped(
        &mut tokens,
        "2023-10-03T12:00:01-05:00 ready",
        syslog::push_text,
    );

    assert_eq!(tokens[0].text, "2023-10-03T12:00:01-05:00");
}

#[test]
fn a_line_without_a_leading_time_adds_nothing() {
    let mut tokens = Vec::new();

    let pushed = container::push_timestamped(&mut tokens, "listening on :8080", syslog::push_text);

    assert!(pushed.is_none());
    assert!(tokens.is_empty());
}
