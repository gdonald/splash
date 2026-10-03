use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::super_log::{self, SuperPlugin};

const COMMAND: &str = "alice@web01 Tue Oct  3 12:00:01 2023\tshutdown (-h now)";

const PROGRAM: &str = "super: bob@web01 Tue Oct  3 12:05:00 2023\tbackup ()";

const SYSLOG: &str = "Oct  3 12:00:01 web01 super[4300]: shutdown (-h now)";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    super_log::parse_line(line)
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
    for line in [COMMAND, PROGRAM, SYSLOG] {
        assert_eq!(super_log::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_log_file_line_colors_the_user_host_and_date() {
    assert_eq!(field(COMMAND, TokenKind::UserId), "alice");
    assert_eq!(field(COMMAND, TokenKind::Host), "web01");
    assert_eq!(
        field(COMMAND, TokenKind::Timestamp),
        "Tue Oct  3 12:00:01 2023"
    );
}

#[test]
fn a_command_colors_its_name_and_arguments() {
    assert_eq!(field(COMMAND, TokenKind::Method), "shutdown");
    assert_eq!(field(COMMAND, TokenKind::Request), "-h now");
}

#[test]
fn a_command_with_no_arguments_is_only_its_parentheses() {
    assert!(all(PROGRAM, TokenKind::Request).is_empty());
}

#[test]
fn a_program_name_before_the_user_is_colored_as_a_tag() {
    assert_eq!(
        super_log::parse_line(PROGRAM).unwrap().tokens[..3],
        [
            Token::new("super", TokenKind::Tag),
            Token::new(":", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
        ]
    );
}

#[test]
fn a_program_name_without_a_colon_is_read() {
    let line = "super bob@web01 Tue Oct  3 12:05:00 2023\tbackup (now)";

    assert_eq!(super_log::parse_line(line).unwrap().text(), line);
    assert_eq!(field(line, TokenKind::Tag), "super");
}

#[test]
fn an_error_super_reports_is_message_text() {
    let line = "alice@web01 Tue Oct  3 12:06:00 2023\tPermission denied: you may not run restart";

    assert!(all(line, TokenKind::Method).is_empty());
}

#[test]
fn a_syslog_line_colors_the_command() {
    assert_eq!(field(SYSLOG, TokenKind::Tag), "super");
    assert_eq!(field(SYSLOG, TokenKind::Method), "shutdown");
}

#[test]
fn a_line_from_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice";

    assert!(super_log::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(super_log::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(SuperPlugin::new().name(), "super");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(SuperPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        SuperPlugin::default().metadata().description,
        "super(1) superuser access logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match SuperPlugin::new().parse_line(COMMAND) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), COMMAND);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        SuperPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        SuperPlugin::new().detect_format(&[COMMAND, PROGRAM, SYSLOG]),
        1.0
    );
}
