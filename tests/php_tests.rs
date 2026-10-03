use splash::output::{Token, TokenKind};
use splash::php::{self, PhpPlugin};
use splash::plugin::{ParseResult, Plugin};

const WARNING: &str = "[03-Oct-2023 12:00:01 UTC] PHP Warning:  Undefined variable $total in /var/www/html/cart.php on line 42";

const UNCAUGHT: &str = "[03-Oct-2023 12:00:05 UTC] PHP Fatal error:  Uncaught Exception: Payment gateway timed out in /var/www/html/pay.php:88";

const FRAME: &str = "#0 /var/www/html/checkout.php(20): charge()";

const THROWN: &str = "  thrown in /var/www/html/pay.php on line 88";

const FPM_CHILD: &str =
    r#"[03-Oct-2023 12:00:06] WARNING: [pool www] child 1234 said into stderr: "PHP Notice:  x""#;

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    php::parse_line(line)
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
    for line in [WARNING, UNCAUGHT, FRAME, THROWN, FPM_CHILD] {
        assert_eq!(php::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_error_log_line_colors_its_date_with_the_time_zone() {
    assert_eq!(
        field(WARNING, TokenKind::Timestamp),
        "03-Oct-2023 12:00:01 UTC"
    );
}

#[test]
fn a_warning_is_colored_as_a_warning() {
    assert_eq!(field(WARNING, TokenKind::Tag), "PHP");
    assert_eq!(field(WARNING, TokenKind::Warning), "Warning");
}

#[test]
fn a_fatal_error_and_an_uncaught_exception_are_failures() {
    assert_eq!(
        all(UNCAUGHT, TokenKind::Failure),
        vec!["Fatal error", "Uncaught"]
    );
}

#[test]
fn notices_and_deprecations_are_levels() {
    assert_eq!(php::error_kind("Notice"), TokenKind::Level);
    assert_eq!(php::error_kind("Deprecated"), TokenKind::Level);
}

#[test]
fn every_other_error_type_is_a_failure() {
    for error in [
        "Fatal error",
        "Recoverable fatal error",
        "Parse error",
        "Unknown error",
    ] {
        assert_eq!(php::error_kind(error), TokenKind::Failure);
    }
}

#[test]
fn the_file_and_line_an_error_happened_on_are_colored() {
    assert_eq!(field(WARNING, TokenKind::Path), "/var/www/html/cart.php");
    assert_eq!(field(WARNING, TokenKind::Number), "42");
}

#[test]
fn a_file_and_line_joined_by_a_colon_are_colored() {
    assert_eq!(field(UNCAUGHT, TokenKind::Path), "/var/www/html/pay.php");
    assert_eq!(field(UNCAUGHT, TokenKind::Number), "88");
}

#[test]
fn a_stack_frame_colors_its_number_file_and_line() {
    assert_eq!(all(FRAME, TokenKind::Number), vec!["0", "20"]);
    assert_eq!(field(FRAME, TokenKind::Path), "/var/www/html/checkout.php");
}

#[test]
fn the_line_closing_a_stack_trace_colors_the_file() {
    assert_eq!(field(THROWN, TokenKind::Failure), "thrown");
    assert_eq!(field(THROWN, TokenKind::Path), "/var/www/html/pay.php");
}

#[test]
fn the_line_opening_a_stack_trace_is_read() {
    assert_eq!(
        php::parse_line("Stack trace:").unwrap().tokens,
        vec![Token::new("Stack trace:", TokenKind::Message)]
    );
}

#[test]
fn a_php_fpm_line_colors_its_level_pool_and_child() {
    assert_eq!(field(FPM_CHILD, TokenKind::Warning), "WARNING");
    assert_eq!(field(FPM_CHILD, TokenKind::Module), "www");
    assert_eq!(field(FPM_CHILD, TokenKind::Pid), "1234");
}

#[test]
fn a_php_message_inside_a_php_fpm_line_is_colored() {
    assert_eq!(field(FPM_CHILD, TokenKind::Level), "Notice");
}

#[test]
fn a_php_fpm_error_or_alert_is_a_failure() {
    let line = "[03-Oct-2023 12:00:06] ALERT: oops";

    assert_eq!(field(line, TokenKind::Failure), "ALERT");
    assert_eq!(
        field("[03-Oct-2023 12:00:06] ERROR: oops", TokenKind::Failure),
        "ERROR"
    );
}

#[test]
fn a_php_fpm_notice_is_a_level() {
    let line = "[03-Oct-2023 12:00:00] NOTICE: fpm is running, pid 1000";

    assert_eq!(field(line, TokenKind::Level), "NOTICE");
    assert_eq!(field(line, TokenKind::Success), "fpm is running");
}

#[test]
fn a_php_fpm_debug_line_keeps_its_microseconds() {
    let line = "[03-Oct-2023 12:00:09.123456] DEBUG: pid 1000, line 378";

    assert_eq!(
        field(line, TokenKind::Timestamp),
        "03-Oct-2023 12:00:09.123456"
    );
}

#[test]
fn a_line_that_is_not_a_php_log_is_not_parsed() {
    assert!(php::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(PhpPlugin::new().name(), "php");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(PhpPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        PhpPlugin::default().metadata().description,
        "PHP error logs and PHP-FPM logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match PhpPlugin::new().parse_line(WARNING) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), WARNING);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        PhpPlugin::new().parse_line("not a php log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        PhpPlugin::new().detect_format(&[WARNING, UNCAUGHT, FRAME, THROWN]),
        1.0
    );
}
