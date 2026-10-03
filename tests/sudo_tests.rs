use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::sudo::{self, SudoPlugin};

const SUDO: &str = "Oct  3 12:00:05 web01 sudo:    alice : TTY=pts/0 ; PWD=/home/alice ; USER=root ; COMMAND=/usr/bin/apt update";

const SU: &str = "Oct  3 12:00:09 web01 su[4201]: FAILED SU (to root) bob on pts/1";

const SWITCH: &str = "Oct  3 12:00:10 web01 su[4202]: + /dev/pts/1 alice:root";

const LOG_FILE: &str =
    "Oct  3 12:00:07 2023 : bob : user NOT in sudoers ; TTY=pts/1 ; USER=root ; COMMAND=/bin/bash";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    sudo::parse_line(line)
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
    for line in [SUDO, SU, SWITCH, LOG_FILE] {
        assert_eq!(sudo::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_sudo_line_colors_the_user_who_ran_it_and_the_command() {
    assert_eq!(all(SUDO, TokenKind::UserId), vec!["alice", "root"]);
    assert_eq!(field(SUDO, TokenKind::Request), "/usr/bin/apt update");
}

#[test]
fn a_failed_su_colors_the_failure_and_target_user() {
    assert_eq!(field(SU, TokenKind::Failure), "FAILED SU");
    assert_eq!(field(SU, TokenKind::UserId), "root");
}

#[test]
fn a_successful_switch_colors_its_terminal_and_users() {
    assert_eq!(
        sudo::parse_line(SWITCH).unwrap().tokens[10..],
        [
            Token::new("+", TokenKind::Success),
            Token::new(" ", TokenKind::Plain),
            Token::new("/dev/pts/1", TokenKind::Path),
            Token::new(" ", TokenKind::Plain),
            Token::new("alice", TokenKind::UserId),
            Token::new(":", TokenKind::Punctuation),
            Token::new("root", TokenKind::UserId),
        ]
    );
}

#[test]
fn a_failed_switch_is_colored_as_a_failure() {
    let line = "Oct  3 12:00:11 web01 su[4203]: - /dev/pts/2 bob:root";

    assert_eq!(field(line, TokenKind::Failure), "-");
}

#[test]
fn a_log_file_line_colors_its_date_and_user() {
    assert_eq!(
        field(LOG_FILE, TokenKind::Timestamp),
        "Oct  3 12:00:07 2023"
    );
    assert_eq!(field(LOG_FILE, TokenKind::UserId), "bob");
    assert_eq!(field(LOG_FILE, TokenKind::Failure), "NOT in sudoers");
}

#[test]
fn a_log_file_line_without_a_year_is_read() {
    let line = "Oct  3 12:00:05 : alice : TTY=pts/0 ; COMMAND=/bin/ls";

    assert_eq!(field(line, TokenKind::Timestamp), "Oct  3 12:00:05");
}

#[test]
fn a_line_from_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice";

    assert!(sudo::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(sudo::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(SudoPlugin::new().name(), "sudo");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(SudoPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        SudoPlugin::default().metadata().description,
        "sudo and su privilege escalation logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match SudoPlugin::new().parse_line(SUDO) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), SUDO);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        SudoPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        SudoPlugin::new().detect_format(&[SUDO, SU, SWITCH, LOG_FILE]),
        1.0
    );
}
