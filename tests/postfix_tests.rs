use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::postfix::{self, PostfixPlugin};

const DELIVERY: &str = concat!(
    "Oct  3 12:00:02 mail postfix/smtp[1236]: 4F2A1C0123: to=<bob@example.org>, ",
    "relay=mx.example.org[93.184.216.34]:25, delay=0.52, delays=0.1/0/0.2/0.22, ",
    "dsn=2.0.0, status=sent (250 2.0.0 OK)"
);

const REJECT: &str = concat!(
    "Oct  3 12:05:12 mail postfix/smtpd[1243]: NOQUEUE: reject: RCPT from unknown[192.0.2.10]: ",
    "554 5.7.1 <erin@example.org>: Relay access denied; from=<spam@example.biz> ",
    "to=<erin@example.org> proto=ESMTP helo=<example.biz>"
);

const CONNECT: &str = "Oct  3 12:00:01 mail postfix/smtpd[1234]: connect from unknown[10.0.0.5]";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    postfix::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    postfix::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_delivery_line_keeps_every_character_of_the_original() {
    assert_eq!(postfix::parse_line(DELIVERY).unwrap().text(), DELIVERY);
}

#[test]
fn a_delivery_line_colors_the_daemon_that_logged_it() {
    assert_eq!(field(DELIVERY, TokenKind::Tag), "postfix/smtp");
}

#[test]
fn a_delivery_line_colors_the_queue_id() {
    assert_eq!(field(DELIVERY, TokenKind::QueueId), "4F2A1C0123");
}

#[test]
fn a_delivery_line_colors_the_recipient_as_an_email() {
    assert_eq!(field(DELIVERY, TokenKind::Email), "bob@example.org");
}

#[test]
fn a_delivery_line_colors_the_relay_and_its_address() {
    let hosts: Vec<&str> = postfix::parse_line(DELIVERY)
        .unwrap()
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Host)
        .map(|token| token.text)
        .collect();

    assert_eq!(hosts, vec!["mail", "mx.example.org"]);
    assert_eq!(field(DELIVERY, TokenKind::Ip), "93.184.216.34");
}

#[test]
fn a_delivery_line_colors_the_delay_and_its_breakdown() {
    assert_eq!(field(DELIVERY, TokenKind::Duration), "0.52");
    assert_eq!(field(DELIVERY, TokenKind::Timers), "0.1/0/0.2/0.22");
}

#[test]
fn a_delivery_line_colors_the_delivery_status_notification_code() {
    assert_eq!(field(DELIVERY, TokenKind::Status), "2.0.0");
}

#[test]
fn a_sent_mail_is_colored_as_a_success() {
    assert_eq!(field(DELIVERY, TokenKind::Success), "sent");
}

#[test]
fn a_deferred_mail_is_colored_as_a_warning() {
    let line = DELIVERY.replace("status=sent", "status=deferred");

    assert_eq!(field(&line, TokenKind::Warning), "deferred");
}

#[test]
fn a_bounced_mail_is_colored_as_a_failure() {
    let line = DELIVERY.replace("status=sent", "status=bounced");

    assert_eq!(field(&line, TokenKind::Failure), "bounced");
}

#[test]
fn a_long_queue_id_is_colored_as_a_queue_id() {
    let line = DELIVERY.replace("4F2A1C0123", "4Wq2Yd1Qm7z4xKV");

    assert_eq!(field(&line, TokenKind::QueueId), "4Wq2Yd1Qm7z4xKV");
}

#[test]
fn a_rejected_mail_colors_noqueue_as_its_queue_id() {
    assert_eq!(field(REJECT, TokenKind::QueueId), "NOQUEUE");
}

#[test]
fn a_rejected_mail_colors_the_rejection_as_a_failure() {
    assert_eq!(field(REJECT, TokenKind::Failure), "reject");
}

#[test]
fn a_rejected_mail_colors_the_reason_as_a_failure() {
    let failures: Vec<String> = postfix::parse_line(REJECT)
        .unwrap()
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Failure)
        .map(|token| token.text.to_string())
        .collect();

    assert_eq!(failures, vec!["reject", "Relay access denied"]);
}

#[test]
fn a_rejected_mail_keeps_every_character_of_the_original() {
    assert_eq!(postfix::parse_line(REJECT).unwrap().text(), REJECT);
}

#[test]
fn a_rejected_mail_colors_the_protocol_the_client_spoke() {
    assert_eq!(field(REJECT, TokenKind::Protocol), "ESMTP");
}

#[test]
fn a_warning_is_colored_as_a_warning() {
    let line =
        "Oct  3 12:05:13 mail postfix/smtpd[1243]: warning: hostname example.biz does not resolve";

    assert_eq!(field(line, TokenKind::Warning), "warning");
}

#[test]
fn a_line_with_no_queue_id_has_no_queue_id_token() {
    assert!(!kinds(CONNECT).contains(&TokenKind::QueueId));
}

#[test]
fn a_line_with_no_queue_id_colors_the_client_address() {
    assert_eq!(field(CONNECT, TokenKind::Ip), "10.0.0.5");
}

#[test]
fn a_removed_mail_is_colored_as_a_success() {
    let line = "Oct  3 12:00:02 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert_eq!(field(line, TokenKind::Success), "removed");
}

#[test]
fn a_line_with_an_rfc_3339_timestamp_is_read() {
    let line = "2023-10-03T12:00:01.451223+00:00 mail postfix/qmgr[900]: 4F2A1C0123: removed";

    assert_eq!(
        field(line, TokenKind::Timestamp),
        "2023-10-03T12:00:01.451223+00:00"
    );
}

#[test]
fn a_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 mail dovecot: imap-login: Login: user=<alice>";

    assert!(postfix::parse_line(line).is_none());
}

#[test]
fn a_line_without_a_syslog_header_is_not_parsed() {
    assert!(postfix::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn a_warning_and_a_held_mail_are_warnings() {
    assert_eq!(postfix::level_kind("warning"), TokenKind::Warning);
    assert_eq!(postfix::level_kind("hold"), TokenKind::Warning);
}

#[test]
fn every_other_leading_action_is_a_failure() {
    for level in [
        "error",
        "fatal",
        "panic",
        "reject",
        "discard",
        "milter-reject",
    ] {
        assert_eq!(postfix::level_kind(level), TokenKind::Failure, "{}", level);
    }
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(PostfixPlugin::new().name(), "postfix");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(PostfixPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        PostfixPlugin::default().metadata().description,
        "Postfix mail queue, SMTP, and delivery logs"
    );
}

#[test]
fn the_plugin_parses_a_delivery_line_into_tokens() {
    let parsed = match PostfixPlugin::new().parse_line(DELIVERY) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the delivery line should parse"),
    };

    assert_eq!(parsed.text(), DELIVERY);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        PostfixPlugin::new().parse_line("not a postfix log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        PostfixPlugin::new().detect_format(&[DELIVERY, REJECT, CONNECT]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(PostfixPlugin::new().detect_format(&["hello", "world"]), 0.0);
}
