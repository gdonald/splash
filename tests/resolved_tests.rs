use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::resolved::{self, ResolvedPlugin};

const DEGRADED: &str = "Oct 03 12:00:01 web01 systemd-resolved[600]: Using degraded feature set UDP instead of UDP+EDNS0 for DNS server 10.0.0.53.";

const ANCHOR: &str =
    "Oct 03 12:00:00 web01 systemd-resolved[600]: . IN DS 20326 8 2 e06d44b80b8f1d39";

const SWITCH: &str =
    "Oct 03 12:00:02 web01 systemd-resolved[600]: Switching to fallback DNS server 2001:db8::53.";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    resolved::parse_line(line)
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
    for line in [DEGRADED, ANCHOR, SWITCH] {
        assert_eq!(resolved::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_degraded_feature_set_is_colored_as_a_warning() {
    assert_eq!(
        field(DEGRADED, TokenKind::Warning),
        "Using degraded feature set"
    );
}

#[test]
fn feature_levels_and_the_server_are_colored() {
    assert_eq!(all(DEGRADED, TokenKind::Protocol), vec!["UDP", "UDP+EDNS0"]);
    assert_eq!(field(DEGRADED, TokenKind::Ip), "10.0.0.53");
}

#[test]
fn a_trust_anchor_colors_its_name_and_type() {
    assert_eq!(all(ANCHOR, TokenKind::Host), vec!["web01", "."]);
    assert_eq!(field(ANCHOR, TokenKind::Protocol), "DS");
}

#[test]
fn an_ipv6_server_is_colored_as_an_address() {
    assert_eq!(field(SWITCH, TokenKind::Ip), "2001:db8::53");
}

#[test]
fn a_time_shaped_like_an_ipv6_address_is_message_text() {
    let line = "Oct 03 12:00:02 web01 systemd-resolved[600]: Clock at 12:00:01 changed";

    assert!(all(line, TokenKind::Ip).is_empty());
}

#[test]
fn a_quoted_name_is_colored_as_a_host() {
    let line = "Oct 03 12:00:00 web01 systemd-resolved[600]: Using system hostname 'web01'.";

    assert_eq!(all(line, TokenKind::Host), vec!["web01", "web01"]);
}

#[test]
fn an_empty_quoted_name_is_only_its_quotes() {
    let line = "Oct 03 12:00:00 web01 systemd-resolved[600]: Using system hostname ''.";

    assert_eq!(all(line, TokenKind::Host), vec!["web01"]);
}

#[test]
fn a_failed_lookup_is_colored_as_a_failure() {
    let line = "Oct 03 12:06:00 web01 systemd-resolved[600]: Server returned error NXDOMAIN";

    assert_eq!(field(line, TokenKind::Failure), "NXDOMAIN");
}

#[test]
fn a_line_from_another_program_is_not_parsed() {
    let line = "Oct 03 12:00:02 web01 systemd[1]: Started nginx.service.";

    assert!(resolved::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(resolved::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        ResolvedPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(ResolvedPlugin::new().name(), "systemd-resolved");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(ResolvedPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        ResolvedPlugin::default().metadata().description,
        "systemd-resolved DNS resolution logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match ResolvedPlugin::new().parse_line(DEGRADED) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), DEGRADED);
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        ResolvedPlugin::new().detect_format(&[DEGRADED, ANCHOR, SWITCH]),
        1.0
    );
}
