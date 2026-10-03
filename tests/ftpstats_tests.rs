use splash::ftpstats::{self, FtpstatsPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const DOWNLOAD: &str = "1696334401 651c0b41.10e1 alice 10.0.0.5 D 4096 2 /home/alice/report.pdf";

const UPLOAD: &str =
    "1696334460 651c0b41.10e1 alice client.example.org U 512 0 /home/alice/notes.txt";

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    ftpstats::parse_line(line)
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
    assert_eq!(ftpstats::parse_line(DOWNLOAD).unwrap().text(), DOWNLOAD);
}

#[test]
fn a_transfer_colors_the_time_and_the_session() {
    assert_eq!(field(DOWNLOAD, TokenKind::Timestamp), "1696334401");
    assert_eq!(field(DOWNLOAD, TokenKind::Transaction), "651c0b41.10e1");
}

#[test]
fn a_transfer_colors_the_user() {
    assert_eq!(field(DOWNLOAD, TokenKind::UserId), "alice");
}

#[test]
fn a_transfer_colors_a_remote_address_as_an_address() {
    assert_eq!(field(DOWNLOAD, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_transfer_colors_a_remote_host_name_as_a_host() {
    assert_eq!(field(UPLOAD, TokenKind::Host), "client.example.org");
}

#[test]
fn a_transfer_colors_the_direction() {
    assert_eq!(field(DOWNLOAD, TokenKind::Method), "D");
    assert_eq!(field(UPLOAD, TokenKind::Method), "U");
}

#[test]
fn a_transfer_colors_the_size_time_and_file() {
    assert_eq!(field(DOWNLOAD, TokenKind::Size), "4096");
    assert_eq!(field(DOWNLOAD, TokenKind::Duration), "2");
    assert_eq!(field(DOWNLOAD, TokenKind::Path), "/home/alice/report.pdf");
}

#[test]
fn a_transfer_without_a_file_name_has_no_path() {
    let line = "1696334401 651c0b41.10e1 alice 10.0.0.5 D 0 0 ";
    let parsed = ftpstats::parse_line(line).unwrap();

    assert_eq!(parsed.text(), line);
    assert!(!parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Path));
}

#[test]
fn a_line_that_is_not_an_ftpstats_line_is_not_parsed() {
    assert!(ftpstats::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(FtpstatsPlugin::new().name(), "ftpstats");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(FtpstatsPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        FtpstatsPlugin::default().metadata().description,
        "pure-ftpd ftpstats transfer logs"
    );
}

#[test]
fn the_plugin_parses_a_transfer_into_tokens() {
    let parsed = match FtpstatsPlugin::new().parse_line(DOWNLOAD) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the transfer should parse"),
    };

    assert_eq!(parsed.text(), DOWNLOAD);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        FtpstatsPlugin::new().parse_line("not an ftpstats line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        FtpstatsPlugin::new().detect_format(&[DOWNLOAD, UPLOAD]),
        1.0
    );
}
