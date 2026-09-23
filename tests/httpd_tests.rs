use splash::httpd::{self, HttpdPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const COMBINED: &str = concat!(
    r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326 "#,
    r#""http://www.example.com/start.html" "Mozilla/5.0 (X11; Linux x86_64)""#
);

const VHOST_COMBINED: &str = concat!(
    r#"example.com:80 10.0.0.5 - - [10/Oct/2000:13:55:36 -0700] "POST /login HTTP/1.1" 302 0 "#,
    r#""-" "curl/8.4.0""#
);

const CLF: &str =
    r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /a.gif HTTP/1.0" 200 2326"#;

const APACHE_ERROR: &str = concat!(
    "[Wed Oct 11 14:32:52.123456 2023] [core:error] [pid 35708:tid 4328636416] ",
    "[client 72.15.99.187:1234] AH00128: File does not exist: /var/www/favicon.ico"
);

const APACHE_ERROR_WITHOUT_MODULE: &str =
    "[Wed Oct 11 14:32:52 2023] [error] [client 1.2.3.4] File does not exist";

const NGINX_ERROR: &str = concat!(
    "2023/10/11 14:32:52 [error] 1234#0: *1 open() failed, client: 10.0.0.9, ",
    "server: localhost"
);

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    httpd::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    httpd::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_combined_access_log_line_keeps_every_character_of_the_original() {
    assert_eq!(httpd::parse_line(COMBINED).unwrap().text(), COMBINED);
}

#[test]
fn a_combined_access_log_line_colors_the_referer() {
    assert_eq!(
        field(COMBINED, TokenKind::Referer),
        "http://www.example.com/start.html"
    );
}

#[test]
fn a_combined_access_log_line_colors_the_user_agent() {
    assert_eq!(
        field(COMBINED, TokenKind::UserAgent),
        "Mozilla/5.0 (X11; Linux x86_64)"
    );
}

#[test]
fn a_combined_access_log_line_colors_the_client_address() {
    assert_eq!(field(COMBINED, TokenKind::Client), "127.0.0.1");
}

#[test]
fn a_combined_access_log_line_colors_the_status_and_size() {
    assert_eq!(field(COMBINED, TokenKind::Status), "200");
    assert_eq!(field(COMBINED, TokenKind::Size), "2326");
}

#[test]
fn a_combined_access_log_line_has_no_virtual_host() {
    assert!(!kinds(COMBINED).contains(&TokenKind::VirtualHost));
}

#[test]
fn a_vhost_combined_line_keeps_every_character_of_the_original() {
    assert_eq!(
        httpd::parse_line(VHOST_COMBINED).unwrap().text(),
        VHOST_COMBINED
    );
}

#[test]
fn a_vhost_combined_line_colors_the_server_name_that_starts_it() {
    assert_eq!(
        field(VHOST_COMBINED, TokenKind::VirtualHost),
        "example.com:80"
    );
}

#[test]
fn a_vhost_combined_line_still_colors_the_client_that_follows_the_server_name() {
    assert_eq!(field(VHOST_COMBINED, TokenKind::Client), "10.0.0.5");
}

#[test]
fn a_combined_line_with_an_ipv6_client_is_recognized() {
    let line = concat!(
        r#"2001:db8::1 - - [10/Oct/2000:13:55:36 -0700] "GET / HTTP/1.1" 200 12 "#,
        r#""-" "curl/8.4.0""#
    );

    assert_eq!(field(line, TokenKind::Client), "2001:db8::1");
}

#[test]
fn an_extended_access_log_line_colors_the_trailing_request_time() {
    let line = format!("{} 1532", COMBINED);

    assert_eq!(field(&line, TokenKind::Duration), "1532");
}

#[test]
fn an_extended_access_log_line_keeps_every_character_of_the_original() {
    let line = format!("{}  1532 \"gzip\"", COMBINED);

    assert_eq!(httpd::parse_line(&line).unwrap().text(), line);
}

#[test]
fn an_extended_field_that_is_not_a_number_is_left_unstyled() {
    let line = format!("{} \"gzip\"", COMBINED);
    let trailing = httpd::parse_line(&line).unwrap().tokens.pop().unwrap();

    assert_eq!(trailing.kind, TokenKind::Plain);
}

#[test]
fn a_common_log_format_line_is_read_when_it_has_no_referer_or_user_agent() {
    assert_eq!(httpd::parse_line(CLF).unwrap().text(), CLF);
}

#[test]
fn a_common_log_format_line_has_no_user_agent() {
    assert!(!kinds(CLF).contains(&TokenKind::UserAgent));
}

#[test]
fn an_apache_error_line_keeps_every_character_of_the_original() {
    assert_eq!(
        httpd::parse_line(APACHE_ERROR).unwrap().text(),
        APACHE_ERROR
    );
}

#[test]
fn an_apache_error_line_separates_the_module_from_the_level() {
    assert_eq!(field(APACHE_ERROR, TokenKind::Module), "core");
    assert_eq!(field(APACHE_ERROR, TokenKind::Level), "error");
}

#[test]
fn an_apache_error_line_colors_the_process_and_thread_identifiers() {
    assert_eq!(
        field(APACHE_ERROR, TokenKind::Pid),
        "[pid 35708:tid 4328636416]"
    );
}

#[test]
fn an_apache_error_line_colors_the_client_address_with_its_brackets() {
    assert_eq!(
        field(APACHE_ERROR, TokenKind::Client),
        "[client 72.15.99.187:1234]"
    );
}

#[test]
fn an_apache_error_line_colors_the_message_that_closes_it() {
    assert_eq!(
        field(APACHE_ERROR, TokenKind::Message),
        "AH00128: File does not exist: /var/www/favicon.ico"
    );
}

#[test]
fn an_apache_error_line_without_a_module_reads_the_field_as_the_level_alone() {
    assert_eq!(
        field(APACHE_ERROR_WITHOUT_MODULE, TokenKind::Level),
        "error"
    );
}

#[test]
fn an_apache_error_line_without_a_module_has_no_module_token() {
    assert!(!kinds(APACHE_ERROR_WITHOUT_MODULE).contains(&TokenKind::Module));
}

#[test]
fn an_apache_error_line_without_a_process_identifier_has_no_pid_token() {
    assert!(!kinds(APACHE_ERROR_WITHOUT_MODULE).contains(&TokenKind::Pid));
}

#[test]
fn an_apache_error_line_colors_an_address_inside_its_message() {
    let line = "[Wed Oct 11 14:32:52 2023] [error] denied for 203.0.113.7 today";

    assert_eq!(field(line, TokenKind::Ip), "203.0.113.7");
}

#[test]
fn an_apache_error_line_that_ends_at_its_level_parses_with_no_message() {
    let line = "[Wed Oct 11 14:32:52 2023] [notice]";

    assert!(!kinds(line).contains(&TokenKind::Message));
}

#[test]
fn an_apache_error_line_that_ends_at_its_level_keeps_every_character() {
    let line = "[Wed Oct 11 14:32:52 2023] [notice]";

    assert_eq!(httpd::parse_line(line).unwrap().text(), line);
}

#[test]
fn an_nginx_error_line_keeps_every_character_of_the_original() {
    assert_eq!(httpd::parse_line(NGINX_ERROR).unwrap().text(), NGINX_ERROR);
}

#[test]
fn an_nginx_error_line_colors_the_level_between_its_brackets() {
    assert_eq!(field(NGINX_ERROR, TokenKind::Level), "error");
}

#[test]
fn an_nginx_error_line_colors_the_worker_and_thread_identifiers() {
    assert_eq!(field(NGINX_ERROR, TokenKind::Pid), "1234#0");
}

#[test]
fn an_nginx_error_line_colors_the_client_address_inside_its_message() {
    assert_eq!(field(NGINX_ERROR, TokenKind::Ip), "10.0.0.9");
}

#[test]
fn an_nginx_error_line_colors_the_timestamp_that_starts_it() {
    assert_eq!(
        field(NGINX_ERROR, TokenKind::Timestamp),
        "2023/10/11 14:32:52"
    );
}

#[test]
fn an_nginx_error_line_with_nothing_after_the_worker_parses_with_no_message() {
    let line = "2023/10/11 14:32:52 [error] 1234#0:";

    assert!(!kinds(line).contains(&TokenKind::Message));
}

#[test]
fn a_message_that_is_only_an_address_produces_one_address_token() {
    let line = "2023/10/11 14:32:52 [error] 1234#0: 10.0.0.9";

    assert_eq!(kinds(line).last(), Some(&TokenKind::Ip));
}

#[test]
fn a_line_that_is_not_a_web_server_log_is_not_parsed() {
    assert!(httpd::parse_line("this is not a web server log").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(HttpdPlugin::new().name(), "httpd");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(HttpdPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        HttpdPlugin::default().metadata().description,
        "Apache and nginx access and error logs"
    );
}

#[test]
fn the_plugin_parses_a_combined_line_into_tokens() {
    let parsed = match HttpdPlugin::new().parse_line(COMBINED) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the combined line should parse"),
    };

    assert_eq!(parsed.text(), COMBINED);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        HttpdPlugin::new().parse_line("not a web server log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        HttpdPlugin::new().detect_format(&[COMBINED, CLF, NGINX_ERROR]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(HttpdPlugin::new().detect_format(&["hello", "world"]), 0.0);
}
