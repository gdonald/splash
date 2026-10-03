use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::vsftpd::{self, VsftpdPlugin};

const CONNECT: &str = r#"Tue Oct  3 12:00:01 2023 [pid 4321] CONNECT: Client "10.0.0.5""#;

const LOGIN: &str = r#"Tue Oct  3 12:00:02 2023 [pid 4320] [alice] OK LOGIN: Client "10.0.0.5""#;

const DOWNLOAD: &str = concat!(
    r#"Tue Oct  3 12:00:03 2023 [pid 4322] [alice] OK DOWNLOAD: Client "10.0.0.5", "#,
    r#""/home/alice/report.pdf", 4096 bytes, 512.00Kbyte/sec"#
);

const FAILED_UPLOAD: &str = concat!(
    r#"Tue Oct  3 12:00:04 2023 [pid 4322] [alice] FAIL UPLOAD: Client "10.0.0.5", "#,
    r#""/home/alice/locked.txt", 0.00Kbyte/sec"#
);

const ANONYMOUS: &str = concat!(
    r#"Tue Oct  3 12:00:05 2023 [pid 4323] [ftp] OK LOGIN: Client "10.0.0.7", "#,
    r#"anon password "guest@example.org""#
);

const COMMAND: &str = r#"Tue Oct  3 12:00:06 2023 [pid 4322] [alice] FTP command: Client "10.0.0.5", "RETR report.pdf""#;

const RESPONSE: &str = concat!(
    r#"Tue Oct  3 12:00:06 2023 [pid 4322] [alice] FTP response: Client "10.0.0.5", "#,
    r#""226 Transfer complete.""#
);

const SYSLOG: &str =
    r#"Oct  3 12:00:07 ftp vsftpd[4324]: [bob] FAIL LOGIN: Client "::ffff:10.0.0.9""#;

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    vsftpd::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    vsftpd::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_log_file_line_keeps_every_character_of_the_original() {
    for line in [
        CONNECT,
        LOGIN,
        DOWNLOAD,
        FAILED_UPLOAD,
        ANONYMOUS,
        COMMAND,
        RESPONSE,
    ] {
        assert_eq!(vsftpd::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_log_file_line_colors_the_date_and_the_process_id() {
    assert_eq!(
        field(CONNECT, TokenKind::Timestamp),
        "Tue Oct  3 12:00:01 2023"
    );
    assert_eq!(field(CONNECT, TokenKind::Pid), "4321");
}

#[test]
fn a_connection_has_no_user_and_no_outcome() {
    assert!(!kinds(CONNECT).contains(&TokenKind::UserId));
    assert!(!kinds(CONNECT).contains(&TokenKind::Success));
}

#[test]
fn an_event_colors_the_user() {
    assert_eq!(field(LOGIN, TokenKind::UserId), "alice");
}

#[test]
fn an_event_colors_what_happened() {
    assert_eq!(field(DOWNLOAD, TokenKind::Method), "DOWNLOAD");
}

#[test]
fn an_event_colors_the_client_address() {
    assert_eq!(field(LOGIN, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn an_event_that_succeeded_is_colored_as_a_success() {
    assert_eq!(field(LOGIN, TokenKind::Success), "OK");
}

#[test]
fn an_event_that_failed_is_colored_as_a_failure() {
    assert_eq!(field(FAILED_UPLOAD, TokenKind::Failure), "FAIL");
}

#[test]
fn a_transfer_colors_the_file_the_bytes_and_the_rate() {
    assert_eq!(field(DOWNLOAD, TokenKind::Path), "/home/alice/report.pdf");
    assert_eq!(field(DOWNLOAD, TokenKind::Size), "4096");
    assert_eq!(field(DOWNLOAD, TokenKind::Number), "512.00");
}

#[test]
fn an_anonymous_login_colors_the_password_it_gave() {
    assert!(vsftpd::parse_line(ANONYMOUS)
        .unwrap()
        .tokens
        .contains(&Token::new("anon password", TokenKind::Header)));
    assert_eq!(
        field(ANONYMOUS, TokenKind::UserIdentifier),
        "guest@example.org"
    );
}

#[test]
fn a_command_is_colored_as_a_request() {
    assert_eq!(field(COMMAND, TokenKind::Request), "RETR report.pdf");
}

#[test]
fn a_response_colors_its_reply_code() {
    assert_eq!(field(RESPONSE, TokenKind::Status), "226");
}

#[test]
fn a_response_without_a_reply_code_is_message_text() {
    let line = r#"Tue Oct  3 12:00:06 2023 [pid 4322] FTP response: Client "10.0.0.5", "ready""#;

    assert_eq!(field(line, TokenKind::Message), "ready");
}

#[test]
fn a_quoted_value_on_a_login_is_message_text() {
    let line =
        r#"Tue Oct  3 12:00:02 2023 [pid 4320] [alice] FAIL LOGIN: Client "10.0.0.5", "denied""#;

    assert_eq!(field(line, TokenKind::Message), "denied");
}

#[test]
fn empty_brackets_and_quotes_are_only_punctuation() {
    let line = r#"Tue Oct  3 12:00:06 2023 [pid 4322] [] OK MKDIR: Client "", """#;

    assert_eq!(
        vsftpd::parse_line(line).unwrap().tokens[8..10],
        [
            Token::new("[", TokenKind::Punctuation),
            Token::new("]", TokenKind::Punctuation),
        ]
    );
    assert_eq!(vsftpd::parse_line(line).unwrap().text(), line);
    assert!(!kinds(line).contains(&TokenKind::Ip));
    assert!(!kinds(line).contains(&TokenKind::Path));
}

#[test]
fn text_after_the_details_is_message_text() {
    let line =
        r#"Tue Oct  3 12:00:06 2023 [pid 4322] [alice] OK DELETE: Client "10.0.0.5" (retry)"#;

    assert_eq!(field(line, TokenKind::Message), " (retry)");
}

#[test]
fn a_log_file_line_that_is_not_an_event_is_message_text() {
    let line = "Tue Oct  3 12:00:06 2023 [pid 4322] session ended for 10.0.0.5";

    assert_eq!(field(line, TokenKind::Message), "session ended for ");
    assert_eq!(field(line, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_syslog_line_keeps_every_character_of_the_original() {
    assert_eq!(vsftpd::parse_line(SYSLOG).unwrap().text(), SYSLOG);
}

#[test]
fn a_syslog_line_colors_the_program_and_an_ipv6_client() {
    assert_eq!(field(SYSLOG, TokenKind::Tag), "vsftpd");
    assert_eq!(field(SYSLOG, TokenKind::Ip), "::ffff:10.0.0.9");
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert!(vsftpd::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_a_vsftpd_log_is_not_parsed() {
    assert!(vsftpd::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(VsftpdPlugin::new().name(), "vsftpd");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(VsftpdPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        VsftpdPlugin::default().metadata().description,
        "vsftpd FTP server logs"
    );
}

#[test]
fn the_plugin_parses_an_event_into_tokens() {
    let parsed = match VsftpdPlugin::new().parse_line(LOGIN) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the login should parse"),
    };

    assert_eq!(parsed.text(), LOGIN);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        VsftpdPlugin::new().parse_line("not a vsftpd log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        VsftpdPlugin::new().detect_format(&[CONNECT, LOGIN, DOWNLOAD, SYSLOG]),
        1.0
    );
}
