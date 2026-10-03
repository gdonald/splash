use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::ssh::{self, SshPlugin};

const ACCEPTED: &str = "Oct  3 12:00:01 web01 sshd[4101]: Accepted publickey for alice from 10.0.0.5 port 52144 ssh2: ED25519 SHA256:4a2Xb9QeZ0kR7pYt3sLmN1cVwH8uJfD6gB5oK2iE0Aq";

const FAILED: &str = "Oct  3 12:00:02 web01 sshd[4102]: Failed password for invalid user admin from 203.0.113.7 port 40022 ssh2";

const POSTPONED: &str = "Oct  3 12:00:04 web01 sshd-session[4104]: Postponed keyboard-interactive/pam for bob from 10.0.0.6 port 52150 ssh2 [preauth]";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    ssh::parse_line(line)
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
    for line in [ACCEPTED, FAILED, POSTPONED] {
        assert_eq!(ssh::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_accepted_login_colors_its_outcome_and_method() {
    assert_eq!(field(ACCEPTED, TokenKind::Success), "Accepted");
    assert_eq!(
        all(ACCEPTED, TokenKind::Protocol),
        vec!["publickey", "ssh2"]
    );
}

#[test]
fn an_accepted_login_colors_the_user_address_and_port() {
    assert_eq!(field(ACCEPTED, TokenKind::UserId), "alice");
    assert_eq!(field(ACCEPTED, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(ACCEPTED, TokenKind::Number), "52144");
}

#[test]
fn a_key_colors_its_type_and_fingerprint() {
    assert_eq!(field(ACCEPTED, TokenKind::Module), "ED25519");
    assert_eq!(
        field(ACCEPTED, TokenKind::Transaction),
        "SHA256:4a2Xb9QeZ0kR7pYt3sLmN1cVwH8uJfD6gB5oK2iE0Aq"
    );
}

#[test]
fn a_failed_login_for_an_invalid_user_is_colored_as_failures() {
    assert_eq!(
        all(FAILED, TokenKind::Failure),
        vec!["Failed", "for invalid user"]
    );
    assert_eq!(field(FAILED, TokenKind::UserId), "admin");
}

#[test]
fn a_postponed_login_is_colored_as_a_warning() {
    assert_eq!(field(POSTPONED, TokenKind::Warning), "Postponed");
    assert_eq!(
        field(POSTPONED, TokenKind::Protocol),
        "keyboard-interactive/pam"
    );
}

#[test]
fn a_line_from_before_authentication_colors_its_tag() {
    assert_eq!(
        all(POSTPONED, TokenKind::Tag),
        vec!["sshd-session", "[preauth]"]
    );
}

#[test]
fn a_line_from_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:05 web01 sudo:    alice : COMMAND=/bin/ls";

    assert!(ssh::parse_line(line).is_none());
}

#[test]
fn a_server_start_is_colored_as_a_success() {
    let line = "Oct  3 12:00:00 web01 sshd[700]: Server listening on 0.0.0.0 port 22.";

    assert!(ssh::parse_line(line)
        .unwrap()
        .tokens
        .contains(&Token::new("Server listening on", TokenKind::Success)));
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(ssh::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(SshPlugin::new().name(), "ssh");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(SshPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        SshPlugin::default().metadata().description,
        "OpenSSH server logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match SshPlugin::new().parse_line(ACCEPTED) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), ACCEPTED);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        SshPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        SshPlugin::new().detect_format(&[ACCEPTED, FAILED, POSTPONED]),
        1.0
    );
}
