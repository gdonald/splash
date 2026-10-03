use splash::kubernetes::{self, KubernetesPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const PLAIN: &str = "I1003 12:00:01.123456       1 controller.go:123] Starting endpoint controller";

const STRUCTURED: &str = r#"E1003 12:00:03.345678    1234 pod_workers.go:965] "Error syncing pod, skipping" err="failed" pod="default/web-0" podUID="0f1e2d3c""#;

const SYSLOG: &str = r#"Oct  3 12:00:05 node01 kubelet[1234]: I1003 12:00:05.567890    1234 kubelet_node_status.go:70] "Attempting to register node" node="node01""#;

const CRI: &str =
    "2023-10-03T12:00:02.234567890Z stderr P panic: runtime error: index out of range";

const CRI_KLOG: &str = "2023-10-03T12:00:03.345678901Z stderr F W1003 12:00:03.345678       1 leaderelection.go:250] lost lease";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    kubernetes::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    kubernetes::parse_line(line)
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
    for line in [PLAIN, STRUCTURED, SYSLOG, CRI, CRI_KLOG] {
        assert_eq!(kubernetes::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_klog_header_colors_its_fields() {
    assert_eq!(field(PLAIN, TokenKind::Level), "I");
    assert_eq!(field(PLAIN, TokenKind::Timestamp), "1003 12:00:01.123456");
    assert_eq!(field(PLAIN, TokenKind::Pid), "1");
    assert_eq!(field(PLAIN, TokenKind::Path), "controller.go");
    assert_eq!(field(PLAIN, TokenKind::Number), "123");
}

#[test]
fn severities_are_colored_by_how_severe_they_are() {
    assert_eq!(kubernetes::severity_kind("F"), TokenKind::Failure);
    assert_eq!(kubernetes::severity_kind("W"), TokenKind::Warning);
}

#[test]
fn a_structured_message_colors_its_message_and_pairs() {
    assert_eq!(
        field(STRUCTURED, TokenKind::Message),
        "Error syncing pod, skipping"
    );
    assert_eq!(all(STRUCTURED, TokenKind::Failure), vec!["E", "failed"]);
    assert_eq!(field(STRUCTURED, TokenKind::Module), "default/web-0");
    assert_eq!(field(STRUCTURED, TokenKind::Transaction), "0f1e2d3c");
}

#[test]
fn an_empty_structured_message_is_only_its_quotes() {
    let line = r#"I1003 12:00:01.123456       1 a.go:1] """#;

    assert_eq!(kubernetes::parse_line(line).unwrap().text(), line);
    assert!(!kinds(line).contains(&TokenKind::Message));
}

#[test]
fn a_duration_pair_is_colored_as_a_duration() {
    assert_eq!(
        kubernetes::value_kind("latency", "2ms"),
        Some(TokenKind::Duration)
    );
}

#[test]
fn a_klog_line_from_syslog_colors_both_headers() {
    assert_eq!(field(SYSLOG, TokenKind::Tag), "kubelet");
    assert_eq!(all(SYSLOG, TokenKind::Pid), vec!["1234", "1234"]);
}

#[test]
fn a_syslog_line_without_a_klog_header_is_not_parsed() {
    let line = "Oct  3 12:00:01 node01 sshd[4101]: Accepted password for alice";

    assert!(kubernetes::parse_line(line).is_none());
}

#[test]
fn a_cri_line_colors_its_stream_and_tag() {
    assert_eq!(field(CRI, TokenKind::Warning), "stderr");
    assert_eq!(field(CRI, TokenKind::Tag), "P");
}

#[test]
fn a_cri_line_on_standard_output_is_colored_as_a_level() {
    let line = "2023-10-03T12:00:01.123456789Z stdout F listening";

    assert_eq!(field(line, TokenKind::Level), "stdout");
}

#[test]
fn a_cri_line_holding_a_klog_line_colors_it() {
    assert_eq!(all(CRI_KLOG, TokenKind::Warning), vec!["stderr", "W"]);
    assert_eq!(field(CRI_KLOG, TokenKind::Path), "leaderelection.go");
}

#[test]
fn an_empty_cri_line_is_only_its_fields() {
    let line = "2023-10-03T12:00:04.456789012Z stdout F";

    assert_eq!(kubernetes::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(kubernetes::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        KubernetesPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(KubernetesPlugin::new().name(), "kubernetes");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(KubernetesPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        KubernetesPlugin::default().metadata().description,
        "Kubernetes component and container logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match KubernetesPlugin::new().parse_line(PLAIN) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), PLAIN);
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        KubernetesPlugin::new().detect_format(&[PLAIN, STRUCTURED, SYSLOG, CRI]),
        1.0
    );
}

#[test]
fn a_pair_without_a_style_is_message_text() {
    assert_eq!(kubernetes::value_kind("source", "api"), None);
}
