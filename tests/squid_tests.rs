use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::squid::{self, SquidPlugin};

const NATIVE_MISS: &str = concat!(
    "1614729600.123    123 10.0.0.5 TCP_MISS/200 4512 GET http://example.com/ - ",
    "HIER_DIRECT/93.184.216.34 text/html"
);

const NATIVE_HIT: &str = concat!(
    "1614729601.004     12 10.0.0.42 TCP_MEM_HIT/200 8810 GET http://example.com/app.css ",
    "alice NONE/- text/css"
);

const NATIVE_DENIED: &str = concat!(
    "1614729602.900      0 192.168.1.50 TCP_DENIED/403 3621 CONNECT example.com:443 - ",
    "HIER_NONE/- text/html"
);

const EMULATED: &str = concat!(
    r#"10.0.0.5 - - [09/Mar/2021:12:00:00 +0000] "GET http://example.com/ HTTP/1.1" 200 4512 "#,
    "TCP_MISS:HIER_DIRECT"
);

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    squid::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    squid::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_native_access_log_line_keeps_every_character_of_the_original() {
    assert_eq!(squid::parse_line(NATIVE_MISS).unwrap().text(), NATIVE_MISS);
}

#[test]
fn a_native_access_log_line_keeps_the_padding_between_its_fields() {
    assert_eq!(
        squid::parse_line(NATIVE_HIT).unwrap().tokens[1].text,
        "     "
    );
}

#[test]
fn a_native_access_log_line_colors_the_timestamp_that_starts_it() {
    assert_eq!(field(NATIVE_MISS, TokenKind::Timestamp), "1614729600.123");
}

#[test]
fn a_native_access_log_line_colors_the_elapsed_milliseconds_as_a_duration() {
    assert_eq!(field(NATIVE_MISS, TokenKind::Duration), "123");
}

#[test]
fn a_native_access_log_line_colors_the_client_that_made_the_request() {
    assert_eq!(field(NATIVE_MISS, TokenKind::Client), "10.0.0.5");
}

#[test]
fn a_native_access_log_line_colors_the_requested_url() {
    assert_eq!(
        field(NATIVE_MISS, TokenKind::Request),
        "http://example.com/"
    );
}

#[test]
fn a_native_access_log_line_colors_the_hierarchy_code() {
    assert_eq!(field(NATIVE_MISS, TokenKind::Hierarchy), "HIER_DIRECT");
}

#[test]
fn a_native_access_log_line_colors_the_content_type_that_ends_it() {
    assert_eq!(field(NATIVE_MISS, TokenKind::ContentType), "text/html");
}

#[test]
fn a_request_squid_had_to_fetch_is_colored_as_a_cache_miss() {
    assert_eq!(field(NATIVE_MISS, TokenKind::CacheMiss), "TCP_MISS");
}

#[test]
fn a_request_squid_served_from_memory_is_colored_as_a_cache_hit() {
    assert_eq!(field(NATIVE_HIT, TokenKind::CacheHit), "TCP_MEM_HIT");
}

#[test]
fn a_result_code_that_is_neither_a_hit_nor_a_miss_gets_the_neutral_style() {
    assert_eq!(field(NATIVE_DENIED, TokenKind::CacheResult), "TCP_DENIED");
}

#[test]
fn a_result_code_of_hit_alone_is_colored_as_a_cache_hit() {
    let line = NATIVE_HIT.replace("TCP_MEM_HIT", "HIT");

    assert_eq!(field(&line, TokenKind::CacheHit), "HIT");
}

#[test]
fn a_result_code_of_miss_alone_is_colored_as_a_cache_miss() {
    let line = NATIVE_MISS.replace("TCP_MISS", "MISS");

    assert_eq!(field(&line, TokenKind::CacheMiss), "MISS");
}

#[test]
fn a_native_line_with_no_leading_padding_is_read() {
    assert_eq!(kinds(NATIVE_MISS).first(), Some(&TokenKind::Timestamp));
}

#[test]
fn a_native_line_with_leading_padding_keeps_it() {
    let line = format!("  {}", NATIVE_MISS);

    assert_eq!(squid::parse_line(&line).unwrap().text(), line);
}

#[test]
fn a_native_line_with_a_field_squid_added_later_keeps_it() {
    let line = format!("{} GET", NATIVE_MISS);

    assert_eq!(squid::parse_line(&line).unwrap().text(), line);
}

#[test]
fn a_native_line_whose_response_had_no_body_is_read() {
    let line = NATIVE_MISS.replace(" 4512 ", " - ");

    assert_eq!(field(&line, TokenKind::Size), "-");
}

#[test]
fn an_httpd_emulated_line_keeps_every_character_of_the_original() {
    assert_eq!(squid::parse_line(EMULATED).unwrap().text(), EMULATED);
}

#[test]
fn an_httpd_emulated_line_colors_the_result_code_appended_to_the_request() {
    assert_eq!(field(EMULATED, TokenKind::CacheMiss), "TCP_MISS");
}

#[test]
fn an_httpd_emulated_line_colors_the_hierarchy_code_appended_to_the_request() {
    assert_eq!(field(EMULATED, TokenKind::Hierarchy), "HIER_DIRECT");
}

#[test]
fn an_httpd_emulated_line_colors_the_protocol_of_the_request() {
    assert_eq!(field(EMULATED, TokenKind::Protocol), "HTTP/1.1");
}

#[test]
fn a_common_log_format_line_with_no_cache_result_is_not_a_squid_line() {
    let line = r#"10.0.0.5 - - [09/Mar/2021:12:00:00 +0000] "GET / HTTP/1.1" 200 4512"#;

    assert!(squid::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_an_access_log_is_not_parsed() {
    assert!(squid::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(SquidPlugin::new().name(), "squid");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(SquidPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        SquidPlugin::default().metadata().description,
        "Squid access logs, native and httpd-emulated"
    );
}

#[test]
fn the_plugin_parses_a_native_line_into_tokens() {
    let parsed = match SquidPlugin::new().parse_line(NATIVE_MISS) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the native line should parse"),
    };

    assert_eq!(parsed.text(), NATIVE_MISS);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        SquidPlugin::new().parse_line("not an access log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        SquidPlugin::new().detect_format(&[NATIVE_MISS, NATIVE_HIT, EMULATED]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(SquidPlugin::new().detect_format(&["hello", "world"]), 0.0);
}
