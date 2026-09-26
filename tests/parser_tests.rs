use splash::output::TokenKind;
use splash::parser::{
    cached_pattern, cached_pattern_count, parse_adhoc_line, parse_clf_line, parse_line,
};
use std::sync::Arc;

fn kinds(line: &splash::output::ParsedLine<'_>) -> Vec<TokenKind> {
    line.tokens.iter().map(|token| token.kind).collect()
}

fn texts(line: &splash::output::ParsedLine<'_>) -> Vec<String> {
    line.tokens
        .iter()
        .map(|token| token.text.to_string())
        .collect()
}

#[test]
fn adhoc_marks_ip_addresses() {
    let parsed = parse_adhoc_line("Connection from 192.168.1.100");

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Ip && token.text == "192.168.1.100"));
}

#[test]
fn adhoc_marks_bare_numbers() {
    let parsed = parse_adhoc_line("status 404");

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Number && token.text == "404"));
}

#[test]
fn adhoc_marks_timezone_offsets() {
    let parsed = parse_adhoc_line("offset -0700");

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::TimezoneOffset && token.text == "-0700"));
}

#[test]
fn adhoc_marks_datetimes() {
    let parsed = parse_adhoc_line("[10/Oct/2000:13:55:36 -0700]");

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::DateTime && token.text == "10/Oct/2000:13:55:36"));
}

#[test]
fn adhoc_marks_http_versions() {
    let parsed = parse_adhoc_line("Request: HTTP/1.0");

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::HttpVersion && token.text == "HTTP/1.0"));
}

#[test]
fn adhoc_marks_a_bare_http_verb() {
    let parsed = parse_adhoc_line("GET /index.html");

    assert_eq!(parsed.tokens[0].kind, TokenKind::HttpVerb);
    assert_eq!(parsed.tokens[0].text, "GET");
}

#[test]
fn adhoc_splits_text_surrounding_an_http_verb() {
    let parsed = parse_adhoc_line("method=POST;");

    assert_eq!(
        texts(&parsed),
        vec!["method=".to_string(), "POST".to_string(), ";".to_string()]
    );
    assert_eq!(
        kinds(&parsed),
        vec![TokenKind::Plain, TokenKind::HttpVerb, TokenKind::Plain]
    );
}

#[test]
fn adhoc_marks_quotes_and_brackets_as_punctuation() {
    let parsed = parse_adhoc_line("[INFO] \"hello\"");

    let punctuation: Vec<String> = parsed
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Punctuation)
        .map(|token| token.text.to_string())
        .collect();

    assert_eq!(punctuation, vec!["[", "]", "\"", "\""]);
}

#[test]
fn adhoc_collapses_runs_of_whitespace_to_one_space() {
    let parsed = parse_adhoc_line("  alpha \t beta  ");

    assert_eq!(parsed.text(), "alpha beta");
}

#[test]
fn adhoc_leaves_unrecognized_words_plain() {
    let parsed = parse_adhoc_line("hello");

    assert_eq!(kinds(&parsed), vec![TokenKind::Plain]);
}

#[test]
fn clf_parses_every_field_of_a_common_log_format_line() {
    let line =
        r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#;
    let parsed = parse_clf_line(line).expect("line should parse as CLF");

    let fields: Vec<(TokenKind, String)> = parsed
        .tokens
        .iter()
        .filter(|token| token.kind != TokenKind::Plain && token.kind != TokenKind::Punctuation)
        .map(|token| (token.kind, token.text.to_string()))
        .collect();

    assert_eq!(
        fields,
        vec![
            (TokenKind::Client, "127.0.0.1".to_string()),
            (TokenKind::UserIdentifier, "-".to_string()),
            (TokenKind::UserId, "frank".to_string()),
            (
                TokenKind::Timestamp,
                "[10/Oct/2000:13:55:36 -0700]".to_string()
            ),
            (TokenKind::Method, "GET".to_string()),
            (TokenKind::Request, "/apache_pb.gif".to_string()),
            (TokenKind::Protocol, "HTTP/1.0".to_string()),
            (TokenKind::Status, "200".to_string()),
            (TokenKind::Size, "2326".to_string()),
        ]
    );
}

#[test]
fn clf_rebuilds_the_original_line_text() {
    let line = r#"10.0.0.5 - - [12/Dec/2001:01:02:03 +0000] "POST /submit HTTP/1.1" 302 -"#;
    let parsed = parse_clf_line(line).expect("line should parse as CLF");

    assert_eq!(parsed.text(), line);
}

#[test]
fn clf_rejects_a_line_that_is_not_common_log_format() {
    assert!(parse_clf_line("this is not a CLF line").is_none());
}

#[test]
fn parse_line_skips_empty_lines() {
    assert!(parse_line("", "ad-hoc").is_none());
}

#[test]
fn parse_line_uses_clf_parsing_for_clf_mode() {
    let line =
        r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#;
    let parsed = parse_line(line, "clf").expect("line should parse as CLF");

    assert_eq!(parsed.tokens[0].kind, TokenKind::Client);
}

#[test]
fn parse_line_falls_back_to_adhoc_for_an_unknown_mode() {
    let parsed = parse_line("192.168.1.1", "not-a-mode").expect("ad-hoc always parses");

    assert_eq!(parsed.tokens[0].kind, TokenKind::Ip);
}

fn borrows_from(line: &str, text: &str) -> bool {
    let start = line.as_ptr() as usize;
    let token = text.as_ptr() as usize;

    token >= start && token + text.len() <= start + line.len()
}

