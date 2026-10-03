use splash::ci::{self, CiPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const FINISHED: &str = "Finished: FAILURE";

const SERVER: &str = "2023-10-03 12:00:03.345+0000 [id=44]\tSEVERE\thudson.triggers.SafeTimerTask#run: Timer task failed";

const GROUP: &str = "2023-10-03T12:00:01.1234567Z ##[group]Run actions/checkout@v4";

const COMMAND: &str = "::warning file=app.js,line=1::Missing semicolon";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    ci::parse_line(line)
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
    for line in [FINISHED, SERVER, GROUP, COMMAND] {
        assert_eq!(ci::parse_line(line).text(), line);
    }
}

#[test]
fn build_results_are_colored_by_how_the_build_went() {
    assert_eq!(field(FINISHED, TokenKind::Failure), "FAILURE");
    assert_eq!(field("Finished: SUCCESS", TokenKind::Success), "SUCCESS");
    assert_eq!(field("Finished: ABORTED", TokenKind::Warning), "ABORTED");
}

#[test]
fn a_server_log_line_colors_its_fields() {
    assert_eq!(
        field(SERVER, TokenKind::Timestamp),
        "2023-10-03 12:00:03.345+0000"
    );
    assert_eq!(field(SERVER, TokenKind::Pid), "44");
    assert_eq!(
        field(SERVER, TokenKind::Module),
        "hudson.triggers.SafeTimerTask#run"
    );
    assert_eq!(all(SERVER, TokenKind::Failure), vec!["SEVERE", "failed"]);
}

#[test]
fn server_log_levels_are_colored_by_how_severe_they_are() {
    let warning = "2023-10-03 12:00:02.234+0000 [id=43]\tWARNING\tx#y: z";
    let info = "2023-10-03 12:00:02.234+0000 [id=43]\tINFO\tx#y: z";

    assert_eq!(field(warning, TokenKind::Warning), "WARNING");
    assert_eq!(field(info, TokenKind::Level), "INFO");
}

#[test]
fn a_github_actions_group_colors_its_time_and_marker() {
    assert_eq!(
        field(GROUP, TokenKind::Timestamp),
        "2023-10-03T12:00:01.1234567Z"
    );
    assert_eq!(field(GROUP, TokenKind::Header), "group");
}

#[test]
fn github_actions_markers_are_colored_by_what_they_report() {
    assert_eq!(
        field(
            "##[error]Process completed with exit code 1.",
            TokenKind::Failure
        ),
        "error"
    );
    assert_eq!(
        field("##[warning]deprecated", TokenKind::Warning),
        "warning"
    );
    assert_eq!(field("##[debug]evaluating", TokenKind::Level), "debug");
}

#[test]
fn a_workflow_command_colors_its_kind_and_parameters() {
    assert_eq!(field(COMMAND, TokenKind::Warning), "warning");
    assert_eq!(field(COMMAND, TokenKind::Path), " file=app.js,line=1");
}

#[test]
fn a_workflow_command_without_parameters_is_read() {
    assert_eq!(field("::error::Missing token", TokenKind::Failure), "error");
}

#[test]
fn a_pipeline_step_is_colored() {
    assert_eq!(field("[Pipeline] sh", TokenKind::Tag), "Pipeline");
    assert_eq!(field("[Pipeline] sh", TokenKind::Module), "sh");
}

#[test]
fn a_shell_command_is_colored_as_a_request() {
    assert_eq!(field("+ make test", TokenKind::Request), "make test");
}

#[test]
fn a_build_start_is_colored_as_a_header() {
    assert_eq!(
        field("Started by user alice", TokenKind::Header),
        "Started by"
    );
}

#[test]
fn a_timestamper_time_is_colored() {
    assert_eq!(
        field("[2023-10-03T12:00:01.123Z] + make", TokenKind::Timestamp),
        "2023-10-03T12:00:01.123Z"
    );
    assert_eq!(field("12:00:01 + make", TokenKind::Timestamp), "12:00:01");
}

#[test]
fn build_output_colors_the_words_that_report_a_problem() {
    assert_eq!(
        field("FAILED tests/order_test.go", TokenKind::Failure),
        "FAILED"
    );
}

#[test]
fn every_line_parses() {
    assert_eq!(
        ci::parse_line("the maintenance window moves to 02:00").text(),
        "the maintenance window moves to 02:00"
    );
}

#[test]
fn a_line_with_a_ci_marker_is_marked() {
    assert!(ci::is_marked(GROUP));
    assert!(!ci::is_marked("compiling main.c"));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(CiPlugin::new().name(), "ci");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(CiPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        CiPlugin::default().metadata().description,
        "Jenkins and GitHub Actions build logs"
    );
}

#[test]
fn the_plugin_parses_every_line_into_tokens() {
    assert!(matches!(
        CiPlugin::new().parse_line("compiling main.c"),
        ParseResult::Parsed(_)
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_whose_lines_carry_ci_markers() {
    assert_eq!(
        CiPlugin::new().detect_format(&[FINISHED, SERVER, GROUP, COMMAND]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_lines_without_ci_markers() {
    assert_eq!(
        CiPlugin::new().detect_format(&["compiling", "linking"]),
        0.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_an_empty_sample() {
    assert_eq!(CiPlugin::new().detect_format(&[]), 0.0);
}
