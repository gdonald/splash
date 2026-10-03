use splash::auth::{self, AuthPlugin};
use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};

const ACCEPTED: &str =
    "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice from 10.0.0.5 port 52144 ssh2";

const SESSION: &str = concat!(
    "Oct  3 12:00:01 web01 sshd[4101]: pam_unix(sshd:session): ",
    "session opened for user alice(uid=1000) by (uid=0)"
);

const INVALID: &str = concat!(
    "Oct  3 12:00:02 web01 sshd[4102]: Failed password for invalid user admin ",
    "from 203.0.113.7 port 40022 ssh2"
);

const SUDO: &str = concat!(
    "Oct  3 12:00:05 web01 sudo:    alice : TTY=pts/0 ; PWD=/home/alice ; USER=root ; ",
    "COMMAND=/usr/bin/apt update"
);

const AUTH_FAILURE: &str = concat!(
    "Oct  3 12:00:06 web01 sudo: pam_unix(sudo:auth): authentication failure; ",
    "logname=bob uid=1001 euid=0 tty=/dev/pts/1 ruser=bob rhost=  user=bob"
);

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    auth::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    auth::parse_line(line)
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
    for line in [ACCEPTED, SESSION, INVALID, SUDO, AUTH_FAILURE] {
        assert_eq!(auth::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_accepted_login_is_colored_as_a_success() {
    assert_eq!(field(ACCEPTED, TokenKind::Success), "Accepted");
}

#[test]
fn a_login_colors_the_user_address_and_port() {
    assert_eq!(field(ACCEPTED, TokenKind::UserId), "alice");
    assert_eq!(field(ACCEPTED, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(ACCEPTED, TokenKind::Number), "52144");
}

#[test]
fn a_pam_line_colors_the_module_and_what_it_ran_for() {
    assert_eq!(
        all(SESSION, TokenKind::Module),
        vec!["pam_unix", "sshd:session"]
    );
}

#[test]
fn a_pam_module_with_nothing_in_parentheses_is_only_the_module() {
    let line = "Oct  3 12:00:01 web01 login[900]: pam_unix(): bad call";

    assert_eq!(all(line, TokenKind::Module), vec!["pam_unix"]);
}

#[test]
fn an_opened_session_is_colored_as_a_success_and_names_its_user() {
    assert_eq!(field(SESSION, TokenKind::Success), "session opened");
    assert_eq!(field(SESSION, TokenKind::UserId), "alice");
}

#[test]
fn a_user_id_in_parentheses_is_colored_as_a_number() {
    assert_eq!(all(SESSION, TokenKind::Number), vec!["1000", "0"]);
}

#[test]
fn a_failed_password_for_an_invalid_user_is_colored_as_failures() {
    assert_eq!(
        all(INVALID, TokenKind::Failure),
        vec!["Failed password", "for invalid user"]
    );
    assert_eq!(field(INVALID, TokenKind::UserId), "admin");
}

#[test]
fn a_user_name_does_not_take_the_period_ending_a_sentence() {
    let line = "Oct  3 12:00:04 web01 systemd-logind[700]: New session 12 of user alice.";

    assert_eq!(field(line, TokenKind::UserId), "alice");
}

#[test]
fn a_user_closing_a_connection_while_authenticating_is_colored() {
    let line = concat!(
        "Oct  3 12:00:03 web01 sshd[4103]: Connection closed by authenticating user root ",
        "203.0.113.7 port 40100 [preauth]"
    );

    assert_eq!(field(line, TokenKind::Warning), "Connection closed");
    assert_eq!(field(line, TokenKind::UserId), "root");
}

#[test]
fn a_sudo_line_colors_the_user_who_ran_it() {
    assert_eq!(
        auth::parse_line(SUDO).unwrap().tokens[7..10],
        [
            Token::new("   ", TokenKind::Plain),
            Token::new("alice", TokenKind::UserId),
            Token::new(" : ", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_sudo_line_without_indent_colors_the_user_who_ran_it() {
    let line = "Oct  3 12:00:05 web01 sudo: alice : TTY=pts/0 ; COMMAND=/bin/ls";

    assert_eq!(field(line, TokenKind::UserId), "alice");
}

#[test]
fn a_sudo_line_colors_the_directory_target_user_and_command() {
    assert_eq!(field(SUDO, TokenKind::Path), "/home/alice");
    assert_eq!(all(SUDO, TokenKind::UserId), vec!["alice", "root"]);
    assert_eq!(field(SUDO, TokenKind::Request), "/usr/bin/apt update");
}

#[test]
fn a_sudo_line_without_a_user_is_read_as_a_message() {
    let line = "Oct  3 12:00:05 web01 sudo: unable to resolve host web01";

    assert!(!kinds(line).contains(&TokenKind::UserId));
}

#[test]
fn an_authentication_failure_colors_its_fields() {
    assert_eq!(
        field(AUTH_FAILURE, TokenKind::Failure),
        "authentication failure"
    );
    assert_eq!(
        all(AUTH_FAILURE, TokenKind::UserId),
        vec!["bob", "bob", "bob"]
    );
    assert_eq!(all(AUTH_FAILURE, TokenKind::Number), vec!["1001", "0"]);
}

#[test]
fn an_empty_field_value_is_only_its_name() {
    assert!(auth::parse_line(AUTH_FAILURE)
        .unwrap()
        .tokens
        .windows(3)
        .any(|window| window
            == [
                Token::new("rhost", TokenKind::Header),
                Token::new("=", TokenKind::Punctuation),
                Token::new("  ", TokenKind::Message),
            ]));
}

#[test]
fn a_remote_host_is_colored_by_whether_it_is_an_address() {
    let line = "Oct  3 12:00:06 web01 sshd[4102]: pam_unix(sshd:auth): authentication failure; rhost=203.0.113.7";

    assert_eq!(field(line, TokenKind::Ip), "203.0.113.7");
}

#[test]
fn a_failed_su_colors_the_target_user() {
    let line = "Oct  3 12:00:08 web01 su[4200]: FAILED SU (to root) bob on pts/1";

    assert_eq!(field(line, TokenKind::Failure), "FAILED SU");
    assert_eq!(field(line, TokenKind::UserId), "root");
}

#[test]
fn a_line_without_a_syslog_header_is_not_parsed() {
    assert!(auth::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(AuthPlugin::new().name(), "auth");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(AuthPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        AuthPlugin::default().metadata().description,
        "Authentication logs from sshd, sudo, su, and PAM"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match AuthPlugin::new().parse_line(ACCEPTED) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), ACCEPTED);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        AuthPlugin::new().parse_line("not an auth line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        AuthPlugin::new().detect_format(&[ACCEPTED, SESSION, INVALID, SUDO]),
        1.0
    );
}
