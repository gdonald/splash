use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::syslog::{self, SyslogPlugin};

const TRADITIONAL: &str =
    "Oct  3 12:00:01 web01 nginx[2200]: worker process 2201 exited on signal 9";

const WIRE: &str = "<34>Oct 11 22:14:15 mymachine su: 'su root' failed for alice on /dev/pts/8";

const RFC_5424: &str = concat!(
    r#"<165>1 2003-10-11T22:14:15.003Z mymachine.example.com evntslog - ID47 "#,
    r#"[exampleSDID@32473 iut="3" eventSource="Application"] An application event log entry"#
);

const NO_STRUCTURED_DATA: &str =
    "<165>1 2003-08-24T05:14:15.000003-07:00 192.0.2.1 myproc 8710 - - %% It's time to make the do-nuts.";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    syslog::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    syslog::parse_line(line)
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

/// The text of the timestamp a header was split into
fn header_timestamp(line: &str) -> &str {
    syslog::split_header(line).unwrap().tokens[0].text
}

#[test]
fn a_traditional_syslog_header_is_split_off_the_message() {
    let header = syslog::split_header("Oct  3 12:00:01 mail postfix/smtpd[1234]: connect").unwrap();

    assert_eq!(header.body, "connect");
}

#[test]
fn a_syslog_header_names_the_program_that_logged_the_line() {
    let header = syslog::split_header("Oct  3 12:00:01 mail postfix/smtpd[1234]: connect").unwrap();

    assert_eq!(header.program, "postfix/smtpd");
}

#[test]
fn a_syslog_header_colors_the_timestamp_host_program_and_process_id() {
    let header = syslog::split_header("Oct  3 12:00:01 mail postfix/smtpd[1234]: connect").unwrap();
    let colored: Vec<(&str, TokenKind)> = header
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Plain && token.kind != TokenKind::Punctuation)
        .map(|token| (token.text, token.kind))
        .collect();

    assert_eq!(
        colored,
        vec![
            ("Oct  3 12:00:01", TokenKind::Timestamp),
            ("mail", TokenKind::Host),
            ("postfix/smtpd", TokenKind::Tag),
            ("1234", TokenKind::Pid),
        ]
    );
}

#[test]
fn a_syslog_header_without_a_process_id_is_split_off_the_message() {
    let header = syslog::split_header("Oct  3 12:00:01 mail dovecot: imap-login: Login").unwrap();

    assert_eq!(header.body, "imap-login: Login");
}

#[test]
fn a_syslog_header_without_a_process_id_has_no_process_id_token() {
    let header = syslog::split_header("Oct  3 12:00:01 mail dovecot: imap-login: Login").unwrap();

    assert!(!header
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Pid));
}

#[test]
fn a_syslog_header_with_an_rfc_3339_timestamp_is_split_off_the_message() {
    let header =
        syslog::split_header("2023-10-03T12:00:01.123456+00:00 mail postfix/qmgr[900]: removed")
            .unwrap();

    assert_eq!(header.tokens[0].text, "2023-10-03T12:00:01.123456+00:00");
}

#[test]
fn a_line_without_a_syslog_header_is_not_split() {
    assert!(syslog::split_header("the maintenance window moves to 02:00").is_none());
}

#[test]
fn a_ctime_date_pattern_matches_a_date_with_a_padded_day() {
    let pattern = regex::Regex::new(&format!("^{}$", syslog::CTIME)).unwrap();

    assert!(pattern.is_match("Tue Oct  3 12:00:01 2023"));
}

#[test]
fn a_header_with_microseconds_is_split_off_the_message() {
    assert_eq!(
        header_timestamp("Oct 03 12:00:03.654321 web01 systemd[1]: Stopped cron.service."),
        "Oct 03 12:00:03.654321"
    );
}

#[test]
fn a_header_with_an_offset_without_a_colon_is_split_off_the_message() {
    assert_eq!(
        header_timestamp("2023-10-03T12:00:01+0000 web01 sshd[4100]: Server listening"),
        "2023-10-03T12:00:01+0000"
    );
}

#[test]
fn a_header_with_a_full_date_and_zone_is_split_off_the_message() {
    assert_eq!(
        header_timestamp("Tue 2023-10-03 12:00:04 UTC web01 systemd[1]: Listening on dbus.socket."),
        "Tue 2023-10-03 12:00:04 UTC"
    );
}

#[test]
fn a_header_with_a_monotonic_timestamp_is_split_off_the_message() {
    assert_eq!(
        header_timestamp("[    5.123456] web01 systemd[1]: Finished systemd-sysctl.service."),
        "[    5.123456]"
    );
}

#[test]
fn a_header_with_a_unix_timestamp_is_split_off_the_message() {
    assert_eq!(
        header_timestamp("1696334406.000123 web01 systemd[1]: Started session-12.scope."),
        "1696334406.000123"
    );
}

#[test]
fn a_bare_header_splits_the_timestamp_and_host_off_the_rest() {
    let (tokens, rest) =
        syslog::split_bare_header("Oct  3 12:00:05 web01 last message repeated 3 times").unwrap();

    assert_eq!(tokens[2], Token::new("web01", TokenKind::Host));
    assert_eq!(rest, "last message repeated 3 times");
}

#[test]
fn a_line_without_a_timestamp_has_no_bare_header() {
    assert!(syslog::split_bare_header("the maintenance window moves to 02:00").is_none());
}

#[test]
fn severities_from_emergency_to_error_are_failures() {
    assert_eq!(syslog::severity_kind(0), TokenKind::Failure);
    assert_eq!(syslog::severity_kind(3), TokenKind::Failure);
}

