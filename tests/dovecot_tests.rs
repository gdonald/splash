use splash::dovecot::{self, DovecotPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const LOGIN: &str = concat!(
    "Oct  3 12:00:01 mail dovecot: imap-login: Login: user=<alice>, method=PLAIN, ",
    "rip=10.0.0.5, lip=10.0.0.1, mpid=4321, TLS, session=<Xy7AbC>"
);

const LOGOUT: &str =
    "Oct  3 12:00:05 mail dovecot: imap(alice)<4321><Xy7AbC>: Logged out in=1024 out=65536";

const AUTH_FAILED: &str = concat!(
    "Oct  3 12:01:10 mail dovecot: pop3-login: Disconnected (auth failed, 1 attempts in 2 secs): ",
    "user=<bob>, method=PLAIN, rip=192.0.2.10, lip=10.0.0.1, TLS, session=<Zq8CdE>"
);

const WORKER_ERROR: &str = concat!(
    "Oct  3 12:01:11 mail dovecot: auth-worker(4400): Error: pam(bob,192.0.2.10): ",
    "Password mismatch"
);

const LOG_FILE: &str = concat!(
    "Oct 03 12:00:01 imap-login: Info: Login: user=<alice>, method=PLAIN, ",
    "rip=10.0.0.5, lip=10.0.0.1, mpid=4321, TLS, session=<Xy7AbC>"
);

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    dovecot::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    dovecot::parse_line(line)
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
fn a_login_line_keeps_every_character_of_the_original() {
    assert_eq!(dovecot::parse_line(LOGIN).unwrap().text(), LOGIN);
}

#[test]
fn a_login_line_colors_the_service_that_wrote_it() {
    assert_eq!(field(LOGIN, TokenKind::Module), "imap-login");
}

#[test]
fn a_login_line_colors_the_login_as_a_success() {
    assert_eq!(field(LOGIN, TokenKind::Success), "Login");
}

#[test]
fn a_login_line_colors_the_user() {
    assert_eq!(field(LOGIN, TokenKind::UserId), "alice");
}

#[test]
fn a_login_line_colors_the_authentication_method() {
    assert_eq!(field(LOGIN, TokenKind::Protocol), "PLAIN");
}

#[test]
fn a_login_line_colors_the_remote_and_local_addresses() {
    assert_eq!(all(LOGIN, TokenKind::Ip), vec!["10.0.0.5", "10.0.0.1"]);
}

#[test]
fn a_login_line_colors_the_mail_process_id() {
    assert_eq!(field(LOGIN, TokenKind::Pid), "4321");
}

#[test]
fn a_login_line_colors_the_session_id() {
    assert_eq!(field(LOGIN, TokenKind::Transaction), "Xy7AbC");
}

#[test]
fn a_line_from_a_service_with_a_user_colors_the_user_process_and_session() {
    assert_eq!(field(LOGOUT, TokenKind::UserId), "alice");
    assert_eq!(field(LOGOUT, TokenKind::Pid), "4321");
    assert_eq!(field(LOGOUT, TokenKind::Transaction), "Xy7AbC");
}

#[test]
fn a_line_from_a_service_with_a_user_keeps_every_character_of_the_original() {
    assert_eq!(dovecot::parse_line(LOGOUT).unwrap().text(), LOGOUT);
}

#[test]
fn a_logout_line_colors_the_bytes_read_and_written() {
    assert_eq!(all(LOGOUT, TokenKind::Size), vec!["1024", "65536"]);
}

#[test]
fn a_failed_login_colors_the_failure() {
    assert_eq!(field(AUTH_FAILED, TokenKind::Failure), "auth failed");
}

#[test]
fn a_service_with_a_process_id_in_parentheses_colors_it_as_a_process_id() {
    assert_eq!(field(WORKER_ERROR, TokenKind::Pid), "4400");
}

#[test]
fn a_service_with_a_process_id_in_parentheses_has_no_user() {
    assert!(!kinds(WORKER_ERROR).contains(&TokenKind::UserId));
}

#[test]
fn a_line_with_a_level_colors_it() {
    assert_eq!(field(WORKER_ERROR, TokenKind::Level), "Error");
}

#[test]
fn a_password_mismatch_is_colored_as_a_failure() {
    assert_eq!(field(WORKER_ERROR, TokenKind::Failure), "Password mismatch");
}

#[test]
fn an_address_inside_the_message_is_colored() {
    assert_eq!(field(WORKER_ERROR, TokenKind::Ip), "192.0.2.10");
}

#[test]
fn a_line_from_the_log_file_keeps_every_character_of_the_original() {
    assert_eq!(dovecot::parse_line(LOG_FILE).unwrap().text(), LOG_FILE);
}

#[test]
fn a_line_from_the_log_file_colors_its_timestamp() {
    assert_eq!(field(LOG_FILE, TokenKind::Timestamp), "Oct 03 12:00:01");
}

#[test]
fn a_line_from_the_log_file_has_no_host() {
    assert!(!kinds(LOG_FILE).contains(&TokenKind::Host));
}

#[test]
fn a_line_from_the_log_file_colors_its_level() {
    assert_eq!(field(LOG_FILE, TokenKind::Level), "Info");
}

#[test]
fn a_saved_mail_is_colored_as_a_success() {
    let line = concat!(
        "Oct  3 12:02:00 mail dovecot: lmtp(alice)<4500><Pk2LmN>: ",
        "msgid=<20231003120200.abc@example.com>: saved mail to INBOX"
    );

    assert_eq!(field(line, TokenKind::Success), "saved mail to");
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert!(dovecot::parse_line(line).is_none());
}

#[test]
fn a_dovecot_line_with_no_service_is_not_parsed() {
    assert!(dovecot::parse_line("Oct  3 12:00:01 mail dovecot: Shutting down").is_none());
}

#[test]
fn a_line_that_is_not_a_dovecot_log_is_not_parsed() {
    assert!(dovecot::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(DovecotPlugin::new().name(), "dovecot");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(DovecotPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        DovecotPlugin::default().metadata().description,
        "Dovecot IMAP, POP3, and LMTP logs"
    );
}

#[test]
fn the_plugin_parses_a_login_line_into_tokens() {
    let parsed = match DovecotPlugin::new().parse_line(LOGIN) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the login line should parse"),
    };

    assert_eq!(parsed.text(), LOGIN);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        DovecotPlugin::new().parse_line("not a dovecot log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        DovecotPlugin::new().detect_format(&[LOGIN, LOGOUT, AUTH_FAILED, WORKER_ERROR, LOG_FILE]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(DovecotPlugin::new().detect_format(&["hello", "world"]), 0.0);
}
