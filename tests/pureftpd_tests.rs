use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::pureftpd::{self, PureftpdPlugin};

const CONNECTION: &str =
    "Oct  3 12:00:01 ftp pure-ftpd: (?@10.0.0.5) [INFO] New connection from 10.0.0.5";

const LOGGED_IN: &str = "Oct  3 12:00:02 ftp pure-ftpd: (?@10.0.0.5) [INFO] alice is now logged in";

const DOWNLOAD: &str = concat!(
    "Oct  3 12:00:03 ftp pure-ftpd: (alice@10.0.0.5) [NOTICE] ",
    "/home/alice//report.pdf downloaded  (4096 bytes, 512.00KB/sec)"
);

const PARTIAL: &str = concat!(
    "Oct  3 12:00:04 ftp pure-ftpd: (alice@client.example.org) [NOTICE] ",
    "/home/alice/backup.tar partially uploaded  (1048576 bytes, 1.50MB/sec)"
);

const AUTH_FAILED: &str =
    "Oct  3 12:00:05 ftp pure-ftpd: (?@10.0.0.9) [WARNING] Authentication failed for user [bob]";

const CLF: &str =
    r#"10.0.0.5 - alice [03/Oct/2023:12:00:03 +0000] "GET /home/alice/report.pdf" 200 4096"#;

const W3C_FIELDS: &str =
    "#Fields: date time c-ip cs-method cs-uri-stem sc-status cs-username sc-bytes";

