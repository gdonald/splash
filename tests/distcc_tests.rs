use splash::distcc::{self, DistccPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const SUMMARY: &str = "distccd[5101] (dcc_job_summary) client: 10.0.0.5:52144 COMPILE_OK exit:0 sig:0 core:0 ret:0 time:1234ms gcc main.c";

const ERROR: &str = "distccd[5103] (dcc_r_token_int) ERROR: read failed: Connection reset by peer";

const SYSLOG: &str = "Oct  3 12:00:01 build01 distccd[5104]: (dcc_job_summary) client: 10.0.0.7:52160 COMPILE_TIMEOUT exit:0 sig:9 core:0 ret:0 time:300000ms gcc slow.c";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    distcc::parse_line(line)
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
    for line in [SUMMARY, ERROR, SYSLOG] {
        assert_eq!(distcc::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_log_file_line_colors_the_program_process_and_function() {
    assert_eq!(field(SUMMARY, TokenKind::Tag), "distccd");
    assert_eq!(field(SUMMARY, TokenKind::Pid), "5101");
    assert_eq!(field(SUMMARY, TokenKind::Module), "dcc_job_summary");
}

#[test]
fn a_summary_colors_the_client_address_and_port() {
    assert_eq!(field(SUMMARY, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(SUMMARY, TokenKind::Number), "52144");
}

#[test]
fn a_summary_colors_its_fields_and_time() {
    assert_eq!(
        all(SUMMARY, TokenKind::Header),
        vec!["exit", "sig", "core", "ret", "time"]
    );
    assert_eq!(all(SUMMARY, TokenKind::Duration), vec!["1234", "ms"]);
}

#[test]
fn a_finished_compile_is_colored_as_a_success() {
    assert_eq!(field(SUMMARY, TokenKind::Success), "COMPILE_OK");
}

#[test]
fn a_timed_out_compile_is_colored_as_a_failure() {
    assert_eq!(field(SYSLOG, TokenKind::Failure), "COMPILE_TIMEOUT");
}

#[test]
fn an_error_is_colored_as_a_failure() {
    assert_eq!(field(ERROR, TokenKind::Failure), "ERROR");
}

#[test]
fn a_line_without_a_function_is_read() {
    let line = "distccd[5100] listening";

    assert!(all(line, TokenKind::Module).is_empty());
}

#[test]
fn a_line_from_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice";

    assert!(distcc::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(distcc::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(DistccPlugin::new().name(), "distcc");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(DistccPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        DistccPlugin::default().metadata().description,
        "distccd distributed compilation logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match DistccPlugin::new().parse_line(SUMMARY) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), SUMMARY);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        DistccPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        DistccPlugin::new().detect_format(&[SUMMARY, ERROR, SYSLOG]),
        1.0
    );
}
