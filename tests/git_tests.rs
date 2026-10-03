use splash::git::{self, GitPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const CONNECTION: &str = "Oct  3 12:00:01 git01 git-daemon[3101]: Connection from 10.0.0.5:52144";

const REQUEST: &str =
    "Oct  3 12:00:01 git01 git-daemon[3101]: Request upload-pack for '/srv/git/app.git'";

const STDERR: &str = "[3000] [3101] Disconnected (with error)";

const TRACE: &str = "12:00:01.123456 git.c:463               trace: built-in: git fetch origin";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    git::parse_line(line)
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
    for line in [CONNECTION, REQUEST, STDERR, TRACE] {
        assert_eq!(git::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_connection_colors_the_client_address_and_port() {
    assert_eq!(field(CONNECTION, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(CONNECTION, TokenKind::Number), "52144");
}

#[test]
fn an_ipv6_client_is_colored_as_an_address() {
    let line = "[3101] Connection from [2001:db8::5]:52144";

    assert_eq!(field(line, TokenKind::Ip), "[2001:db8::5]");
}

#[test]
fn a_request_colors_the_service_and_repository() {
    assert_eq!(field(REQUEST, TokenKind::Method), "upload-pack");
    assert_eq!(field(REQUEST, TokenKind::Path), "/srv/git/app.git");
}

#[test]
fn a_quoted_attribute_is_message_text() {
    let line = r#"[3101] Extended attribute "host": git.example.com"#;

    assert!(all(line, TokenKind::Message).contains(&"host".to_string()));
}

#[test]
fn empty_quotes_are_only_punctuation() {
    let line = r#"[3101] Extended attribute "": '' none"#;

    assert_eq!(git::parse_line(line).unwrap().text(), line);
    assert!(all(line, TokenKind::Path).is_empty());
}

#[test]
fn a_standard_error_line_colors_its_process_id() {
    assert_eq!(field(STDERR, TokenKind::Pid), "3000");
    assert_eq!(field(STDERR, TokenKind::Warning), "Disconnected");
    assert_eq!(field(STDERR, TokenKind::Failure), "with error");
}

#[test]
fn a_refused_repository_is_colored_as_a_failure() {
    let line =
        "Oct  3 12:00:02 git01 git-daemon[3102]: '/srv/git/secret.git': repository not exported.";

    assert_eq!(field(line, TokenKind::Failure), "repository not exported");
}

#[test]
fn a_line_from_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 git01 sshd[4101]: Accepted password for alice";

    assert!(git::parse_line(line).is_none());
}

#[test]
fn a_trace_line_colors_its_time_file_and_line() {
    assert_eq!(field(TRACE, TokenKind::Timestamp), "12:00:01.123456");
    assert_eq!(field(TRACE, TokenKind::Path), "git.c");
    assert_eq!(field(TRACE, TokenKind::Number), "463");
}

#[test]
fn a_trace_line_colors_what_git_ran() {
    assert_eq!(field(TRACE, TokenKind::Module), "built-in");
    assert_eq!(field(TRACE, TokenKind::Request), "git fetch origin");
}

#[test]
fn a_trace_line_with_nothing_after_its_label_has_no_request() {
    let line = "12:00:01.123456 git.c:463               trace: built-in: ";

    assert!(all(line, TokenKind::Request).is_empty());
}

#[test]
fn a_trace_line_that_is_not_a_command_is_message_text() {
    let line = "12:00:01.123456 pkt-line.c:80           packet: fetch< 0000";

    assert_eq!(field(line, TokenKind::Message), "packet: fetch< 0000");
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(git::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        GitPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(GitPlugin::new().name(), "git");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(GitPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        GitPlugin::default().metadata().description,
        "git daemon and Git trace logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match GitPlugin::new().parse_line(CONNECTION) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), CONNECTION);
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        GitPlugin::new().detect_format(&[CONNECTION, REQUEST, STDERR, TRACE]),
        1.0
    );
}
