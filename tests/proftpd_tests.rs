use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::proftpd::{self, ProftpdPlugin};

const LOGIN: &str = concat!(
    "2023-10-03 12:00:01,123 ftp proftpd[4321] ftp.example.com ",
    "(client.example.org[10.0.0.5]): USER alice: Login successful."
);

const FAILED_LOGIN: &str = concat!(
    "2023-10-03 12:00:02,456 ftp proftpd[4322] ftp.example.com ",
    "(10.0.0.9[10.0.0.9]): USER bob (Login failed): Incorrect password"
);

const STARTUP: &str =
    "2023-10-03 12:00:00,001 proftpd[4300]: ProFTPD 1.3.8 standalone mode STARTUP";

const SERVER_ONLY: &str =
    "2023-10-03 12:00:00,002 ftp proftpd[4300] ftp.example.com: ProFTPD killed (signal 15)";

const SYSLOG: &str = concat!(
    "Oct  3 12:00:01 ftp proftpd[4321]: ftp.example.com ",
    "(client.example.org[10.0.0.5]) - ANON anonymous: Login successful."
);

const EXTENDED: &str =
    r#"10.0.0.5 - alice [03/Oct/2023:12:00:03 +0000] "RETR report.pdf" 226 4096"#;

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    proftpd::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    proftpd::parse_line(line)
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
fn a_system_log_line_keeps_every_character_of_the_original() {
    for line in [LOGIN, FAILED_LOGIN, STARTUP, SERVER_ONLY] {
        assert_eq!(proftpd::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_system_log_line_colors_the_date_host_label_and_process_id() {
    assert_eq!(
        field(LOGIN, TokenKind::Timestamp),
        "2023-10-03 12:00:01,123"
    );
    assert_eq!(field(LOGIN, TokenKind::Host), "ftp");
    assert_eq!(field(LOGIN, TokenKind::Tag), "proftpd");
    assert_eq!(field(LOGIN, TokenKind::Pid), "4321");
}

#[test]
fn a_system_log_line_colors_the_server_and_the_client() {
    assert_eq!(field(LOGIN, TokenKind::VirtualHost), "ftp.example.com");
    assert_eq!(
        all(LOGIN, TokenKind::Host),
        vec!["ftp", "client.example.org"]
    );
    assert_eq!(field(LOGIN, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_client_without_a_name_colors_its_address_twice() {
    assert_eq!(
        all(FAILED_LOGIN, TokenKind::Ip),
        vec!["10.0.0.9", "10.0.0.9"]
    );
}

#[test]
fn a_login_colors_the_user() {
    assert_eq!(field(LOGIN, TokenKind::Method), "USER");
    assert_eq!(field(LOGIN, TokenKind::UserId), "alice");
}

#[test]
fn a_successful_login_is_colored_as_a_success() {
    assert_eq!(field(LOGIN, TokenKind::Success), "Login successful");
}

#[test]
fn a_failed_login_is_colored_as_a_failure() {
    assert_eq!(
        all(FAILED_LOGIN, TokenKind::Failure),
        vec!["Login failed", "Incorrect password"]
    );
}

#[test]
fn a_line_without_a_host_or_server_colors_the_label_and_message() {
    assert!(!kinds(STARTUP).contains(&TokenKind::Host));
    assert!(!kinds(STARTUP).contains(&TokenKind::VirtualHost));
    assert_eq!(field(STARTUP, TokenKind::Tag), "proftpd");
}

#[test]
fn a_line_with_a_server_and_no_client_colors_only_the_server() {
    assert_eq!(
        field(SERVER_ONLY, TokenKind::VirtualHost),
        "ftp.example.com"
    );
    assert!(!kinds(SERVER_ONLY).contains(&TokenKind::Ip));
}

#[test]
fn a_client_written_against_the_server_name_keeps_no_gap() {
    let line = "2023-10-03 12:00:01,123 proftpd[4321] ftp.example.com(10.0.0.5[10.0.0.5]): FTP session opened.";

    assert_eq!(proftpd::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_client_with_an_empty_name_and_address_is_only_punctuation() {
    let line = "2023-10-03 12:00:01,123 proftpd[4321] ftp.example.com ([]): FTP session closed.";

    assert_eq!(
        proftpd::parse_line(line).unwrap().tokens[9..13],
        [
            Token::new("(", TokenKind::Punctuation),
            Token::new("[", TokenKind::Punctuation),
            Token::new("]", TokenKind::Punctuation),
            Token::new(")", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_syslog_line_keeps_every_character_of_the_original() {
    assert_eq!(proftpd::parse_line(SYSLOG).unwrap().text(), SYSLOG);
}

#[test]
fn a_syslog_line_colors_the_server_client_and_anonymous_user() {
    assert_eq!(field(SYSLOG, TokenKind::VirtualHost), "ftp.example.com");
    assert_eq!(field(SYSLOG, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(SYSLOG, TokenKind::Method), "ANON");
    assert_eq!(field(SYSLOG, TokenKind::UserId), "anonymous");
}

#[test]
fn a_syslog_line_without_a_server_is_message_text() {
    let line = "Oct  3 12:00:00 ftp proftpd[4300]: ProFTPD killed (signal 15)";

    assert_eq!(
        field(line, TokenKind::Message),
        "ProFTPD killed (signal 15)"
    );
    assert!(!kinds(line).contains(&TokenKind::VirtualHost));
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert!(proftpd::parse_line(line).is_none());
}

#[test]
fn an_extended_log_line_keeps_every_character_of_the_original() {
    assert_eq!(proftpd::parse_line(EXTENDED).unwrap().text(), EXTENDED);
}

#[test]
fn an_extended_log_line_colors_the_command() {
    assert_eq!(field(EXTENDED, TokenKind::Method), "RETR");
}

#[test]
fn a_line_that_is_not_a_proftpd_log_is_not_parsed() {
    assert!(proftpd::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(ProftpdPlugin::new().name(), "proftpd");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(ProftpdPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        ProftpdPlugin::default().metadata().description,
        "ProFTPD system and extended logs"
    );
}

#[test]
fn the_plugin_parses_a_login_into_tokens() {
    let parsed = match ProftpdPlugin::new().parse_line(LOGIN) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the login should parse"),
    };

    assert_eq!(parsed.text(), LOGIN);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        ProftpdPlugin::new().parse_line("not a proftpd log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        ProftpdPlugin::new().detect_format(&[LOGIN, SYSLOG, EXTENDED]),
        1.0
    );
}
