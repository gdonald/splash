use splash::docker::{self, DockerPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const JSON_FILE: &str = r#"{"log":"error: connection refused\n","stream":"stderr","time":"2023-10-03T12:00:02.234567890Z"}"#;

const DAEMON: &str =
    r#"time="2023-10-03T12:00:02.234567890Z" level=error msg="No such image: app:missing""#;

const SYSLOG: &str = r#"Oct  3 12:00:03 web01 dockerd[900]: time="2023-10-03T12:00:03.345678901Z" level=info msg="ignoring event" container=4f2a1c0123ab module=libcontainerd"#;

const TIMESTAMPED: &str = "2023-10-03T12:00:01.123456789Z listening on 10.0.0.5:8080";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    docker::parse_line(line)
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
    for line in [JSON_FILE, DAEMON, SYSLOG, TIMESTAMPED] {
        assert_eq!(docker::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_json_file_line_colors_the_line_stream_and_time() {
    assert_eq!(
        field(JSON_FILE, TokenKind::Message),
        r"error: connection refused\n"
    );
    assert_eq!(field(JSON_FILE, TokenKind::Warning), "stderr");
    assert_eq!(
        field(JSON_FILE, TokenKind::Timestamp),
        "2023-10-03T12:00:02.234567890Z"
    );
}

#[test]
fn standard_output_is_colored_as_a_level() {
    assert_eq!(
        docker::json_value_kind("stream", "stdout"),
        Some(TokenKind::Level)
    );
    assert_eq!(docker::json_value_kind("attrs", "web"), None);
}

#[test]
fn a_daemon_line_colors_its_level_by_how_severe_it_is() {
    assert_eq!(field(DAEMON, TokenKind::Failure), "error");
}

#[test]
fn a_daemon_line_from_syslog_colors_the_container_and_module() {
    assert_eq!(field(SYSLOG, TokenKind::Tag), "dockerd");
    assert_eq!(field(SYSLOG, TokenKind::Transaction), "4f2a1c0123ab");
    assert_eq!(field(SYSLOG, TokenKind::Module), "libcontainerd");
}

#[test]
fn daemon_values_are_colored_by_their_keys() {
    assert_eq!(
        docker::logfmt_value_kind("err", "x"),
        Some(TokenKind::Failure)
    );
    assert_eq!(docker::logfmt_value_kind("topic", "/tasks"), None);
}

#[test]
fn a_syslog_line_from_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice";

    assert!(docker::parse_line(line).is_none());
}

#[test]
fn a_syslog_line_from_dockerd_without_logfmt_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 dockerd[900]: plain message";

    assert!(docker::parse_line(line).is_none());
}

#[test]
fn a_timestamped_line_colors_the_time_and_any_address() {
    assert_eq!(
        field(TIMESTAMPED, TokenKind::Timestamp),
        "2023-10-03T12:00:01.123456789Z"
    );
    assert_eq!(field(TIMESTAMPED, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(docker::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        DockerPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(DockerPlugin::new().name(), "docker");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(DockerPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        DockerPlugin::default().metadata().description,
        "Docker container and daemon logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match DockerPlugin::new().parse_line(DAEMON) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), DAEMON);
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        DockerPlugin::new().detect_format(&[JSON_FILE, DAEMON, SYSLOG, TIMESTAMPED]),
        1.0
    );
}
