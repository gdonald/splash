use splash::apache_error::{self, ApacheErrorPlugin};
use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};

const CORE: &str = concat!(
    "[Wed Oct 11 14:32:52.123456 2023] [core:error] [pid 35708:tid 4328636416] ",
    "[client 72.15.99.187:1234] AH00128: File does not exist: /var/www/favicon.ico"
);

const PHP: &str = concat!(
    "[Wed Oct 11 14:33:05.000100 2023] [php7:error] [pid 35710] [client 2001:db8::1:52144] ",
    "PHP Warning:  Undefined variable $total in /var/www/html/cart.php on line 42"
);

const OLD: &str = "[Wed Oct 11 14:33:01 2023] [warn] [client 192.168.1.50] mod_deflate: skipping";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    apache_error::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    apache_error::parse_line(line)
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
    for line in [CORE, PHP, OLD] {
        assert_eq!(apache_error::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_line_colors_its_date_and_module() {
    assert_eq!(
        field(CORE, TokenKind::Timestamp),
        "Wed Oct 11 14:32:52.123456 2023"
    );
    assert_eq!(field(CORE, TokenKind::Module), "core");
}

#[test]
fn an_error_is_colored_as_a_failure() {
    assert_eq!(field(CORE, TokenKind::Failure), "error");
}

#[test]
fn a_warning_is_colored_as_a_warning() {
    assert_eq!(field(OLD, TokenKind::Warning), "warn");
}

#[test]
fn a_notice_is_colored_as_a_level() {
    assert_eq!(
        field("[Wed Oct 11 14:33:20 2023] [notice]", TokenKind::Level),
        "notice"
    );
}

#[test]
fn the_process_and_thread_are_colored_apart() {
    assert_eq!(all(CORE, TokenKind::Pid), vec!["35708", "4328636416"]);
}

#[test]
fn a_process_without_a_thread_is_read() {
    assert_eq!(all(PHP, TokenKind::Pid), vec!["35710"]);
}

#[test]
fn the_client_address_and_port_are_colored_apart() {
    assert_eq!(field(CORE, TokenKind::Ip), "72.15.99.187");
    assert_eq!(field(CORE, TokenKind::Number), "1234");
}

#[test]
fn an_ipv6_client_keeps_its_own_colons() {
    assert_eq!(field(PHP, TokenKind::Ip), "2001:db8::1");
    assert_eq!(field(PHP, TokenKind::Number), "52144");
}

#[test]
fn a_client_without_a_port_is_one_address() {
    assert_eq!(field(OLD, TokenKind::Ip), "192.168.1.50");
    assert!(!kinds(OLD).contains(&TokenKind::Number));
}

#[test]
fn a_client_given_as_a_name_is_colored_as_a_host() {
    let line = "[Wed Oct 11 14:33:01 2023] [error] [client proxy.example.org:x] denied";

    assert_eq!(field(line, TokenKind::Host), "proxy.example.org:x");
}

#[test]
fn an_empty_client_is_only_its_label() {
    let line = "[Wed Oct 11 14:33:01 2023] [error] [client ] denied";

    assert_eq!(apache_error::parse_line(line).unwrap().text(), line);
    assert!(!kinds(line).contains(&TokenKind::Ip));
}

#[test]
fn an_error_code_is_colored_as_a_transaction() {
    assert_eq!(field(CORE, TokenKind::Transaction), "AH00128");
}

#[test]
fn a_php_message_is_colored() {
    assert_eq!(field(PHP, TokenKind::Warning), "Warning");
    assert_eq!(field(PHP, TokenKind::Path), "/var/www/html/cart.php");
}

#[test]
fn a_message_without_a_leading_space_is_read() {
    let line = "[Wed Oct 11 14:33:20 2023] [notice]:x";

    assert_eq!(
        apache_error::parse_line(line).unwrap().tokens.last(),
        Some(&Token::new(":x", TokenKind::Message))
    );
}

#[test]
fn an_access_log_line_is_not_parsed() {
    let line = r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET / HTTP/1.0" 200 2326"#;

    assert!(apache_error::parse_line(line).is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(ApacheErrorPlugin::new().name(), "apache-error");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(ApacheErrorPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        ApacheErrorPlugin::default().metadata().description,
        "Apache error logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match ApacheErrorPlugin::new().parse_line(CORE) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), CORE);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        ApacheErrorPlugin::new().parse_line("not an error log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        ApacheErrorPlugin::new().detect_format(&[CORE, PHP, OLD]),
        1.0
    );
}
