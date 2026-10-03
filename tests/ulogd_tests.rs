use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::ulogd::{self, UlogdPlugin};

const LOGEMU: &str = concat!(
    "Oct  3 12:00:01 fw01 [UFW BLOCK] IN=eth0 OUT= MAC=52:54:00:12:34:56 SRC=203.0.113.7 ",
    "DST=10.0.0.1 LEN=60 TOS=00 TTL=52 ID=54321 DF PROTO=TCP SPT=40022 DPT=22 SYN URGP=0"
);

const KERNEL: &str = concat!(
    "Oct  3 12:00:01 fw01 kernel: [12345.678901] INPUT ACCEPT: IN=eth0 OUT= SRC=10.0.0.5 ",
    "DST=10.0.0.1 LEN=84 PROTO=ICMP TYPE=8 CODE=0"
);

const NO_PREFIX: &str = concat!(
    "Oct  3 12:00:03 fw01 IN=eth1 OUT=eth0 SRC=2001:db8::7 DST=fw.example.org LEN=72 ",
    "PROTO=UDP SPT=53001 DPT=53 UID=1000 GID=1000 MARK=1f FRAG:120 INCOMPLETE [junk]"
);

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    ulogd::parse_line(line)
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
    for line in [LOGEMU, KERNEL, NO_PREFIX] {
        assert_eq!(ulogd::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_logemu_line_colors_its_date_and_host() {
    assert_eq!(field(LOGEMU, TokenKind::Timestamp), "Oct  3 12:00:01");
    assert_eq!(field(LOGEMU, TokenKind::Host), "fw01");
}

#[test]
fn a_blocked_packet_is_colored_as_a_failure() {
    assert_eq!(field(LOGEMU, TokenKind::Failure), "BLOCK");
}

#[test]
fn an_accepted_packet_is_colored_as_a_success() {
    assert_eq!(field(KERNEL, TokenKind::Success), "ACCEPT");
}

#[test]
fn a_kernel_line_colors_the_seconds_since_boot() {
    assert_eq!(
        all(KERNEL, TokenKind::Timestamp),
        vec!["Oct  3 12:00:01", "[12345.678901]"]
    );
}

#[test]
fn the_interfaces_are_colored_as_modules() {
    assert_eq!(all(NO_PREFIX, TokenKind::Module), vec!["eth1", "eth0"]);
}

#[test]
fn an_empty_field_is_only_its_name() {
    assert!(ulogd::parse_line(LOGEMU)
        .unwrap()
        .tokens
        .windows(3)
        .any(|window| window
            == [
                Token::new("OUT", TokenKind::Header),
                Token::new("=", TokenKind::Punctuation),
                Token::new(" ", TokenKind::Plain),
            ]));
}

#[test]
fn the_addresses_are_colored_by_whether_they_are_addresses() {
    assert_eq!(all(LOGEMU, TokenKind::Ip), vec!["203.0.113.7", "10.0.0.1"]);
    assert_eq!(field(NO_PREFIX, TokenKind::Ip), "2001:db8::7");
    assert_eq!(
        all(NO_PREFIX, TokenKind::Host),
        vec!["fw01", "fw.example.org"]
    );
}

#[test]
fn the_protocol_length_and_ports_are_colored() {
    assert_eq!(field(LOGEMU, TokenKind::Protocol), "TCP");
    assert_eq!(field(LOGEMU, TokenKind::Size), "60");
    assert!(all(LOGEMU, TokenKind::Number).contains(&"22".to_string()));
}

#[test]
fn the_user_and_group_are_colored_as_users() {
    assert_eq!(all(NO_PREFIX, TokenKind::UserId), vec!["1000", "1000"]);
}

#[test]
fn the_prefix_text_and_hardware_address_are_message_text() {
    assert_eq!(
        all(LOGEMU, TokenKind::Message),
        vec!["[UFW ", "] ", "52:54:00:12:34:56"]
    );
}

#[test]
fn bare_flags_are_colored_as_tags() {
    assert_eq!(all(LOGEMU, TokenKind::Tag), vec!["DF", "SYN"]);
    assert_eq!(
        all(NO_PREFIX, TokenKind::Tag),
        vec!["FRAG:120", "INCOMPLETE"]
    );
}

#[test]
fn a_word_that_is_neither_a_field_nor_a_flag_is_message_text() {
    assert_eq!(all(NO_PREFIX, TokenKind::Message), vec!["[junk]"]);
}

#[test]
fn a_line_without_packet_fields_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 nginx[2200]: worker process 2201 exited";

    assert!(ulogd::parse_line(line).is_none());
}

#[test]
fn a_line_without_a_header_is_not_parsed() {
    assert!(ulogd::parse_line("IN=eth0 OUT= SRC=10.0.0.5").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(UlogdPlugin::new().name(), "ulogd");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(UlogdPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        UlogdPlugin::default().metadata().description,
        "Netfilter packet logs from ulogd and the kernel"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match UlogdPlugin::new().parse_line(LOGEMU) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), LOGEMU);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        UlogdPlugin::new().parse_line("not a packet log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        UlogdPlugin::new().detect_format(&[LOGEMU, KERNEL, NO_PREFIX]),
        1.0
    );
}
