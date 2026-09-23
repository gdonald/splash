use splash::caddy::{self, value_kind, CaddyPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const ACCESS: &str = concat!(
    r#"{"level":"info","ts":1646861401.5241024,"logger":"http.log.access","#,
    r#""msg":"handled request","request":{"remote_ip":"127.0.0.1","remote_port":"41342","#,
    r#""client_ip":"127.0.0.1","proto":"HTTP/2.0","method":"GET","host":"localhost","#,
    r#""uri":"/orders","headers":{"User-Agent":["curl/7.82.0"]},"#,
    r#""tls":{"resumed":false,"version":772}},"bytes_read":0,"user_id":"","#,
    r#""duration":0.000929675,"size":10900,"status":200,"#,
    r#""resp_headers":{"Server":["Caddy"]}}"#
);

const ERROR: &str = concat!(
    r#"{"level":"error","ts":1646861402.1,"logger":"tls","msg":"job failed","#,
    r#""error":"no such host"}"#
);

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    caddy::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    caddy::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

/// The text of every token of the given kind
fn fields(line: &str, kind: TokenKind) -> Vec<String> {
    caddy::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .filter(|token| token.kind == kind)
        .map(|token| token.text.to_string())
        .collect()
}

#[test]
fn an_access_log_line_keeps_every_character_of_the_original() {
    assert_eq!(caddy::parse_line(ACCESS).unwrap().text(), ACCESS);
}

#[test]
fn an_access_log_line_colors_the_level() {
    assert_eq!(field(ACCESS, TokenKind::Level), "info");
}

#[test]
fn an_access_log_line_colors_the_unix_timestamp() {
    assert_eq!(field(ACCESS, TokenKind::Timestamp), "1646861401.5241024");
}

#[test]
fn an_access_log_line_colors_the_logger_that_wrote_it() {
    assert_eq!(field(ACCESS, TokenKind::Tag), "http.log.access");
}

#[test]
fn an_access_log_line_colors_the_message() {
    assert_eq!(field(ACCESS, TokenKind::Message), "handled request");
}

#[test]
fn an_access_log_line_colors_the_address_nested_inside_the_request() {
    assert_eq!(field(ACCESS, TokenKind::Ip), "127.0.0.1");
}

#[test]
fn an_access_log_line_colors_the_method_nested_inside_the_request() {
    assert_eq!(field(ACCESS, TokenKind::Method), "GET");
}

#[test]
fn an_access_log_line_colors_the_uri_nested_inside_the_request() {
    assert_eq!(field(ACCESS, TokenKind::Request), "/orders");
}

#[test]
fn an_access_log_line_colors_the_host_nested_inside_the_request() {
    assert_eq!(field(ACCESS, TokenKind::Host), "localhost");
}

#[test]
fn an_access_log_line_colors_the_protocol_nested_inside_the_request() {
    assert_eq!(field(ACCESS, TokenKind::Protocol), "HTTP/2.0");
}

#[test]
fn an_access_log_line_colors_the_status() {
    assert_eq!(field(ACCESS, TokenKind::Status), "200");
}

#[test]
fn an_access_log_line_colors_the_bytes_read_and_the_response_size() {
    assert_eq!(fields(ACCESS, TokenKind::Size), vec!["0", "10900"]);
}

#[test]
fn an_access_log_line_colors_how_long_the_request_took() {
    assert_eq!(field(ACCESS, TokenKind::Duration), "0.000929675");
}

#[test]
fn every_key_in_the_object_is_colored_as_a_key() {
    let keys = fields(ACCESS, TokenKind::Header);

    assert!(keys.contains(&"level".to_string()));
    assert!(keys.contains(&"remote_ip".to_string()));
    assert!(keys.contains(&"User-Agent".to_string()));
}

#[test]
fn a_value_under_a_key_splash_has_no_field_for_keeps_the_plain_style() {
    let line = r#"{"custom":"unrecognized"}"#;

    assert_eq!(fields(line, TokenKind::Plain), vec!["unrecognized"]);
}

#[test]
fn a_number_under_a_key_splash_has_no_field_for_is_colored_as_a_number() {
    let line = r#"{"version":772}"#;

    assert_eq!(field(line, TokenKind::Number), "772");
}

#[test]
fn the_three_bare_words_json_allows_keep_the_plain_style() {
    let line = r#"{"status":null,"size":true,"duration":false}"#;
    let values: Vec<TokenKind> = kinds(line)
        .into_iter()
        .filter(|kind| *kind == TokenKind::Plain)
        .collect();

    assert_eq!(values.len(), 3);
}

#[test]
fn an_empty_string_value_produces_no_token_of_its_own() {
    assert!(!fields(ACCESS, TokenKind::UserId).contains(&String::new()));
}

#[test]
fn an_error_line_colors_what_went_wrong_as_message_text() {
    assert_eq!(
        fields(ERROR, TokenKind::Message),
        vec!["job failed", "no such host"]
    );
}

#[test]
fn an_error_line_keeps_every_character_of_the_original() {
    assert_eq!(caddy::parse_line(ERROR).unwrap().text(), ERROR);
}

#[test]
fn an_object_written_with_spacing_between_its_pieces_keeps_that_spacing() {
    let line = r#"  { "level" : "info" , "size" : 12 }  "#;

    assert_eq!(caddy::parse_line(line).unwrap().text(), line);
}

#[test]
fn an_empty_object_is_read() {
    assert_eq!(caddy::parse_line("{}").unwrap().text(), "{}");
}

#[test]
fn an_empty_array_value_is_read() {
    let line = r#"{"list":[]}"#;

    assert_eq!(caddy::parse_line(line).unwrap().text(), line);
}

#[test]
fn every_element_of_an_array_takes_the_style_of_the_key_above_it() {
    let line = r#"{"remote_ip":["10.0.0.9","10.0.0.5"]}"#;

    assert_eq!(fields(line, TokenKind::Ip), vec!["10.0.0.9", "10.0.0.5"]);
}

#[test]
fn a_string_holding_an_escaped_quote_is_read_to_its_real_end() {
    let line = r#"{"msg":"said \"hello\" twice","status":200}"#;

    assert_eq!(field(line, TokenKind::Status), "200");
}

#[test]
fn a_string_holding_an_escaped_quote_keeps_every_character() {
    let line = r#"{"msg":"said \"hello\" twice"}"#;

    assert_eq!(caddy::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_line_that_is_not_json_is_not_parsed() {
    assert!(caddy::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn a_json_array_rather_than_an_object_is_not_parsed() {
    assert!(caddy::parse_line("[1, 2, 3]").is_none());
}

#[test]
fn an_object_that_was_cut_off_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"level":"info""#).is_none());
}

#[test]
fn an_object_with_something_after_it_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"level":"info"} trailing"#).is_none());
}

#[test]
fn an_object_whose_key_is_not_a_string_is_not_parsed() {
    assert!(caddy::parse_line(r#"{level:"info"}"#).is_none());
}

#[test]
fn an_object_missing_the_colon_after_a_key_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"level" "info"}"#).is_none());
}

#[test]
fn an_object_missing_a_comma_between_pairs_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"level":"info" "size":1}"#).is_none());
}

