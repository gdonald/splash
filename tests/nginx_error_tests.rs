use splash::nginx_error::{self, NginxErrorPlugin};
use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};

const OPEN: &str = r#"2023/10/03 12:00:01 [error] 1234#5678: *42 open() "/usr/share/nginx/html/favicon.ico" failed (2: No such file or directory), client: 10.0.0.5, server: example.com, request: "GET /favicon.ico HTTP/1.1", host: "example.com", referrer: "https://example.com/""#;

const UPSTREAM: &str = r#"2023/10/03 12:00:02 [warn] 1234#5678: *43 upstream timed out (110: Connection timed out), client: 10.0.0.6, upstream: "http://127.0.0.1:3000/api", host: """#;

const NOTICE: &str = "2023/10/03 12:00:04 [notice] 1200#1200: signal process started";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    nginx_error::parse_line(line)
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
    for line in [OPEN, UPSTREAM, NOTICE] {
        assert_eq!(nginx_error::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_line_colors_its_date_level_process_and_connection() {
    assert_eq!(field(OPEN, TokenKind::Timestamp), "2023/10/03 12:00:01");
    assert_eq!(all(OPEN, TokenKind::Pid), vec!["1234", "5678"]);
    assert_eq!(field(OPEN, TokenKind::Transaction), "42");
}

#[test]
fn levels_are_colored_by_how_severe_they_are() {
    assert_eq!(field(OPEN, TokenKind::Failure), "error");
    assert_eq!(field(UPSTREAM, TokenKind::Warning), "warn");
    assert_eq!(field(NOTICE, TokenKind::Level), "notice");
}

#[test]
fn a_system_call_and_its_file_are_colored() {
    assert_eq!(field(OPEN, TokenKind::Module), "open()");
    assert_eq!(
        field(OPEN, TokenKind::Path),
        "/usr/share/nginx/html/favicon.ico"
    );
}

#[test]
fn an_error_number_and_its_text_are_colored() {
    assert_eq!(field(OPEN, TokenKind::Number), "2");
    assert_eq!(
        all(OPEN, TokenKind::Failure),
        vec!["error", "failed", "No such file or directory"]
    );
}

#[test]
fn the_request_context_is_colored_by_name() {
    assert_eq!(field(OPEN, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(OPEN, TokenKind::VirtualHost), "example.com");
    assert_eq!(field(OPEN, TokenKind::Request), "GET /favicon.ico HTTP/1.1");
    assert_eq!(field(OPEN, TokenKind::Host), "example.com");
    assert_eq!(field(OPEN, TokenKind::Referer), "https://example.com/");
}

#[test]
fn an_upstream_is_colored_as_a_request() {
    assert_eq!(
        field(UPSTREAM, TokenKind::Request),
        "http://127.0.0.1:3000/api"
    );
}

#[test]
fn an_empty_quoted_context_value_is_only_its_quotes() {
    assert_eq!(
        nginx_error::parse_line(UPSTREAM).unwrap().tokens.last(),
        Some(&Token::new("\"", TokenKind::Punctuation))
    );
}

#[test]
fn an_empty_bare_context_value_is_only_its_name() {
    let line = "2023/10/03 12:00:04 [error] 1200#1200: *1 denied, client: , server: x";

    assert_eq!(nginx_error::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_context_value_without_a_style_is_message_text() {
    let line = "2023/10/03 12:00:04 [error] 1200#1200: *1 denied, client: 10.0.0.5, login: alice";

    assert!(all(line, TokenKind::Message).contains(&"alice".to_string()));
}

#[test]
fn a_quoted_value_that_is_not_a_file_is_message_text() {
    let line = r#"2023/10/03 12:00:04 [error] 1200#1200: invalid "x" and """#;

    assert!(all(line, TokenKind::Message).contains(&"x".to_string()));
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(nginx_error::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        NginxErrorPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(NginxErrorPlugin::new().name(), "nginx-error");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(NginxErrorPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        NginxErrorPlugin::default().metadata().description,
        "nginx error logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match NginxErrorPlugin::new().parse_line(OPEN) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), OPEN);
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        NginxErrorPlugin::new().detect_format(&[OPEN, UPSTREAM, NOTICE]),
        1.0
    );
}