const W3C_RECORD: &str =
    "2023-10-03 12:00:03 10.0.0.5 []sent /home/alice/report.pdf 226 alice 4096";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    pureftpd::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    pureftpd::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_syslog_line_keeps_every_character_of_the_original() {
    for line in [CONNECTION, LOGGED_IN, DOWNLOAD, PARTIAL, AUTH_FAILED] {
        assert_eq!(pureftpd::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_syslog_line_colors_the_program() {
    assert_eq!(field(CONNECTION, TokenKind::Tag), "pure-ftpd");
}

#[test]
fn a_syslog_line_colors_the_user_host_and_level() {
    assert_eq!(field(DOWNLOAD, TokenKind::UserId), "alice");
    assert_eq!(field(DOWNLOAD, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(DOWNLOAD, TokenKind::Level), "NOTICE");
}

#[test]
fn a_session_with_a_host_name_colors_it_as_a_host() {
    assert_eq!(
        pureftpd::parse_line(PARTIAL)
            .unwrap()
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Host)
            .map(|token| token.text.to_string())
            .collect::<Vec<_>>(),
        vec!["ftp", "client.example.org"]
    );
}

#[test]
fn an_address_in_a_message_is_colored() {
    let parsed = pureftpd::parse_line(CONNECTION).unwrap();

    assert_eq!(
        parsed.tokens.last().unwrap(),
        &Token::new("10.0.0.5", TokenKind::Ip)
    );
}

#[test]
fn a_login_colors_the_user_and_the_success() {
    let tokens = pureftpd::parse_line(LOGGED_IN).unwrap().tokens;

    assert_eq!(
        tokens[tokens.len() - 3..],
        [
            Token::new("alice", TokenKind::UserId),
            Token::new(" ", TokenKind::Plain),
            Token::new("is now logged in", TokenKind::Success),
        ]
    );
}

#[test]
fn a_download_colors_the_file_size_and_rate() {
    assert_eq!(field(DOWNLOAD, TokenKind::Path), "/home/alice//report.pdf");
    assert_eq!(field(DOWNLOAD, TokenKind::Size), "4096");
    assert_eq!(field(DOWNLOAD, TokenKind::Number), "512.00");
}

#[test]
fn a_finished_transfer_is_colored_as_a_success() {
    assert_eq!(field(DOWNLOAD, TokenKind::Success), "downloaded");
}

#[test]
fn a_partial_upload_is_colored_as_a_warning() {
    assert_eq!(field(PARTIAL, TokenKind::Warning), "partially uploaded");
}

#[test]
fn a_failed_login_colors_the_failure_and_the_user() {
    assert_eq!(
        field(AUTH_FAILED, TokenKind::Failure),
        "Authentication failed"
    );
    assert_eq!(
        pureftpd::parse_line(AUTH_FAILED).unwrap().tokens[20],
        Token::new("bob", TokenKind::UserId)
    );
}

#[test]
fn a_failed_login_for_an_empty_user_is_only_brackets() {
    let line =
        "Oct  3 12:00:05 ftp pure-ftpd: (?@10.0.0.9) [WARNING] Authentication failed for user []";

    assert_eq!(pureftpd::parse_line(line).unwrap().text(), line);
    assert_eq!(kinds(line).last(), Some(&TokenKind::Punctuation));
}

#[test]
fn a_timeout_is_colored_as_a_warning() {
    let line = "Oct  3 12:00:06 ftp pure-ftpd: (alice@10.0.0.5) [INFO] Timeout - try typing a little faster next time";

    assert_eq!(field(line, TokenKind::Warning), "Timeout");
}

#[test]
fn a_message_without_a_level_has_no_level_token() {
    let line = "Oct  3 12:00:06 ftp pure-ftpd: (alice@10.0.0.5) Logout.";

    assert!(!kinds(line).contains(&TokenKind::Level));
    assert_eq!(field(line, TokenKind::Message), "Logout.");
}

#[test]
fn an_empty_user_and_host_are_only_punctuation() {
    let line = "Oct  3 12:00:06 ftp pure-ftpd: (@) [INFO] Logout.";

    assert_eq!(pureftpd::parse_line(line).unwrap().text(), line);
    assert!(!kinds(line).contains(&TokenKind::UserId));
}

#[test]
fn a_message_without_a_session_is_message_text() {
    let line = "Oct  3 12:00:00 ftp pure-ftpd: Shutting down";

    assert_eq!(field(line, TokenKind::Message), "Shutting down");
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert!(pureftpd::parse_line(line).is_none());
}

#[test]
fn a_clf_transfer_log_line_colors_the_command_and_file() {
    assert_eq!(field(CLF, TokenKind::Method), "GET");
    assert_eq!(field(CLF, TokenKind::Request), "/home/alice/report.pdf");
}

#[test]
fn a_w3c_header_keeps_every_character_of_the_original() {
    assert_eq!(pureftpd::parse_line(W3C_FIELDS).unwrap().text(), W3C_FIELDS);
}

#[test]
fn a_w3c_header_colors_its_name() {
    assert_eq!(field(W3C_FIELDS, TokenKind::Header), "Fields");
}

#[test]
fn a_w3c_header_without_a_value_is_only_its_name() {
    assert_eq!(
        pureftpd::parse_line("#Remark:").unwrap().tokens,
        vec![
            Token::new("#", TokenKind::Punctuation),
            Token::new("Remark", TokenKind::Header),
            Token::new(":", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_w3c_record_keeps_every_character_of_the_original() {
    assert_eq!(pureftpd::parse_line(W3C_RECORD).unwrap().text(), W3C_RECORD);
}

#[test]
fn a_w3c_record_colors_its_fields() {
    assert_eq!(
        field(W3C_RECORD, TokenKind::Timestamp),
        "2023-10-03 12:00:03"
    );
    assert_eq!(field(W3C_RECORD, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(W3C_RECORD, TokenKind::Method), "sent");
    assert_eq!(field(W3C_RECORD, TokenKind::Path), "/home/alice/report.pdf");
    assert_eq!(field(W3C_RECORD, TokenKind::Status), "226");
    assert_eq!(field(W3C_RECORD, TokenKind::UserId), "alice");
    assert_eq!(field(W3C_RECORD, TokenKind::Size), "4096");
}

#[test]
fn a_line_that_is_not_a_pure_ftpd_log_is_not_parsed() {
    assert!(pureftpd::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(PureftpdPlugin::new().name(), "pure-ftpd");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(PureftpdPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        PureftpdPlugin::default().metadata().description,
        "pure-ftpd syslog and transfer logs"
    );
}

#[test]
fn the_plugin_parses_a_download_into_tokens() {
    let parsed = match PureftpdPlugin::new().parse_line(DOWNLOAD) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the download should parse"),
    };

    assert_eq!(parsed.text(), DOWNLOAD);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        PureftpdPlugin::new().parse_line("not a pure-ftpd log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        PureftpdPlugin::new().detect_format(&[CONNECTION, DOWNLOAD, CLF, W3C_FIELDS, W3C_RECORD]),
        1.0
    );
}
