use splash::icecast::{self, IcecastPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const STARTED: &str = "[2023-10-03  12:00:00] INFO main/main Icecast 2.4.4 server started";

const ACCESS: &str = r#"10.0.0.5 - - [03/Oct/2023:12:00:01 +0000] "GET /live.mp3 HTTP/1.1" 200 3456789 "-" "VLC/3.0.18 LibVLC/3.0.18" 3600"#;

const OLD: &str =
    "[03/Oct/2023:12:00:01] Admin [3:Connection Handler] Accepted admin from 10.0.0.30";

const USAGE: &str = "[03/Oct/2023:12:05:00] [2:Bandwidth Calculator] [03/Oct/2023:12:05:00] Bandwidth:128.50KB/s Sources:1 Clients:42 Admins:1";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    icecast::parse_line(line)
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
    for line in [STARTED, ACCESS, OLD, USAGE] {
        assert_eq!(icecast::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_error_log_line_colors_its_date_level_and_module() {
    assert_eq!(field(STARTED, TokenKind::Timestamp), "2023-10-03  12:00:00");
    assert_eq!(field(STARTED, TokenKind::Level), "INFO");
    assert_eq!(field(STARTED, TokenKind::Module), "main/main");
    assert_eq!(field(STARTED, TokenKind::Success), "server started");
}

#[test]
fn error_log_levels_are_colored_by_how_severe_they_are() {
    let error = "[2023-10-03  12:00:03] EROR sock/sock_write_bytes_wrapper Failed to write";
    let warning = "[2023-10-03  12:00:02] WARN source/get_next_buffer Disconnecting source";

    assert_eq!(all(error, TokenKind::Failure), vec!["EROR", "Failed"]);
    assert_eq!(
        all(warning, TokenKind::Warning),
        vec!["WARN", "Disconnecting"]
    );
}

#[test]
fn an_access_log_line_colors_how_long_the_listener_stayed() {
    assert_eq!(field(ACCESS, TokenKind::Duration), "3600");
}

#[test]
fn an_icecast_1_line_colors_the_admin_mark_and_thread() {
    assert_eq!(field(OLD, TokenKind::Tag), "Admin");
    assert_eq!(field(OLD, TokenKind::Number), "3");
    assert_eq!(field(OLD, TokenKind::Module), "Connection Handler");
}

#[test]
fn an_icecast_1_admin_mark_without_spacing_is_read() {
    let line = "[03/Oct/2023:12:00:01] Admin[3:Connection Handler] Accepted admin";

    assert_eq!(icecast::parse_line(line).unwrap().text(), line);
}

#[test]
fn an_icecast_1_thread_without_a_number_is_read() {
    let line = "[03/Oct/2023:12:00:00] [main] Icecast Version 1.3.12 Starting..";

    assert_eq!(icecast::parse_line(line).unwrap().text(), line);
    assert_eq!(field(line, TokenKind::Module), "main");
}

#[test]
fn an_icecast_1_thread_without_a_name_is_read() {
    let line = "[03/Oct/2023:12:00:00] [1:] starting";

    assert_eq!(icecast::parse_line(line).unwrap().text(), line);
    assert!(all(line, TokenKind::Module).is_empty());
}

#[test]
fn a_usage_line_colors_the_bandwidth_and_counts() {
    assert_eq!(
        all(USAGE, TokenKind::Header),
        vec!["Bandwidth", "Sources", "Clients", "Admins"]
    );
    assert_eq!(
        all(USAGE, TokenKind::Number),
        vec!["2", "128.50", "1", "42", "1"]
    );
}

#[test]
fn a_usage_line_without_a_unit_is_read() {
    let line = "[03/Oct/2023:12:05:00] [2:Bandwidth Calculator] [03/Oct/2023:12:05:00] Bandwidth:0 Sources:0 Clients:0 Admins:0";

    assert_eq!(icecast::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(icecast::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(IcecastPlugin::new().name(), "icecast");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(IcecastPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        IcecastPlugin::default().metadata().description,
        "Icecast streaming server logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match IcecastPlugin::new().parse_line(STARTED) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), STARTED);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        IcecastPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        IcecastPlugin::new().detect_format(&[STARTED, ACCESS, OLD, USAGE]),
        1.0
    );
}
