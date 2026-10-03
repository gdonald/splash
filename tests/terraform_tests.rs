use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::terraform::{self, TerraformPlugin};

const HCLOG: &str = "2023-10-03T12:00:02.234Z [DEBUG] provider.terraform-provider-aws_v5.0.0: HTTP Request Sent: tf_req_id=7c1f2b3a tf_rpc=ApplyResourceChange @caller=/src/provider.go:42";

const JSON: &str = r#"{"@level":"error","@message":"apply failed","@module":"terraform.ui","@timestamp":"2023-10-03T12:00:05Z","type":"apply_errored","hook":{"action":"create","elapsed_seconds":3}}"#;

const COMPLETE: &str = "aws_instance.web: Creation complete after 32s [id=i-0abc123]";

const SUMMARY: &str = "Apply complete! Resources: 1 added, 0 changed, 1 destroyed.";

const PLAN: &str = "  # aws_instance.web will be created";

const ERROR: &str = "│ Error: Invalid reference";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    terraform::parse_line(line)
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
    for line in [HCLOG, JSON, COMPLETE, SUMMARY, PLAN, ERROR] {
        assert_eq!(terraform::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_hclog_line_colors_its_time_level_and_logger() {
    assert_eq!(
        field(HCLOG, TokenKind::Timestamp),
        "2023-10-03T12:00:02.234Z"
    );
    assert_eq!(field(HCLOG, TokenKind::Level), "DEBUG");
    assert_eq!(
        field(HCLOG, TokenKind::Module),
        "provider.terraform-provider-aws_v5.0.0"
    );
}

#[test]
fn an_hclog_line_colors_its_pairs() {
    assert_eq!(
        all(HCLOG, TokenKind::Transaction),
        vec!["7c1f2b3a", "ApplyResourceChange"]
    );
    assert_eq!(field(HCLOG, TokenKind::Path), "/src/provider.go:42");
}

#[test]
fn an_hclog_line_without_a_logger_is_read() {
    let line = "2023-10-03T12:00:01.123Z [INFO]  Terraform version: 1.6.0";

    assert!(all(line, TokenKind::Module).is_empty());
}

#[test]
fn an_hclog_error_is_colored_as_a_failure() {
    let line = "2023-10-03T12:00:04.456Z [ERROR] vertex error: timeout";

    assert_eq!(all(line, TokenKind::Failure), vec!["ERROR", "error"]);
}

#[test]
fn a_json_line_colors_its_fields_by_key() {
    assert_eq!(field(JSON, TokenKind::Failure), "error");
    assert_eq!(field(JSON, TokenKind::Module), "terraform.ui");
    assert_eq!(field(JSON, TokenKind::Tag), "apply_errored");
    assert_eq!(field(JSON, TokenKind::Method), "create");
    assert_eq!(field(JSON, TokenKind::Duration), "3");
}

#[test]
fn json_values_are_colored_by_their_keys() {
    assert_eq!(
        terraform::json_value_kind("id_value", "i-0abc"),
        Some(TokenKind::Transaction)
    );
    assert_eq!(
        terraform::json_value_kind("severity", "warning"),
        Some(TokenKind::Warning)
    );
    assert_eq!(terraform::json_value_kind("summary", "x"), None);
}

#[test]
fn a_finished_operation_colors_its_resource_time_and_id() {
    assert_eq!(field(COMPLETE, TokenKind::Module), "aws_instance.web");
    assert_eq!(field(COMPLETE, TokenKind::Success), "Creation complete");
    assert_eq!(field(COMPLETE, TokenKind::Duration), "32s");
    assert_eq!(field(COMPLETE, TokenKind::Transaction), "i-0abc123");
}

#[test]
fn an_operation_in_progress_colors_how_long_it_has_run() {
    let line = "aws_instance.web: Still creating... [10s elapsed]";

    assert_eq!(field(line, TokenKind::Warning), "Still creating...");
    assert_eq!(field(line, TokenKind::Duration), "10s");
}

#[test]
fn an_errored_operation_is_colored_as_a_failure() {
    let line = "aws_instance.web: Creation errored after 5s";

    assert_eq!(field(line, TokenKind::Failure), "Creation errored");
}

#[test]
fn an_operation_with_an_empty_id_is_only_its_brackets() {
    let line = "aws_instance.web: Destroying... [id=]";

    assert_eq!(terraform::parse_line(line).unwrap().text(), line);
    assert!(all(line, TokenKind::Transaction).is_empty());
}

#[test]
fn a_summary_colors_its_counts() {
    assert_eq!(field(SUMMARY, TokenKind::Success), "Apply complete!");
    assert_eq!(all(SUMMARY, TokenKind::Number), vec!["1", "0", "1"]);
}

#[test]
fn a_plan_summary_is_colored_as_a_header() {
    assert_eq!(
        field(
            "Plan: 1 to add, 0 to change, 0 to destroy.",
            TokenKind::Header
        ),
        "Plan:"
    );
}

#[test]
fn a_summary_ending_in_a_count_is_read() {
    assert_eq!(all("Plan: 3", TokenKind::Number), vec!["3"]);
}

#[test]
fn plan_signs_are_colored_by_their_action() {
    assert_eq!(field("  + ami = \"ami-0abc\"", TokenKind::Success), "+");
    assert_eq!(field("  - monitoring = true", TokenKind::Failure), "-");
    assert_eq!(
        field(
            "-/+ resource \"aws_instance\" \"web\" {",
            TokenKind::Failure
        ),
        "-/+"
    );
    assert_eq!(field("  ~ tags = {", TokenKind::Warning), "~");
    assert_eq!(
        field(" <= data \"aws_ami\" \"base\" {", TokenKind::Level),
        "<="
    );
}

#[test]
fn a_plan_comment_colors_what_will_happen() {
    assert_eq!(field(PLAN, TokenKind::Success), "will be created");
}

#[test]
fn a_diagnostic_inside_its_box_is_colored() {
    assert_eq!(field(ERROR, TokenKind::Failure), "Error");
}

#[test]
fn a_warning_outside_a_box_is_colored() {
    assert_eq!(
        field("Warning: Deprecated attribute", TokenKind::Warning),
        "Warning"
    );
}

#[test]
fn a_box_border_alone_is_read() {
    assert_eq!(field("╷", TokenKind::Punctuation), "╷");
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(terraform::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        TerraformPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(TerraformPlugin::new().name(), "terraform");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(TerraformPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        TerraformPlugin::default().metadata().description,
        "Terraform logs, JSON output, and CLI output"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match TerraformPlugin::new().parse_line(COMPLETE) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), COMPLETE);
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        TerraformPlugin::new().detect_format(&[HCLOG, JSON, COMPLETE, SUMMARY, PLAN, ERROR]),
        1.0
    );
}

#[test]
fn an_hclog_line_colors_its_module_time_and_error_pairs() {
    let line = "2023-10-03T12:00:02.234Z [WARN]  plugin: retry: @module=aws timestamp=2023-10-03T12:00:02Z error=timeout attempt=2";

    assert_eq!(all(line, TokenKind::Module), vec!["plugin", "aws"]);
    assert_eq!(
        all(line, TokenKind::Timestamp),
        vec!["2023-10-03T12:00:02.234Z", "2023-10-03T12:00:02Z"]
    );
    assert_eq!(field(line, TokenKind::Failure), "timeout");
    assert!(all(line, TokenKind::Message).contains(&"2".to_string()));
}