#[test]
fn an_object_with_an_empty_value_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"level":}"#).is_none());
}

#[test]
fn an_array_missing_a_comma_between_elements_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"list":[1 2]}"#).is_none());
}

#[test]
fn an_array_that_was_cut_off_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"list":[1,2"#).is_none());
}

#[test]
fn a_string_that_was_never_closed_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"level":"info}"#).is_none());
}

#[test]
fn a_key_splash_has_no_field_for_maps_to_no_style() {
    assert_eq!(value_kind("something_else"), None);
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(CaddyPlugin::new().name(), "caddy");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(CaddyPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        CaddyPlugin::default().metadata().description,
        "Caddy structured JSON logs"
    );
}

#[test]
fn the_plugin_parses_an_access_log_line_into_tokens() {
    let parsed = match CaddyPlugin::new().parse_line(ACCESS) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the access log line should parse"),
    };

    assert_eq!(parsed.text(), ACCESS);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        CaddyPlugin::new().parse_line("not a caddy log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(CaddyPlugin::new().detect_format(&[ACCESS, ERROR]), 1.0);
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(CaddyPlugin::new().detect_format(&["hello", "world"]), 0.0);
}

#[test]
fn an_object_that_ends_after_its_opening_brace_is_not_parsed() {
    assert!(caddy::parse_line("{").is_none());
}

#[test]
fn an_object_that_ends_after_a_comma_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"level":"info","#).is_none());
}

#[test]
fn an_array_that_ends_after_its_opening_bracket_is_not_parsed() {
    assert!(caddy::parse_line(r#"{"list":["#).is_none());
}
