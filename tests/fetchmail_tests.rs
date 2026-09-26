use splash::fetchmail::{self, FetchmailPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const POLL: &str = concat!(
    "Oct  3 12:00:02 laptop fetchmail[2200]: 3 messages for alice at mail.example.com ",
    "(12345 octets)."
);

const LOG_FILE_POLL: &str =
    "fetchmail: 3 messages (1 seen) for alice at mail.example.com (12345 octets).";

const READING: &str = "reading message alice@mail.example.com:1 of 3 (4096 octets) flushed";

const NOT_FLUSHED: &str =
    "reading message alice@mail.example.com:2 of 3 (4096 header octets) not flushed";

const QUERY_STATUS: &str = "Oct  3 12:00:03 laptop fetchmail[2200]: Query status=3 (AUTHFAIL)";

const SLEEPING: &str =
    "Oct  3 12:00:04 laptop fetchmail[2200]: sleeping at Tue Oct  3 12:00:04 2023 for 900 seconds";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    fetchmail::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    fetchmail::parse_line(line)
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
fn a_poll_line_keeps_every_character_of_the_original() {
    assert_eq!(fetchmail::parse_line(POLL).unwrap().text(), POLL);
}

#[test]
fn a_poll_line_colors_the_program_that_logged_it() {
    assert_eq!(field(POLL, TokenKind::Tag), "fetchmail");
}

#[test]
fn a_poll_line_colors_the_message_count() {
    assert_eq!(field(POLL, TokenKind::Number), "3");
}

#[test]
fn a_poll_line_colors_the_user_and_the_server() {
    assert_eq!(field(POLL, TokenKind::UserId), "alice");
    assert_eq!(
        all(POLL, TokenKind::Host),
        vec!["laptop", "mail.example.com"]
    );
}

#[test]
fn a_poll_line_colors_the_size_in_octets() {
    assert_eq!(field(POLL, TokenKind::Size), "12345");
}

#[test]
fn a_poll_line_from_the_log_file_keeps_every_character_of_the_original() {
    assert_eq!(
        fetchmail::parse_line(LOG_FILE_POLL).unwrap().text(),
        LOG_FILE_POLL
    );
}

#[test]
fn a_poll_line_from_the_log_file_colors_its_prefix() {
    assert_eq!(field(LOG_FILE_POLL, TokenKind::Tag), "fetchmail");
}

#[test]
fn a_poll_line_from_the_log_file_colors_the_seen_count() {
    assert_eq!(all(LOG_FILE_POLL, TokenKind::Number), vec!["3", "1"]);
}

#[test]
fn a_progress_line_keeps_every_character_of_the_original() {
    assert_eq!(fetchmail::parse_line(READING).unwrap().text(), READING);
}

#[test]
fn a_progress_line_has_no_prefix() {
    assert!(!kinds(READING).contains(&TokenKind::Tag));
}

#[test]
fn a_progress_line_colors_the_mailbox_as_an_email() {
    assert_eq!(field(READING, TokenKind::Email), "alice@mail.example.com");
}

#[test]
fn a_progress_line_colors_the_message_number_and_the_total() {
    assert_eq!(all(READING, TokenKind::Number), vec!["1", "3"]);
}

#[test]
fn a_progress_line_colors_a_flushed_message_as_a_success() {
    assert_eq!(field(READING, TokenKind::Success), "flushed");
}

#[test]
fn a_progress_line_colors_a_message_left_on_the_server_as_a_warning() {
    assert_eq!(field(NOT_FLUSHED, TokenKind::Warning), "not flushed");
}

#[test]
fn a_progress_line_colors_the_size_of_a_header_only_read() {
    assert_eq!(field(NOT_FLUSHED, TokenKind::Size), "4096");
}

#[test]
fn a_skipped_message_line_is_read() {
    let line = "skipping message alice@mail.example.com:3 (4153 octets) not flushed";

    assert_eq!(fetchmail::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_query_status_colors_the_status_field() {
    assert_eq!(field(QUERY_STATUS, TokenKind::Header), "status");
}

#[test]
fn a_query_status_colors_an_authentication_failure() {
    assert_eq!(field(QUERY_STATUS, TokenKind::Failure), "AUTHFAIL");
}

#[test]
fn a_date_inside_a_message_is_colored_as_a_timestamp() {
    assert_eq!(
        all(SLEEPING, TokenKind::Timestamp),
        vec!["Oct  3 12:00:04", "Tue Oct  3 12:00:04 2023"]
    );
}

#[test]
fn a_date_inside_a_message_keeps_every_character_of_the_original() {
    assert_eq!(fetchmail::parse_line(SLEEPING).unwrap().text(), SLEEPING);
}

#[test]
fn a_server_given_as_an_address_is_colored_as_an_address() {
    let line = "fetchmail: No mail for bob at 10.0.0.25";

    assert_eq!(field(line, TokenKind::Host), "10.0.0.25");
}

#[test]
fn an_address_inside_a_message_is_colored() {
    let line = "fetchmail: connection to 10.0.0.25 failed";

    assert_eq!(field(line, TokenKind::Ip), "10.0.0.25");
}

#[test]
fn a_version_number_is_colored_as_one_number() {
    let line = "fetchmail: starting fetchmail 6.4.37 daemon";

    assert_eq!(field(line, TokenKind::Number), "6.4.37");
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert!(fetchmail::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_a_fetchmail_log_is_not_parsed() {
    assert!(fetchmail::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(FetchmailPlugin::new().name(), "fetchmail");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(FetchmailPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        FetchmailPlugin::default().metadata().description,
        "Fetchmail email retrieval logs"
    );
}

#[test]
fn the_plugin_parses_a_poll_line_into_tokens() {
    let parsed = match FetchmailPlugin::new().parse_line(POLL) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the poll line should parse"),
    };

    assert_eq!(parsed.text(), POLL);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        FetchmailPlugin::new().parse_line("not a fetchmail log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        FetchmailPlugin::new().detect_format(&[POLL, LOG_FILE_POLL, READING, QUERY_STATUS]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(
        FetchmailPlugin::new().detect_format(&["hello", "world"]),
        0.0
    );
}
