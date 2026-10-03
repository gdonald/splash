use splash::mongodb::{self, MongodbPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const ACCEPTED: &str = r#"{"t":{"$date":"2023-10-03T12:00:02.234+00:00"},"s":"I",  "c":"NETWORK",  "id":22943,   "ctx":"listener","msg":"Connection accepted","attr":{"remote":"10.0.0.5:52144","connectionId":12}}"#;

const AUTH_FAILED: &str = r#"{"t":{"$date":"2023-10-03T12:00:04.456+00:00"},"s":"E","c":"ACCESS","id":20249,"ctx":"conn13","msg":"Authentication failed","attr":{"user":"app","db":"admin","error":"AuthenticationFailed"}}"#;

const TEXT: &str =
    "2019-10-03T12:00:01.123+0000 I  NETWORK  [listener] connection accepted from 10.0.0.5:52144";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    mongodb::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .filter(|token| token.kind == kind)
        .map(|token| token.text.to_string())
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    all(line, kind)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
}

#[test]
fn a_line_keeps_every_character_of_the_original() {
    for line in [ACCEPTED, AUTH_FAILED, TEXT] {
        assert_eq!(mongodb::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_structured_line_colors_its_date_component_and_context() {
    assert_eq!(
        field(ACCEPTED, TokenKind::Timestamp),
        "2023-10-03T12:00:02.234+00:00"
    );
    assert_eq!(field(ACCEPTED, TokenKind::Module), "NETWORK");
    assert_eq!(field(ACCEPTED, TokenKind::Transaction), "22943");
    assert_eq!(field(ACCEPTED, TokenKind::Tag), "listener");
}

#[test]
fn a_structured_line_colors_its_message_and_remote_address() {
    assert_eq!(field(ACCEPTED, TokenKind::Message), "Connection accepted");
    assert_eq!(field(ACCEPTED, TokenKind::Host), "10.0.0.5:52144");
}

#[test]
fn an_error_colors_its_severity_and_error_as_failures() {
    assert_eq!(
        all(AUTH_FAILED, TokenKind::Failure),
        vec!["E", "AuthenticationFailed"]
    );
    assert_eq!(field(AUTH_FAILED, TokenKind::UserId), "app");
}

#[test]
fn severities_are_colored_by_how_severe_they_are() {
    assert_eq!(mongodb::severity_kind("F"), TokenKind::Failure);
    assert_eq!(mongodb::severity_kind("W"), TokenKind::Warning);
    assert_eq!(mongodb::severity_kind("I"), TokenKind::Level);
    assert_eq!(mongodb::severity_kind("D1"), TokenKind::Level);
}

#[test]
fn values_are_colored_by_their_keys() {
    assert_eq!(
        mongodb::value_kind("ns", "appdb.orders"),
        Some(TokenKind::Module)
    );
    assert_eq!(
        mongodb::value_kind("durationMillis", "2345"),
        Some(TokenKind::Duration)
    );
    assert_eq!(
        mongodb::value_kind("tags", "startupWarnings"),
        Some(TokenKind::Tag)
    );
    assert_eq!(mongodb::value_kind("port", "27017"), None);
}

#[test]
fn a_text_line_colors_its_fields() {
    assert_eq!(
        field(TEXT, TokenKind::Timestamp),
        "2019-10-03T12:00:01.123+0000"
    );
    assert_eq!(field(TEXT, TokenKind::Level), "I");
    assert_eq!(field(TEXT, TokenKind::Module), "NETWORK");
    assert_eq!(field(TEXT, TokenKind::Tag), "listener");
    assert_eq!(field(TEXT, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_text_line_with_an_empty_context_is_read() {
    let line = "2019-10-03T12:00:01.123+0000 E  STORAGE  [] Failed to open journal";

    assert_eq!(mongodb::parse_line(line).unwrap().text(), line);
    assert!(all(line, TokenKind::Tag).is_empty());
    assert_eq!(all(line, TokenKind::Failure), vec!["E", "Failed"]);
}

#[test]
fn a_line_that_is_not_a_mongodb_log_is_not_parsed() {
    assert!(mongodb::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(MongodbPlugin::new().name(), "mongodb");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(MongodbPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        MongodbPlugin::default().metadata().description,
        "MongoDB structured and text logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match MongodbPlugin::new().parse_line(ACCEPTED) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), ACCEPTED);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        MongodbPlugin::new().parse_line("not a mongodb log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        MongodbPlugin::new().detect_format(&[ACCEPTED, AUTH_FAILED, TEXT]),
        1.0
    );
}
