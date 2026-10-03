use splash::ftp;
use splash::output::{Token, TokenKind};

const EXTENDED: &str =
    r#"10.0.0.5 - alice [03/Oct/2023:12:00:03 +0000] "RETR report.pdf" 226 4096"#;

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    ftp::parse_transfer_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn an_ipv4_address_is_colored_as_an_address() {
    assert_eq!(ftp::host_kind("10.0.0.5"), TokenKind::Ip);
}

#[test]
fn an_ipv6_address_is_colored_as_an_address() {
    assert_eq!(ftp::host_kind("::ffff:10.0.0.5"), TokenKind::Ip);
}

#[test]
fn a_host_name_is_colored_as_a_host() {
    assert_eq!(ftp::host_kind("client.example.org"), TokenKind::Host);
}

#[test]
fn a_transfer_line_keeps_every_character_of_the_original() {
    assert_eq!(ftp::parse_transfer_line(EXTENDED).unwrap().text(), EXTENDED);
}

#[test]
fn a_transfer_line_colors_the_remote_host_by_whether_it_is_an_address() {
    assert_eq!(field(EXTENDED, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_transfer_line_colors_the_user_and_the_date() {
    assert_eq!(field(EXTENDED, TokenKind::UserId), "alice");
    assert_eq!(
        field(EXTENDED, TokenKind::Timestamp),
        "[03/Oct/2023:12:00:03 +0000]"
    );
}

#[test]
fn a_transfer_line_colors_the_command_and_its_argument() {
    assert_eq!(field(EXTENDED, TokenKind::Method), "RETR");
    assert_eq!(field(EXTENDED, TokenKind::Request), "report.pdf");
}

#[test]
fn a_transfer_line_colors_the_status_and_the_size() {
    assert_eq!(field(EXTENDED, TokenKind::Status), "226");
    assert_eq!(field(EXTENDED, TokenKind::Size), "4096");
}

#[test]
fn a_command_without_an_argument_is_only_the_command() {
    let line = r#"client.example.org - - [03/Oct/2023:12:00:04 +0000] "PASV" 227 -"#;

    assert_eq!(
        ftp::parse_transfer_line(line).unwrap().tokens[8..11],
        [
            Token::new("\"", TokenKind::Punctuation),
            Token::new("PASV", TokenKind::Method),
            Token::new("\"", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_command_with_a_trailing_space_keeps_the_space() {
    let line = r#"10.0.0.5 - alice [03/Oct/2023:12:00:05 +0000] "NOOP " 200 0"#;

    assert_eq!(ftp::parse_transfer_line(line).unwrap().text(), line);
}

#[test]
fn a_line_of_another_shape_is_not_a_transfer_line() {
    assert!(ftp::parse_transfer_line("the maintenance window moves to 02:00").is_none());
}
