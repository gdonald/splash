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

const PREFIXED_KLOG: &str =
    "[pod/web-0/app] I1003 12:00:01.123456       1 controller.go:123] Starting endpoint controller";

const PREFIXED_TEXT: &str = "[pod/web-0/app] the maintenance window moves to 02:00";

const PREFIXED_TIMESTAMPED_ACCESS: &str = r#"[pod/web-7d4b9c6f5-abcde/nginx] 2023-10-03T12:00:04.123456789Z 10.0.0.7 - - [03/Oct/2023:12:00:04 +0000] "GET / HTTP/1.1" 200 612 "-" "curl/8.4.0""#;

const CRI_ACCESS: &str = r#"2023-10-03T12:00:02.234567890Z stdout F 10.0.0.5 - - [03/Oct/2023:12:00:02 +0000] "GET /healthz HTTP/1.1" 200 2 "-" "kube-probe/1.28""#;

fn display_kinds(line: &str) -> Vec<TokenKind> {
    kubernetes::parse_display_line(line)
        .tokens
        .iter()
        .map(|token| token.kind)
        .filter(|kind| *kind != TokenKind::Plain)
        .collect()
}

#[test]
fn a_prefixed_line_colors_the_pod_and_container_tag() {
    let parsed = kubernetes::parse_prefixed_line(PREFIXED_KLOG).unwrap();

    assert_eq!(parsed.text(), PREFIXED_KLOG);
    assert_eq!(parsed.tokens[0].text, "[pod/web-0/app]");
    assert_eq!(parsed.tokens[0].kind, TokenKind::Module);
    assert_eq!(parsed.tokens[2].kind, TokenKind::Level);
}

#[test]
fn a_prefixed_line_of_free_text_colors_the_text_as_a_message() {
    let parsed = kubernetes::parse_line(PREFIXED_TEXT).unwrap();

    assert_eq!(parsed.tokens[2].kind, TokenKind::Message);
}

#[test]
fn a_line_without_a_kubernetes_format_is_not_read() {
    assert!(kubernetes::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn a_line_kubectl_printed_is_shown_as_text() {
    assert_eq!(
        display_kinds("the maintenance window moves to 02:00"),
        [TokenKind::Message]
    );
}

#[test]
fn a_timestamped_klog_line_kubectl_printed_is_shown_as_klog() {
    let line = "2023-10-03T12:00:01.123456789Z W1003 12:00:01.123456       1 leaderelection.go:250] lost lease";

    assert_eq!(
        display_kinds(line)[..2],
        [TokenKind::Timestamp, TokenKind::Warning]
    );
}

#[test]
fn a_prefixed_timestamped_line_is_shown_in_the_container_format() {
    let kinds = display_kinds(PREFIXED_TIMESTAMPED_ACCESS);

    assert_eq!(
        kinds[..3],
        [TokenKind::Module, TokenKind::Timestamp, TokenKind::Client]
    );
    assert_eq!(
        kubernetes::parse_display_line(PREFIXED_TIMESTAMPED_ACCESS).text(),
        PREFIXED_TIMESTAMPED_ACCESS
    );
}

#[test]
fn a_cri_line_shows_the_container_line_in_its_own_format() {
    assert!(display_kinds(CRI_ACCESS).contains(&TokenKind::Client));
    assert!(!kinds(CRI_ACCESS).contains(&TokenKind::Client));
}

#[test]
fn a_cri_klog_line_is_shown_as_klog() {
    assert_eq!(
        kubernetes::parse_display_line(CRI_KLOG).tokens,
        kubernetes::parse_line(CRI_KLOG).unwrap().tokens
    );
}

#[test]
fn klog_and_syslog_lines_are_shown_as_they_are_read() {
    for line in [PLAIN, SYSLOG] {
        assert_eq!(
            kubernetes::parse_display_line(line).tokens,
            kubernetes::parse_line(line).unwrap().tokens
        );
    }
}

#[test]
fn the_plugin_shows_every_line() {
    assert!(matches!(
        KubernetesPlugin::new().parse_display_line("the maintenance window moves to 02:00"),
        ParseResult::Parsed(_)
    ));
}
