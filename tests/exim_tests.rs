use splash::exim::{self, EximPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const ARRIVAL: &str = concat!(
    "2023-10-03 12:00:01 1qnXYZ-000ABC-12 <= alice@example.com H=mail.example.com [10.0.0.5] ",
    "P=esmtps S=4512 id=20231003120001.abc@example.com"
);

const DELIVERY: &str = concat!(
    "2023-10-03 12:00:02 1qnXYZ-000ABC-12 => bob@example.org R=dnslookup T=remote_smtp ",
    r#"H=mx.example.org [93.184.216.34] C="250 2.0.0 OK""#
);

const COMPLETED: &str = "2023-10-03 12:00:02 1qnXYZ-000ABC-12 Completed";

const CONNECTION: &str = concat!(
    "2023-10-03 12:05:12.345 +0000 [4321] SMTP connection from [192.0.2.10]:51234 ",
    "(TCP/IP connection count = 1)"
);

const SYSLOG: &str = concat!(
    "Oct  3 12:00:01 mail exim[4321]: 1qnXYZ-000ABC-12 => bob@example.org ",
    "R=dnslookup T=remote_smtp"
);

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    exim::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    exim::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn an_arrival_line_keeps_every_character_of_the_original() {
    assert_eq!(exim::parse_line(ARRIVAL).unwrap().text(), ARRIVAL);
}

#[test]
fn an_arrival_line_colors_the_timestamp() {
    assert_eq!(field(ARRIVAL, TokenKind::Timestamp), "2023-10-03 12:00:01");
}

#[test]
fn an_arrival_line_colors_the_message_id() {
    assert_eq!(field(ARRIVAL, TokenKind::QueueId), "1qnXYZ-000ABC-12");
}

#[test]
fn an_arrival_line_colors_its_flag_as_a_success() {
    assert_eq!(field(ARRIVAL, TokenKind::Success), "<=");
}

#[test]
fn an_arrival_line_colors_the_sender_as_an_email() {
    assert_eq!(field(ARRIVAL, TokenKind::Email), "alice@example.com");
}

#[test]
fn an_arrival_line_colors_the_sending_host_and_its_address() {
    assert_eq!(field(ARRIVAL, TokenKind::Host), "mail.example.com");
    assert_eq!(field(ARRIVAL, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn an_arrival_line_colors_the_protocol_and_the_size() {
    assert_eq!(field(ARRIVAL, TokenKind::Protocol), "esmtps");
    assert_eq!(field(ARRIVAL, TokenKind::Size), "4512");
}

#[test]
fn an_arrival_line_colors_the_message_header_id() {
    assert_eq!(
        field(ARRIVAL, TokenKind::Transaction),
        "20231003120001.abc@example.com"
    );
}

#[test]
fn a_delivery_line_colors_the_router_and_the_transport() {
    let modules: Vec<String> = exim::parse_line(DELIVERY)
        .unwrap()
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Module)
        .map(|token| token.text.to_string())
        .collect();

    assert_eq!(modules, vec!["dnslookup", "remote_smtp"]);
}

#[test]
fn a_delivery_line_colors_the_quoted_confirmation_as_message_text() {
    assert!(exim::parse_line(DELIVERY)
        .unwrap()
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Message && token.text == "250 2.0.0 OK"));
}

#[test]
fn a_delivery_line_keeps_every_character_of_the_original() {
    assert_eq!(exim::parse_line(DELIVERY).unwrap().text(), DELIVERY);
}

#[test]
fn a_deferral_is_colored_as_a_warning() {
    let line = DELIVERY.replace(" => ", " == ");

    assert_eq!(field(&line, TokenKind::Warning), "==");
}

#[test]
fn a_failed_delivery_is_colored_as_a_failure() {
    let line = DELIVERY.replace(" => ", " ** ");

    assert_eq!(field(&line, TokenKind::Failure), "**");
}

#[test]
fn a_completed_message_is_colored_as_a_success() {
    assert_eq!(field(COMPLETED, TokenKind::Success), "Completed");
}

#[test]
fn a_message_id_with_nothing_after_it_is_read() {
    let line = "2023-10-03 12:00:02 1qnXYZ-000ABC-12";

    assert_eq!(exim::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_message_id_in_the_longer_form_exim_writes_since_4_97_is_read() {
    let line = COMPLETED.replace("1qnXYZ-000ABC-12", "1qnXYZ-000000ABC-1234");

    assert_eq!(field(&line, TokenKind::QueueId), "1qnXYZ-000000ABC-1234");
}

#[test]
fn a_line_with_milliseconds_and_a_time_zone_colors_the_full_timestamp() {
    assert_eq!(
        field(CONNECTION, TokenKind::Timestamp),
        "2023-10-03 12:05:12.345 +0000"
    );
}

#[test]
fn a_line_with_a_process_id_colors_it() {
    assert_eq!(field(CONNECTION, TokenKind::Pid), "4321");
}

#[test]
fn a_line_about_no_message_has_no_message_id() {
    assert!(!kinds(CONNECTION).contains(&TokenKind::QueueId));
}

#[test]
fn a_line_about_no_message_keeps_every_character_of_the_original() {
    assert_eq!(exim::parse_line(CONNECTION).unwrap().text(), CONNECTION);
}

#[test]
fn a_line_sent_to_syslog_is_read() {
    assert_eq!(exim::parse_line(SYSLOG).unwrap().text(), SYSLOG);
}

#[test]
fn a_line_sent_to_syslog_colors_the_message_id() {
    assert_eq!(field(SYSLOG, TokenKind::QueueId), "1qnXYZ-000ABC-12");
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert!(exim::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_an_exim_log_is_not_parsed() {
    assert!(exim::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn a_timestamp_is_split_off_a_main_log_line() {
    let (tokens, body) = exim::split_timestamp(COMPLETED).unwrap();

    assert_eq!(tokens[0].text, "2023-10-03 12:00:02");
    assert_eq!(body, "1qnXYZ-000ABC-12 Completed");
}

#[test]
fn arrivals_and_deliveries_are_successes() {
    for flag in ["<=", "=>", "->"] {
        assert_eq!(exim::flag_kind(flag), TokenKind::Success, "{}", flag);
    }
}

#[test]
fn suppressed_and_deferred_deliveries_are_warnings() {
    for flag in ["*>", "=="] {
        assert_eq!(exim::flag_kind(flag), TokenKind::Warning, "{}", flag);
    }
}

#[test]
fn a_failed_delivery_flag_is_a_failure() {
    assert_eq!(exim::flag_kind("**"), TokenKind::Failure);
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(EximPlugin::new().name(), "exim");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(EximPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        EximPlugin::default().metadata().description,
        "Exim mail routing and delivery logs"
    );
}

#[test]
fn the_plugin_parses_an_arrival_line_into_tokens() {
    let parsed = match EximPlugin::new().parse_line(ARRIVAL) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the arrival line should parse"),
    };

    assert_eq!(parsed.text(), ARRIVAL);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        EximPlugin::new().parse_line("not an exim log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        EximPlugin::new().detect_format(&[ARRIVAL, DELIVERY, COMPLETED, CONNECTION, SYSLOG]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(EximPlugin::new().detect_format(&["hello", "world"]), 0.0);
}
