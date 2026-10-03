use splash::oops::{self, OopsPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const BUG: &str = "[ 1234.567890] BUG: kernel NULL pointer dereference, address: 0000000000000008";

const CPU: &str =
    "[ 1234.567945] CPU: 3 PID: 2201 Comm: nginx Tainted: G           O       6.1.0-13-amd64 #1";

const RIP: &str = "[ 1234.567967] RIP: 0010:ext4_do_writepages+0x1a/0x40 [ext4]";

const FRAME: &str = "Oct  3 12:00:03 web01 kernel: [ 1234.568033]  ? __die+0x23/0x70";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    oops::parse_line(line)
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
    for line in [BUG, CPU, RIP, FRAME] {
        assert_eq!(oops::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_headline_is_colored_as_a_failure() {
    assert_eq!(
        all(BUG, TokenKind::Failure),
        vec!["BUG", "kernel NULL pointer dereference"]
    );
}

#[test]
fn an_address_is_colored_as_a_number() {
    assert_eq!(field(BUG, TokenKind::Header), "address");
    assert_eq!(field(BUG, TokenKind::Number), "0000000000000008");
}

#[test]
fn the_cpu_process_and_command_are_colored() {
    assert_eq!(all(CPU, TokenKind::Number), vec!["3", "2201"]);
    assert_eq!(field(CPU, TokenKind::Tag), "nginx");
    assert_eq!(
        all(CPU, TokenKind::Header),
        vec!["CPU", "PID", "Comm", "Tainted"]
    );
}

#[test]
fn the_faulting_function_and_its_module_are_colored() {
    assert_eq!(
        field(RIP, TokenKind::Module),
        "ext4_do_writepages+0x1a/0x40"
    );
    assert_eq!(field(RIP, TokenKind::Tag), "ext4");
}

#[test]
fn a_call_trace_frame_from_syslog_is_read() {
    assert_eq!(field(FRAME, TokenKind::Module), "__die+0x23/0x70");
}

#[test]
fn a_hexadecimal_value_with_its_prefix_is_colored() {
    let line = "#PF: error_code(0x0000) - not-present page";

    assert_eq!(field(line, TokenKind::Number), "0x0000");
}

#[test]
fn a_line_without_a_prefix_is_read() {
    assert_eq!(
        field("Oops: 0000 [#1] PREEMPT SMP NOPTI", TokenKind::Failure),
        "Oops"
    );
}

#[test]
fn a_warning_is_colored_as_a_warning() {
    let line = "[ 2000.000001] WARNING: CPU: 1 PID: 0 at kernel/sched/core.c:123 update_rq_clock+0x12/0x30";

    assert_eq!(field(line, TokenKind::Warning), "WARNING");
}

#[test]
fn a_kernel_line_that_is_not_part_of_an_oops_is_not_parsed() {
    assert!(oops::parse_line("[    1.234567] usb 1-1: new high-speed USB device").is_none());
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(oops::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(OopsPlugin::new().name(), "oops");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(OopsPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        OopsPlugin::default().metadata().description,
        "Linux kernel oops, warning, and panic messages"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match OopsPlugin::new().parse_line(BUG) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), BUG);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        OopsPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        OopsPlugin::new().detect_format(&[BUG, CPU, RIP, FRAME]),
        1.0
    );
}
