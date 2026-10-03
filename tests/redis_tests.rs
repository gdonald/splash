use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::redis::{self, RedisPlugin};

const READY: &str = "1234:M 03 Oct 2023 12:00:00.123 * Ready to accept connections tcp";

const WARNING: &str =
    "1234:M 03 Oct 2023 12:06:00.000 # WARNING: The TCP backlog setting of 511 cannot be enforced";

const REPLICA: &str = "2345:S 03 Oct 2023 12:07:00.000 - Accepted 10.0.0.5:52144";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    redis::parse_line(line)
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
    for line in [READY, WARNING, REPLICA] {
        assert_eq!(redis::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_line_colors_its_process_role_and_date() {
    assert_eq!(field(READY, TokenKind::Pid), "1234");
    assert_eq!(field(READY, TokenKind::Module), "M");
    assert_eq!(
        field(READY, TokenKind::Timestamp),
        "03 Oct 2023 12:00:00.123"
    );
}

#[test]
fn a_notice_mark_is_colored_as_a_level() {
    assert_eq!(field(READY, TokenKind::Level), "*");
}

#[test]
fn a_warning_mark_and_word_are_colored_as_warnings() {
    assert_eq!(all(WARNING, TokenKind::Warning), vec!["#", "WARNING"]);
}

#[test]
fn readiness_is_colored_as_a_success() {
    assert_eq!(
        field(READY, TokenKind::Success),
        "Ready to accept connections"
    );
}

#[test]
fn an_address_in_a_message_is_colored() {
    assert_eq!(field(REPLICA, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_line_that_is_not_a_redis_log_is_not_parsed() {
    assert!(redis::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(RedisPlugin::new().name(), "redis");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(RedisPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        RedisPlugin::default().metadata().description,
        "Redis server logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match RedisPlugin::new().parse_line(READY) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), READY);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        RedisPlugin::new().parse_line("not a redis log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        RedisPlugin::new().detect_format(&[READY, WARNING, REPLICA]),
        1.0
    );
}
