use splash::dmesg::{self, DmesgPlugin};
use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};

const DEFAULT: &str = "[    1.234567] usb 1-1: new high-speed USB device number 2 using xhci_hcd";

const DECODED: &str =
    "kern  :err   : [ 1234.567890] blk_update_request: I/O error, dev sdb, sector 2048";

const RAW: &str = "<4>[    4.567890] ACPI Warning: SystemIO range conflicts with OpRegion";

const CTIME: &str = "[Tue Oct  3 12:00:01 2023] e1000e 0000:00:19.0 eth0: NIC Link is Down";

const ISO: &str = "2023-10-03T12:00:02,123456+0000 EXT4-fs (sda1): mounted filesystem";

const SYSLOG: &str =
    "Oct  3 12:00:03 web01 kernel: [ 3456.789012] Out of memory: Killed process 2201 (nginx)";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    dmesg::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    dmesg::parse_line(line)
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
    for line in [DEFAULT, DECODED, RAW, CTIME, ISO, SYSLOG] {
        assert_eq!(dmesg::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_line_colors_the_seconds_since_boot() {
    assert_eq!(field(DEFAULT, TokenKind::Timestamp), "[    1.234567]");
}

#[test]
fn a_line_colors_the_subsystem_it_opens_with() {
    assert_eq!(field(DEFAULT, TokenKind::Module), "usb 1-1");
}

#[test]
fn a_device_with_its_bus_address_is_one_subsystem() {
    assert_eq!(field(CTIME, TokenKind::Module), "e1000e 0000:00:19.0 eth0");
}

#[test]
fn a_decoded_line_colors_the_facility_and_level() {
    assert_eq!(
        dmesg::parse_line(DECODED).unwrap().tokens[..8],
        [
            Token::new("kern", TokenKind::Module),
            Token::new("  ", TokenKind::Plain),
            Token::new(":", TokenKind::Punctuation),
            Token::new("err", TokenKind::Failure),
            Token::new("   ", TokenKind::Plain),
            Token::new(":", TokenKind::Punctuation),
            Token::new(" ", TokenKind::Plain),
            Token::new("[ 1234.567890]", TokenKind::Timestamp),
        ]
    );
}

#[test]
fn a_warning_level_is_colored_as_a_warning() {
    let line = "kern  :warn  : [    4.567890] ACPI: conflict";

    assert_eq!(field(line, TokenKind::Warning), "warn");
}

#[test]
fn an_informational_level_is_colored_as_a_level() {
    let line = "daemon:info  : started";

    assert_eq!(field(line, TokenKind::Level), "info");
}

#[test]
fn a_decoded_level_without_padding_is_read() {
    let line = "kern:notice: [    0.000000] Linux version 6.1.0";

    assert_eq!(dmesg::parse_line(line).unwrap().text(), line);
    assert_eq!(field(line, TokenKind::Level), "notice");
}

#[test]
fn a_raw_priority_is_colored_by_its_severity() {
    assert_eq!(field(RAW, TokenKind::Warning), "4");
}

#[test]
fn a_raw_priority_alone_is_enough_to_read_a_line() {
    let line = "<6>Linux version 6.1.0";

    assert_eq!(dmesg::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_subsystem_holding_a_problem_word_is_colored_as_words() {
    assert_eq!(all(RAW, TokenKind::Warning), vec!["4", "Warning"]);
    assert!(!kinds(RAW).contains(&TokenKind::Module));
}

#[test]
fn a_human_readable_date_is_colored_as_a_timestamp() {
    assert_eq!(
        field(CTIME, TokenKind::Timestamp),
        "[Tue Oct  3 12:00:01 2023]"
    );
}

#[test]
fn an_iso_date_is_colored_as_a_timestamp() {
    assert_eq!(
        field(ISO, TokenKind::Timestamp),
        "2023-10-03T12:00:02,123456+0000"
    );
}

#[test]
fn a_kernel_line_from_syslog_colors_both_timestamps() {
    assert_eq!(
        all(SYSLOG, TokenKind::Timestamp),
        vec!["Oct  3 12:00:03", "[ 3456.789012]"]
    );
}

#[test]
fn a_kernel_line_from_syslog_without_uptime_is_read() {
    let line = "Oct  3 12:00:03 web01 kernel: Linux version 6.1.0";

    assert_eq!(field(line, TokenKind::Tag), "kernel");
}

#[test]
fn a_problem_is_colored_by_how_severe_it_is() {
    assert_eq!(field(DECODED, TokenKind::Failure), "err");
    assert_eq!(all(DECODED, TokenKind::Failure), vec!["err", "I/O error"]);
    assert_eq!(field(SYSLOG, TokenKind::Failure), "Out of memory");
}

#[test]
fn a_link_coming_up_is_colored_as_a_success() {
    let line = "[    3.456789] e1000e 0000:00:19.0 eth0: NIC Link is Up 1000 Mbps Full Duplex";

    assert_eq!(field(line, TokenKind::Success), "Link is Up");
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 nginx[2200]: worker process 2201 exited";

    assert!(dmesg::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_a_dmesg_line_is_not_parsed() {
    assert!(dmesg::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(DmesgPlugin::new().name(), "dmesg");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(DmesgPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        DmesgPlugin::default().metadata().description,
        "Kernel ring buffer messages from dmesg"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match DmesgPlugin::new().parse_line(DEFAULT) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), DEFAULT);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        DmesgPlugin::new().parse_line("not a dmesg line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        DmesgPlugin::new().detect_format(&[DEFAULT, DECODED, RAW, CTIME, ISO, SYSLOG]),
        1.0
    );
}