#[test]
fn adhoc_tokens_borrow_the_text_of_the_line_they_came_from() {
    let line = String::from("[INFO] 192.168.1.1 GET /index.html 200");
    let parsed = parse_adhoc_line(&line);

    for token in &parsed.tokens {
        if token.text == " " {
            continue;
        }

        assert!(borrows_from(&line, token.text), "copied {:?}", token.text);
    }
}

#[test]
fn clf_tokens_borrow_the_text_of_the_line_they_came_from() {
    let line = String::from(
        r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#,
    );
    let parsed = parse_clf_line(&line).expect("line should parse as CLF");

    for token in &parsed.tokens {
        if token.text == " " || token.text == "\"" {
            continue;
        }

        assert!(borrows_from(&line, token.text), "copied {:?}", token.text);
    }
}

#[test]
fn a_pattern_is_compiled_once_and_reused() {
    let first = cached_pattern(r"^first-pattern-\d+$").unwrap();
    let second = cached_pattern(r"^first-pattern-\d+$").unwrap();

    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn a_cached_pattern_still_matches() {
    let pattern = cached_pattern(r"^worker-\d+$").unwrap();

    assert!(pattern.is_match("worker-12"));
    assert!(!pattern.is_match("worker"));
}

#[test]
fn different_patterns_are_compiled_separately() {
    let first = cached_pattern("^alpha$").unwrap();
    let second = cached_pattern("^beta$").unwrap();

    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn caching_a_pattern_counts_it() {
    cached_pattern("^counted-pattern$").unwrap();

    assert!(cached_pattern_count() >= 1);
}

#[test]
fn an_invalid_pattern_is_rejected() {
    assert!(cached_pattern("(unclosed").is_err());
}

#[test]
fn the_httpd_mode_parses_a_combined_access_log_line() {
    let line = concat!(
        r#"127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET / HTTP/1.1" 200 12 "-" "#,
        r#""curl/8.4.0""#
    );
    let parsed = parse_line(line, "httpd").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::UserAgent));
}

#[test]
fn the_httpd_mode_drops_a_line_that_is_not_a_web_server_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "httpd").is_none());
}

#[test]
fn a_mode_no_plugin_claims_falls_back_to_the_general_patterns() {
    let parsed = parse_line("10.0.0.9 started", "not-a-plugin").unwrap();

    assert_eq!(parsed.tokens[0].kind, TokenKind::Ip);
}

#[test]
fn the_squid_mode_parses_a_native_access_log_line() {
    let line = concat!(
        "1614729600.123    123 10.0.0.5 TCP_MISS/200 4512 GET http://example.com/ - ",
        "HIER_DIRECT/93.184.216.34 text/html"
    );
    let parsed = parse_line(line, "squid").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::CacheMiss));
}

#[test]
fn the_squid_mode_drops_a_line_that_is_not_an_access_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "squid").is_none());
}

#[test]
fn the_varnish_mode_parses_a_transaction_record() {
    let parsed = parse_line("-   ReqMethod      GET", "varnish").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Tag));
}

#[test]
fn the_varnish_mode_drops_a_line_that_is_not_a_varnish_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "varnish").is_none());
}

#[test]
fn the_haproxy_mode_parses_a_connection_line() {
    let line = concat!(
        "10.0.1.2:33313 [06/Feb/2009:12:12:51.443] fnt bck/srv1 0/0/5007 212 -- ",
        "0/0/0/0/3 0/0"
    );
    let parsed = parse_line(line, "haproxy").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Termination));
}

#[test]
fn the_haproxy_mode_drops_a_line_that_is_not_a_haproxy_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "haproxy").is_none());
}

#[test]
fn the_caddy_mode_parses_a_structured_log_line() {
    let line = r#"{"level":"info","status":200}"#;
    let parsed = parse_line(line, "caddy").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Status));
}

#[test]
fn the_caddy_mode_drops_a_line_that_is_not_json() {
    assert!(parse_line("the maintenance window moves to 02:00", "caddy").is_none());
}

#[test]
fn the_postfix_mode_parses_a_delivery_line() {
    let line =
        "Oct  3 12:00:02 mail postfix/smtp[1236]: 4F2A1C0123: to=<bob@example.org>, status=sent";
    let parsed = parse_line(line, "postfix").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::QueueId));
}

#[test]
fn the_postfix_mode_drops_a_line_that_is_not_a_postfix_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "postfix").is_none());
}

#[test]
fn the_exim_mode_parses_a_delivery_line() {
    let line = "2023-10-03 12:00:02 1qnXYZ-000ABC-12 => bob@example.org R=dnslookup";
    let parsed = parse_line(line, "exim").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_exim_mode_drops_a_line_that_is_not_an_exim_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "exim").is_none());
}

#[test]
fn the_fetchmail_mode_parses_a_progress_line() {
    let line = "reading message alice@mail.example.com:1 of 3 (4096 octets) flushed";
    let parsed = parse_line(line, "fetchmail").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Size));
}

#[test]
fn the_fetchmail_mode_drops_a_line_that_is_not_a_fetchmail_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "fetchmail").is_none());
}

#[test]
fn the_dovecot_mode_parses_a_login_line() {
    let line = "Oct  3 12:00:01 mail dovecot: imap-login: Login: user=<alice>, rip=10.0.0.5";
    let parsed = parse_line(line, "dovecot").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::UserId));
}

#[test]
fn the_dovecot_mode_drops_a_line_that_is_not_a_dovecot_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "dovecot").is_none());
}

#[test]
fn the_procmail_mode_parses_a_folder_line() {
    let line = "  Folder: /home/bob/Mail/inbox\t\t   4512";
    let parsed = parse_line(line, "procmail").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Path));
}

#[test]
fn the_procmail_mode_drops_a_line_that_is_not_a_procmail_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "procmail").is_none());
}
