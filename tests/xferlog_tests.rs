use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::xferlog::{self, XferlogPlugin};

const COMPLETE: &str =
    "Tue Oct  3 12:00:01 2023 2 10.0.0.5 4096 /home/alice/report.pdf b _ o r alice ftp 0 * c";

const INCOMPLETE: &str = concat!(
    "Tue Oct  3 12:00:09 2023 0 client.example.org 0 /pub/release notes.txt ",
    "a _ i a guest@example.org ftp 0 * i"
);

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    xferlog::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_transfer_keeps_every_character_of_the_original() {
    assert_eq!(xferlog::parse_line(COMPLETE).unwrap().text(), COMPLETE);
}

#[test]
fn a_transfer_colors_the_date_and_the_transfer_time() {
    assert_eq!(
        field(COMPLETE, TokenKind::Timestamp),
        "Tue Oct  3 12:00:01 2023"
    );
    assert_eq!(field(COMPLETE, TokenKind::Duration), "2");
}

#[test]
fn a_transfer_colors_a_remote_address_as_an_address() {
    assert_eq!(field(COMPLETE, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_transfer_colors_a_remote_host_name_as_a_host() {
    assert_eq!(field(INCOMPLETE, TokenKind::Host), "client.example.org");
}

#[test]
fn a_transfer_colors_the_size_and_the_file() {
    assert_eq!(field(COMPLETE, TokenKind::Size), "4096");
    assert_eq!(field(COMPLETE, TokenKind::Path), "/home/alice/report.pdf");
}

#[test]
fn a_file_name_holding_a_space_is_one_path() {
    assert_eq!(field(INCOMPLETE, TokenKind::Path), "/pub/release notes.txt");
}

#[test]
fn a_transfer_colors_the_type_flag_direction_and_access_mode() {
    assert_eq!(field(COMPLETE, TokenKind::Protocol), "b");
    assert_eq!(field(COMPLETE, TokenKind::Tag), "_");
    assert_eq!(field(COMPLETE, TokenKind::Method), "o");
    assert_eq!(field(COMPLETE, TokenKind::Level), "r");
}

#[test]
fn a_transfer_colors_the_user_and_the_service() {
    assert_eq!(field(COMPLETE, TokenKind::UserId), "alice");
    assert_eq!(field(COMPLETE, TokenKind::Module), "ftp");
}

#[test]
fn a_transfer_colors_the_authentication_method_and_user_id() {
    assert_eq!(field(COMPLETE, TokenKind::Number), "0");
    assert_eq!(field(COMPLETE, TokenKind::UserIdentifier), "*");
}

#[test]
fn a_completed_transfer_is_colored_as_a_success() {
    assert_eq!(field(COMPLETE, TokenKind::Success), "c");
}

#[test]
fn an_incomplete_transfer_is_colored_as_a_failure() {
    assert_eq!(field(INCOMPLETE, TokenKind::Failure), "i");
}

#[test]
fn a_line_that_is_not_an_xferlog_line_is_not_parsed() {
    assert!(xferlog::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(XferlogPlugin::new().name(), "xferlog");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(XferlogPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        XferlogPlugin::default().metadata().description,
        "Generic xferlog FTP transfer logs"
    );
}

#[test]
fn the_plugin_parses_a_transfer_into_tokens() {
    let parsed = match XferlogPlugin::new().parse_line(COMPLETE) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the transfer should parse"),
    };

    assert_eq!(parsed.text(), COMPLETE);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        XferlogPlugin::new().parse_line("not an xferlog line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        XferlogPlugin::new().detect_format(&[COMPLETE, INCOMPLETE]),
        1.0
    );
}