#[test]
fn the_warning_severity_is_a_warning() {
    assert_eq!(syslog::severity_kind(4), TokenKind::Warning);
}

#[test]
fn severities_from_notice_to_debug_are_levels() {
    assert_eq!(syslog::severity_kind(5), TokenKind::Level);
    assert_eq!(syslog::severity_kind(7), TokenKind::Level);
}

#[test]
fn a_priority_is_colored_by_the_severity_it_holds() {
    let (tokens, rest) = syslog::split_priority("<12>message").unwrap();

    assert_eq!(
        tokens,
        vec![
            Token::new("<", TokenKind::Punctuation),
            Token::new("12", TokenKind::Warning),
            Token::new(">", TokenKind::Punctuation),
        ]
    );
    assert_eq!(rest, "message");
}

#[test]
fn a_line_without_a_priority_has_none_split_off() {
    assert!(syslog::split_priority("Oct  3 12:00:01").is_none());
}

#[test]
fn free_text_colors_words_that_report_a_problem() {
    let mut tokens = Vec::new();
    syslog::push_text(&mut tokens, "disk error, retry warning");

    assert_eq!(
        tokens,
        vec![
            Token::new("disk ", TokenKind::Message),
            Token::new("error", TokenKind::Failure),
            Token::new(", retry ", TokenKind::Message),
            Token::new("warning", TokenKind::Warning),
        ]
    );
}

#[test]
fn a_traditional_line_keeps_every_character_of_the_original() {
    assert_eq!(syslog::parse_line(TRADITIONAL).unwrap().text(), TRADITIONAL);
}

#[test]
fn a_traditional_line_colors_its_header() {
    assert_eq!(field(TRADITIONAL, TokenKind::Tag), "nginx");
    assert_eq!(field(TRADITIONAL, TokenKind::Pid), "2200");
}

#[test]
fn a_line_without_a_program_colors_its_timestamp_and_host() {
    let line = "Oct  3 12:00:05 web01 last message repeated 3 times";

    assert_eq!(syslog::parse_line(line).unwrap().text(), line);
    assert_eq!(field(line, TokenKind::Host), "web01");
    assert!(!kinds(line).contains(&TokenKind::Tag));
}

#[test]
fn a_line_read_off_the_wire_keeps_every_character_of_the_original() {
    assert_eq!(syslog::parse_line(WIRE).unwrap().text(), WIRE);
}

#[test]
fn a_line_read_off_the_wire_colors_its_priority_by_severity() {
    assert_eq!(field(WIRE, TokenKind::Failure), "34");
}

#[test]
fn a_message_colors_a_failure() {
    assert_eq!(all(WIRE, TokenKind::Failure), vec!["34", "failed"]);
}

#[test]
fn an_rfc_5424_line_keeps_every_character_of_the_original() {
    for line in [RFC_5424, NO_STRUCTURED_DATA] {
        assert_eq!(syslog::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_rfc_5424_line_colors_its_header_fields() {
    assert_eq!(field(RFC_5424, TokenKind::Number), "1");
    assert_eq!(
        field(RFC_5424, TokenKind::Timestamp),
        "2003-10-11T22:14:15.003Z"
    );
    assert_eq!(field(RFC_5424, TokenKind::Host), "mymachine.example.com");
    assert_eq!(field(RFC_5424, TokenKind::Tag), "evntslog");
    assert_eq!(field(RFC_5424, TokenKind::Transaction), "ID47");
    assert_eq!(field(NO_STRUCTURED_DATA, TokenKind::Pid), "8710");
}

#[test]
fn an_rfc_5424_field_left_out_is_a_dash() {
    assert!(!kinds(RFC_5424).contains(&TokenKind::Pid));
    assert!(!kinds(NO_STRUCTURED_DATA).contains(&TokenKind::Module));
}

#[test]
fn structured_data_colors_its_id_and_parameter_names() {
    assert_eq!(field(RFC_5424, TokenKind::Module), "exampleSDID@32473");
    assert_eq!(all(RFC_5424, TokenKind::Header), vec!["iut", "eventSource"]);
}

#[test]
fn structured_data_colors_each_parameter_value() {
    assert_eq!(
        all(RFC_5424, TokenKind::Message),
        vec!["3", "Application", " An application event log entry"]
    );
}

#[test]
fn several_structured_data_elements_and_an_empty_value_are_read() {
    let line = r#"<13>1 2023-10-03T12:00:07Z web01 app 3300 - [meta@32473 note=""][origin@32473 ip="10.0.0.5"] queue is full"#;

    assert_eq!(syslog::parse_line(line).unwrap().text(), line);
    assert_eq!(
        all(line, TokenKind::Module),
        vec!["meta@32473", "origin@32473"]
    );
}

#[test]
fn a_version_after_a_priority_is_read_only_as_rfc_5424() {
    let line = "1 2023-10-03T12:00:07Z web01 app 3300 - - queue is full";

    assert!(syslog::parse_line(line).is_none());
}

#[test]
fn a_priority_before_text_that_has_no_header_is_not_parsed() {
    assert!(syslog::parse_line("<13>the maintenance window moves to 02:00").is_none());
}

#[test]
fn a_line_that_is_not_a_syslog_line_is_not_parsed() {
    assert!(syslog::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(SyslogPlugin::new().name(), "syslog");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(SyslogPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        SyslogPlugin::default().metadata().description,
        "Syslog lines from any program, including RFC 5424"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match SyslogPlugin::new().parse_line(TRADITIONAL) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), TRADITIONAL);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        SyslogPlugin::new().parse_line("not a syslog line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        SyslogPlugin::new().detect_format(&[TRADITIONAL, WIRE, RFC_5424]),
        1.0
    );
}
