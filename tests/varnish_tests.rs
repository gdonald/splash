use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::varnish::{self, VarnishPlugin};

const TRANSACTION: &str = "*   << Request  >> 32770";
const BACKEND_TRANSACTION: &str = "**  << BeReq    >> 32771";
const METHOD_RECORD: &str = "-   ReqMethod      GET";
const HEADER_RECORD: &str = "-   ReqHeader      Host: example.com";
const RAW_RECORD: &str = "        32770 Timestamp      c Start: 1614729600.123456 0.000000";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    varnish::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    varnish::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_transaction_header_keeps_every_character_of_the_original() {
    assert_eq!(
        varnish::parse_line(TRANSACTION).unwrap().text(),
        TRANSACTION
    );
}

#[test]
fn a_transaction_header_colors_the_kind_of_transaction_it_opens() {
    assert_eq!(field(TRANSACTION, TokenKind::Transaction), "Request");
}

#[test]
fn a_transaction_header_colors_the_transaction_id() {
    assert_eq!(field(TRANSACTION, TokenKind::Number), "32770");
}

#[test]
fn a_nested_transaction_header_keeps_the_stars_that_show_its_depth() {
    assert_eq!(
        varnish::parse_line(BACKEND_TRANSACTION).unwrap().tokens[0].text,
        "**"
    );
}

#[test]
fn a_transaction_header_with_no_padding_inside_its_brackets_is_read() {
    let line = "*   <<Request>> 1";

    assert_eq!(varnish::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_record_keeps_every_character_of_the_original() {
    assert_eq!(
        varnish::parse_line(METHOD_RECORD).unwrap().text(),
        METHOD_RECORD
    );
}

#[test]
fn a_record_colors_the_tag_that_introduces_it() {
    assert_eq!(field(METHOD_RECORD, TokenKind::Tag), "ReqMethod");
}

#[test]
fn a_request_method_record_colors_its_payload_as_a_method() {
    assert_eq!(field(METHOD_RECORD, TokenKind::Method), "GET");
}

#[test]
fn a_backend_request_method_record_colors_its_payload_as_a_method() {
    assert_eq!(field("--  BereqMethod    GET", TokenKind::Method), "GET");
}

#[test]
fn a_url_record_colors_its_payload_as_a_request() {
    assert_eq!(
        field("-   ReqURL         /index.html", TokenKind::Request),
        "/index.html"
    );
}

#[test]
fn a_protocol_record_colors_its_payload_as_a_protocol() {
    assert_eq!(
        field("-   ReqProtocol    HTTP/1.1", TokenKind::Protocol),
        "HTTP/1.1"
    );
}

#[test]
fn a_response_status_record_colors_its_payload_as_a_status() {
    assert_eq!(field("-   RespStatus     200", TokenKind::Status), "200");
}

#[test]
fn a_timestamp_record_colors_its_payload_as_a_timestamp() {
    let line = "-   Timestamp      Start: 1614729600.123456 0.000000";

    assert_eq!(
        field(line, TokenKind::Timestamp),
        "Start: 1614729600.123456 0.000000"
    );
}

#[test]
fn a_header_record_colors_the_header_name_apart_from_its_value() {
    assert_eq!(field(HEADER_RECORD, TokenKind::Header), "Host");
}

#[test]
fn a_header_record_keeps_every_character_of_the_original() {
    assert_eq!(
        varnish::parse_line(HEADER_RECORD).unwrap().text(),
        HEADER_RECORD
    );
}

#[test]
fn a_header_record_with_no_colon_is_left_as_message_text() {
    let line = "-   ReqHeader      malformed";

    assert_eq!(field(line, TokenKind::Message), "malformed");
}

#[test]
fn a_record_with_a_tag_splash_has_no_field_for_is_left_as_message_text() {
    assert_eq!(
        field("-   Begin          req 32769 rxreq", TokenKind::Message),
        "req 32769 rxreq"
    );
}

#[test]
fn a_record_colors_an_address_inside_its_payload() {
    assert_eq!(
        field("-   ReqStart       10.0.0.5 54321 a0", TokenKind::Ip),
        "10.0.0.5"
    );
}

#[test]
fn a_record_with_no_payload_ends_at_its_tag() {
    assert_eq!(kinds("-   End").last(), Some(&TokenKind::Tag));
}

#[test]
fn a_record_with_no_payload_keeps_every_character_of_the_original() {
    assert_eq!(varnish::parse_line("-   End").unwrap().text(), "-   End");
}

#[test]
fn a_raw_record_keeps_every_character_of_the_original() {
    assert_eq!(varnish::parse_line(RAW_RECORD).unwrap().text(), RAW_RECORD);
}

#[test]
fn a_raw_record_colors_the_transaction_id_that_leads_it() {
    assert_eq!(field(RAW_RECORD, TokenKind::Number), "32770");
}

#[test]
fn a_raw_record_colors_the_side_the_record_came_from() {
    assert_eq!(field(RAW_RECORD, TokenKind::Transaction), "c");
}

#[test]
fn a_raw_record_colors_its_payload_by_its_tag() {
    assert_eq!(
        field(RAW_RECORD, TokenKind::Timestamp),
        "Start: 1614729600.123456 0.000000"
    );
}

#[test]
fn a_raw_record_with_no_leading_padding_is_read() {
    let line = "32770 ReqMethod c GET";

    assert_eq!(field(line, TokenKind::Method), "GET");
}

#[test]
fn a_line_that_is_not_a_varnish_log_is_not_parsed() {
    assert!(varnish::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(VarnishPlugin::new().name(), "varnish");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(VarnishPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        VarnishPlugin::default().metadata().description,
        "Varnish request and response logs"
    );
}

#[test]
fn the_plugin_parses_a_record_into_tokens() {
    let parsed = match VarnishPlugin::new().parse_line(METHOD_RECORD) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the record should parse"),
    };

    assert_eq!(parsed.text(), METHOD_RECORD);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        VarnishPlugin::new().parse_line("not a varnish log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        VarnishPlugin::new().detect_format(&[TRANSACTION, METHOD_RECORD, RAW_RECORD]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(VarnishPlugin::new().detect_format(&["hello", "world"]), 0.0);
}

#[test]
fn a_raw_record_with_no_payload_keeps_every_character_of_the_original() {
    let line = "        32770 End            c";

    assert_eq!(varnish::parse_line(line).unwrap().text(), line);
}

#[test]
fn a_raw_record_with_no_payload_ends_at_the_side_it_came_from() {
    let line = "        32770 End            c";

    assert_eq!(kinds(line).last(), Some(&TokenKind::Transaction));
}
