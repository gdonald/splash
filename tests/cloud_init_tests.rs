use splash::cloud_init::{self, CloudInitPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const RUNNING: &str = "2023-10-03 12:00:01,123 - util.py[DEBUG]: Cloud-init v. 23.3.1 running 'init-local' at Tue, 03 Oct 2023 12:00:01 +0000. Up 5.12 seconds.";

const FINISH: &str = "2023-10-03 12:00:03,345 - handlers.py[DEBUG]: finish: init-network/config-ssh: SUCCESS: config-ssh ran successfully";

const SYSLOG: &str =
    "Oct  3 12:00:06 web01 cloud-init[1200]: [CLOUDINIT] util.py[WARNING]: Failed to set hostname";

const OUTPUT: &str = "ci-info: | eth0 | True | 10.0.0.11 | 255.255.255.0 | global |";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    cloud_init::parse_line(line)
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
    for line in [RUNNING, FINISH, SYSLOG, OUTPUT] {
        assert_eq!(cloud_init::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_log_line_colors_its_date_file_and_level() {
    assert_eq!(
        field(RUNNING, TokenKind::Timestamp),
        "2023-10-03 12:00:01,123"
    );
    assert_eq!(field(RUNNING, TokenKind::Path), "util.py");
    assert_eq!(field(RUNNING, TokenKind::Level), "DEBUG");
}

#[test]
fn a_stage_banner_colors_the_version_stage_and_uptime() {
    assert_eq!(field(RUNNING, TokenKind::Number), "23.3.1");
    assert_eq!(field(RUNNING, TokenKind::Module), "init-local");
    assert_eq!(field(RUNNING, TokenKind::Duration), "5.12");
}

#[test]
fn a_stage_event_colors_the_module_and_its_result() {
    assert_eq!(field(FINISH, TokenKind::Header), "finish");
    assert_eq!(field(FINISH, TokenKind::Module), "init-network/config-ssh");
    assert_eq!(
        all(FINISH, TokenKind::Success),
        vec!["SUCCESS", "ran successfully"]
    );
}

#[test]
fn levels_are_colored_by_how_severe_they_are() {
    assert_eq!(all(SYSLOG, TokenKind::Warning), vec!["WARNING"]);

    let error = "2023-10-03 12:00:05,567 - cc.py[ERROR]: Failed to run module";

    assert_eq!(all(error, TokenKind::Failure), vec!["ERROR", "Failed"]);
}

#[test]
fn a_syslog_line_colors_its_tag() {
    assert_eq!(all(SYSLOG, TokenKind::Tag), vec!["cloud-init", "CLOUDINIT"]);
}

#[test]
fn an_empty_quoted_stage_is_only_its_quotes() {
    let line = "Cloud-init v. 23.3.1 running '' at now";

    assert!(all(line, TokenKind::Module).is_empty());
}

#[test]
fn an_output_line_colors_any_address() {
    assert_eq!(field(OUTPUT, TokenKind::Ip), "10.0.0.11");
}

#[test]
fn a_syslog_line_without_the_tag_is_not_parsed() {
    let line = "Oct  3 12:00:06 web01 cloud-init[1200]: Reading package lists...";

    assert!(cloud_init::parse_line(line).is_none());
}

#[test]
fn a_dated_line_without_a_file_and_level_is_not_parsed() {
    assert!(cloud_init::parse_line("2023-10-03 12:00:01,123 - plain text").is_none());
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(cloud_init::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        CloudInitPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(CloudInitPlugin::new().name(), "cloud-init");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(CloudInitPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        CloudInitPlugin::default().metadata().description,
        "cloud-init instance initialization logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match CloudInitPlugin::new().parse_line(RUNNING) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), RUNNING);
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        CloudInitPlugin::new().detect_format(&[RUNNING, FINISH, SYSLOG, OUTPUT]),
        1.0
    );
}
