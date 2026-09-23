use splash::haproxy::{self, HaproxyPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const HTTP_BODY: &str = concat!(
    "10.0.1.2:33317 [06/Feb/2009:12:14:14.655] http-in static/srv1 10/0/30/69/109 ",
    r#"200 2750 - - ---- 1/1/1/1/0 0/0 "GET /index.html HTTP/1.1""#
);

const HTTP_LOG: &str = concat!(
    "Feb  6 12:14:14 gateway haproxy[14389]: 10.0.1.2:33317 [06/Feb/2009:12:14:14.655] ",
    r#"http-in static/srv1 10/0/30/69/109 200 2750 - - ---- 1/1/1/1/0 0/0 "GET /index.html HTTP/1.1""#
);

const TCP_LOG: &str = concat!(
    "Feb  6 12:12:56 gateway haproxy[14387]: 10.0.1.2:33313 [06/Feb/2009:12:12:51.443] ",
    "fnt bck/srv1 0/0/5007 212 -- 0/0/0/0/3 0/0"
);

const ERROR_LOG: &str = concat!(
    "Feb  6 12:12:56 gateway haproxy[14387]: 127.0.0.1:34550 [06/Feb/2009:12:12:51.443] ",
    "frt/f1: invalid request"
);

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    haproxy::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    haproxy::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn an_http_connection_line_keeps_every_character_of_the_original() {
    assert_eq!(haproxy::parse_line(HTTP_LOG).unwrap().text(), HTTP_LOG);
}

#[test]
fn an_http_connection_line_colors_the_host_that_logged_it() {
    assert_eq!(field(HTTP_LOG, TokenKind::Host), "gateway");
}

#[test]
fn an_http_connection_line_colors_the_process_and_its_identifier() {
    assert_eq!(field(HTTP_LOG, TokenKind::Tag), "haproxy");
    assert_eq!(field(HTTP_LOG, TokenKind::Pid), "14389");
}

#[test]
fn an_http_connection_line_colors_the_client_with_its_port() {
    assert_eq!(field(HTTP_LOG, TokenKind::Client), "10.0.1.2:33317");
}

#[test]
fn an_http_connection_line_colors_the_frontend_it_arrived_on() {
    assert_eq!(field(HTTP_LOG, TokenKind::Frontend), "http-in");
}

#[test]
fn an_http_connection_line_separates_the_backend_from_the_server() {
    assert_eq!(field(HTTP_LOG, TokenKind::Backend), "static");
    assert_eq!(field(HTTP_LOG, TokenKind::Server), "srv1");
}

#[test]
fn an_http_connection_line_colors_the_timers_as_one_field() {
    assert_eq!(field(HTTP_LOG, TokenKind::Timers), "10/0/30/69/109");
}

#[test]
fn an_http_connection_line_colors_the_status_and_the_bytes_read() {
    assert_eq!(field(HTTP_LOG, TokenKind::Status), "200");
    assert_eq!(field(HTTP_LOG, TokenKind::Size), "2750");
}

#[test]
fn an_http_connection_line_colors_the_termination_state() {
    assert_eq!(field(HTTP_LOG, TokenKind::Termination), "----");
}

#[test]
fn an_http_connection_line_colors_the_connection_counters() {
    assert_eq!(field(HTTP_LOG, TokenKind::Counters), "1/1/1/1/0");
}

#[test]
fn an_http_connection_line_colors_the_request_it_ends_with() {
    assert_eq!(field(HTTP_LOG, TokenKind::Method), "GET");
    assert_eq!(field(HTTP_LOG, TokenKind::Request), "/index.html");
    assert_eq!(field(HTTP_LOG, TokenKind::Protocol), "HTTP/1.1");
}

#[test]
fn an_http_connection_line_logged_without_a_syslog_header_is_read() {
    assert_eq!(haproxy::parse_line(HTTP_BODY).unwrap().text(), HTTP_BODY);
}

#[test]
fn an_http_connection_line_logged_without_a_syslog_header_has_no_host() {
    assert!(!kinds(HTTP_BODY).contains(&TokenKind::Host));
}

#[test]
fn an_http_connection_line_with_captured_headers_keeps_every_character() {
    let line = HTTP_BODY.replace(r#"0/0 "GET"#, r#"0/0 {example.com|curl/8.4.0} "GET"#);

    assert_eq!(haproxy::parse_line(&line).unwrap().text(), line);
}

#[test]
fn an_http_connection_line_with_captured_headers_still_colors_the_request() {
    let line = HTTP_BODY.replace(r#"0/0 "GET"#, r#"0/0 {example.com|curl/8.4.0} "GET"#);

    assert_eq!(field(&line, TokenKind::Method), "GET");
}

#[test]
fn an_http_connection_line_with_no_status_is_read() {
    let line = HTTP_BODY.replace(" 200 2750 ", " -1 0 ");

    assert_eq!(field(&line, TokenKind::Status), "-1");
}

#[test]
fn an_http_connection_line_whose_request_was_never_read_is_left_as_message_text() {
    let line = HTTP_BODY.replace(r#" "GET /index.html HTTP/1.1""#, " <BADREQ>");

    assert_eq!(field(&line, TokenKind::Message), "<BADREQ>");
}

#[test]
fn a_tcp_connection_line_keeps_every_character_of_the_original() {
    assert_eq!(haproxy::parse_line(TCP_LOG).unwrap().text(), TCP_LOG);
}

#[test]
fn a_tcp_connection_line_colors_the_three_timers_it_reports() {
    assert_eq!(field(TCP_LOG, TokenKind::Timers), "0/0/5007");
}

#[test]
fn a_tcp_connection_line_colors_its_two_character_termination_state() {
    assert_eq!(field(TCP_LOG, TokenKind::Termination), "--");
}

#[test]
fn a_tcp_connection_line_colors_the_bytes_read() {
    assert_eq!(field(TCP_LOG, TokenKind::Size), "212");
}

#[test]
fn a_tcp_connection_line_has_no_status() {
    assert!(!kinds(TCP_LOG).contains(&TokenKind::Status));
}

#[test]
fn an_error_line_keeps_every_character_of_the_original() {
    assert_eq!(haproxy::parse_line(ERROR_LOG).unwrap().text(), ERROR_LOG);
}

#[test]
fn an_error_line_colors_the_frontend_and_the_listener_that_refused_it() {
    assert_eq!(field(ERROR_LOG, TokenKind::Frontend), "frt");
    assert_eq!(field(ERROR_LOG, TokenKind::Server), "f1");
}

#[test]
fn an_error_line_colors_what_went_wrong_as_message_text() {
    assert_eq!(field(ERROR_LOG, TokenKind::Message), " invalid request");
}

#[test]
fn an_error_line_colors_an_address_inside_its_message() {
    let line = ERROR_LOG.replace("invalid request", "no server available for 10.0.0.9");

    assert_eq!(field(&line, TokenKind::Ip), "10.0.0.9");
}

#[test]
fn a_syslog_header_with_no_haproxy_fields_after_it_is_not_parsed() {
    assert!(haproxy::parse_line("Feb  6 12:12:56 gateway haproxy[14387]: reloading").is_none());
}

#[test]
fn a_line_that_is_not_a_haproxy_log_is_not_parsed() {
    assert!(haproxy::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(HaproxyPlugin::new().name(), "haproxy");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(HaproxyPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        HaproxyPlugin::default().metadata().description,
        "HAProxy connection and error logs"
    );
}

#[test]
fn the_plugin_parses_a_connection_line_into_tokens() {
    let parsed = match HaproxyPlugin::new().parse_line(HTTP_LOG) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the connection line should parse"),
    };

    assert_eq!(parsed.text(), HTTP_LOG);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        HaproxyPlugin::new().parse_line("not a haproxy log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        HaproxyPlugin::new().detect_format(&[HTTP_LOG, TCP_LOG, ERROR_LOG]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(HaproxyPlugin::new().detect_format(&["hello", "world"]), 0.0);
}
