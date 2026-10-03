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

#[test]
fn the_vsftpd_mode_parses_a_download() {
    let line = r#"Tue Oct  3 12:00:03 2023 [pid 4322] [alice] OK DOWNLOAD: Client "10.0.0.5", "/home/alice/report.pdf", 4096 bytes, 512.00Kbyte/sec"#;
    let parsed = parse_line(line, "vsftpd").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Path));
}

#[test]
fn the_vsftpd_mode_drops_a_line_that_is_not_a_vsftpd_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "vsftpd").is_none());
}

#[test]
fn the_proftpd_mode_parses_an_extended_log_line() {
    let line = r#"10.0.0.5 - alice [03/Oct/2023:12:00:03 +0000] "RETR report.pdf" 226 4096"#;
    let parsed = parse_line(line, "proftpd").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Method));
}

#[test]
fn the_proftpd_mode_drops_a_line_that_is_not_a_proftpd_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "proftpd").is_none());
}

#[test]
fn the_pure_ftpd_mode_parses_a_login() {
    let line = "Oct  3 12:00:02 ftp pure-ftpd: (?@10.0.0.5) [INFO] alice is now logged in";
    let parsed = parse_line(line, "pure-ftpd").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_pure_ftpd_mode_drops_a_line_that_is_not_a_pure_ftpd_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "pure-ftpd").is_none());
}

#[test]
fn the_xferlog_mode_parses_a_transfer() {
    let line =
        "Tue Oct  3 12:00:01 2023 2 10.0.0.5 4096 /home/alice/report.pdf b _ o r alice ftp 0 * c";
    let parsed = parse_line(line, "xferlog").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Size));
}

#[test]
fn the_xferlog_mode_drops_a_line_that_is_not_an_xferlog_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "xferlog").is_none());
}

#[test]
fn the_ftpstats_mode_parses_a_transfer() {
    let line = "1696334401 651c0b41.10e1 alice 10.0.0.5 D 4096 2 /home/alice/report.pdf";
    let parsed = parse_line(line, "ftpstats").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Transaction));
}

#[test]
fn the_ftpstats_mode_drops_a_line_that_is_not_an_ftpstats_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "ftpstats").is_none());
}

#[test]
fn the_syslog_mode_parses_a_line_from_any_program() {
    let line = "Oct  3 12:00:01 web01 nginx[2200]: worker exited";
    let parsed = parse_line(line, "syslog").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Tag));
}

#[test]
fn the_syslog_mode_drops_a_line_that_is_not_a_syslog_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "syslog").is_none());
}

#[test]
fn the_journalctl_mode_parses_a_boot_marker() {
    let line = "-- Boot 6f1d2c3b4a5968778695a4b3c2d1e0f9 --";
    let parsed = parse_line(line, "journalctl").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Transaction));
}

#[test]
fn the_journalctl_mode_drops_a_line_that_is_not_journal_output() {
    assert!(parse_line("the maintenance window moves to 02:00", "journalctl").is_none());
}

#[test]
fn the_dmesg_mode_parses_a_kernel_message() {
    let line = "[    1.234567] usb 1-1: new high-speed USB device";
    let parsed = parse_line(line, "dmesg").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Module));
}

#[test]
fn the_dmesg_mode_drops_a_line_that_is_not_a_dmesg_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "dmesg").is_none());
}

#[test]
fn the_auth_mode_parses_an_accepted_login() {
    let line = "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice";
    let parsed = parse_line(line, "auth").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_auth_mode_drops_a_line_that_is_not_an_auth_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "auth").is_none());
}

#[test]
fn the_cron_mode_parses_a_command() {
    let line = "Oct  3 12:00:01 web01 CRON[5101]: (root) CMD (run-parts /etc/cron.hourly)";
    let parsed = parse_line(line, "cron").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Request));
}

#[test]
fn the_cron_mode_drops_a_line_that_is_not_a_cron_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "cron").is_none());
}

#[test]
fn the_ulogd_mode_parses_a_packet_log() {
    let line = "Oct  3 12:00:01 fw01 [UFW BLOCK] IN=eth0 OUT= SRC=203.0.113.7";
    let parsed = parse_line(line, "ulogd").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Header));
}

#[test]
fn the_ulogd_mode_drops_a_line_that_is_not_a_packet_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "ulogd").is_none());
}

#[test]
fn the_php_mode_parses_a_warning() {
    let line = r#"[03-Oct-2023 12:00:01 UTC] PHP Warning:  Undefined variable $total in /var/www/html/cart.php on line 42"#;
    let parsed = parse_line(line, "php").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Path));
}

#[test]
fn the_php_mode_drops_a_line_that_is_not_a_php_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "php").is_none());
}

#[test]
fn the_apache_error_mode_parses_an_error_line() {
    let line = r#"[Wed Oct 11 14:33:01 2023] [warn] [client 192.168.1.50] mod_deflate: skipping"#;
    let parsed = parse_line(line, "apache-error").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Warning));
}

#[test]
fn the_apache_error_mode_drops_a_line_that_is_not_an_apache_error_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "apache-error").is_none());
}

#[test]
fn the_mysql_mode_parses_an_error_log_line() {
    let line = r#"2023-10-03T12:00:02.234567Z 0 [Warning] [MY-010068] [Server] CA certificate ca.pem is self signed."#;
    let parsed = parse_line(line, "mysql").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Transaction));
}

#[test]
fn the_mysql_mode_drops_a_line_that_is_not_a_mysql_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "mysql").is_none());
}

#[test]
fn the_postgresql_mode_parses_a_log_line() {
    let line = r#"2023-10-03 12:00:01.123 UTC [1234] LOG:  database system is ready to accept connections"#;
    let parsed = parse_line(line, "postgresql").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Pid));
}

#[test]
fn the_postgresql_mode_drops_a_line_that_is_not_a_postgresql_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "postgresql").is_none());
}

#[test]
fn the_redis_mode_parses_a_log_line() {
    let line = r#"1234:M 03 Oct 2023 12:00:00.123 * Ready to accept connections tcp"#;
    let parsed = parse_line(line, "redis").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_redis_mode_drops_a_line_that_is_not_a_redis_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "redis").is_none());
}

#[test]
fn the_mongodb_mode_parses_a_structured_line() {
    let line = r#"{"t":{"$date":"2023-10-03T12:00:01.123+00:00"},"s":"W","c":"COMMAND","msg":"Slow query"}"#;
    let parsed = parse_line(line, "mongodb").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Warning));
}

#[test]
fn the_mongodb_mode_drops_a_line_that_is_not_a_mongodb_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "mongodb").is_none());
}

#[test]
fn the_elasticsearch_mode_parses_a_text_line() {
    let line =
        r#"[2023-10-03T12:00:01,123][INFO ][o.e.n.Node               ] [node-1] starting ..."#;
    let parsed = parse_line(line, "elasticsearch").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Module));
}

#[test]
fn the_elasticsearch_mode_drops_a_line_that_is_not_an_elasticsearch_log() {
    assert!(parse_line("the maintenance window moves to 02:00", "elasticsearch").is_none());
}

#[test]
fn the_ssh_mode_parses_a_login() {
    let line = "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice from 10.0.0.5 port 52144 ssh2";
    let parsed = parse_line(line, "ssh").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Protocol));
}

#[test]
fn the_ssh_mode_drops_a_line_that_is_not_an_sshd_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "ssh").is_none());
}

#[test]
fn the_sudo_mode_parses_a_command() {
    let line = "Oct  3 12:00:05 web01 sudo:    alice : TTY=pts/0 ; COMMAND=/usr/bin/apt update";
    let parsed = parse_line(line, "sudo").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Request));
}

#[test]
fn the_sudo_mode_drops_a_line_that_is_not_a_sudo_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "sudo").is_none());
}

#[test]
fn the_super_mode_parses_a_command() {
    let line = "alice@web01 Tue Oct  3 12:00:01 2023\tshutdown (-h now)";
    let parsed = parse_line(line, "super").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Method));
}

#[test]
fn the_super_mode_drops_a_line_that_is_not_a_super_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "super").is_none());
}

#[test]
fn the_sulog_mode_parses_a_switch() {
    let line = "SU 10/03 12:00 + pts/1 alice-root";
    let parsed = parse_line(line, "sulog").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_sulog_mode_drops_a_line_that_is_not_a_sulog_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "sulog").is_none());
}

#[test]
fn the_distcc_mode_parses_a_job_summary() {
    let line = "distccd[5101] (dcc_job_summary) client: 10.0.0.5:52144 COMPILE_OK exit:0";
    let parsed = parse_line(line, "distcc").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_distcc_mode_drops_a_line_that_is_not_a_distcc_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "distcc").is_none());
}

#[test]
fn the_icecast_mode_parses_an_error_log_line() {
    let line = "[2023-10-03  12:00:00] INFO main/main Icecast 2.4.4 server started";
    let parsed = parse_line(line, "icecast").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Level));
}

#[test]
fn the_icecast_mode_drops_a_line_that_is_not_an_icecast_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "icecast").is_none());
}

#[test]
fn the_apm_mode_parses_a_battery_line() {
    let line = "Oct  3 12:00:00 laptop apmd[800]: Battery: 87%, discharging";
    let parsed = parse_line(line, "apm").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Size));
}

#[test]
fn the_apm_mode_drops_a_line_that_is_not_an_apmd_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "apm").is_none());
}

#[test]
fn the_oops_mode_parses_an_oops_headline() {
    let line = "Oops: 0000 [#1] PREEMPT SMP NOPTI";
    let parsed = parse_line(line, "oops").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Failure));
}

#[test]
fn the_oops_mode_drops_a_line_that_is_not_part_of_an_oops() {
    assert!(parse_line("the maintenance window moves to 02:00", "oops").is_none());
}

#[test]
fn the_docker_mode_parses_a_daemon_line() {
    let line = r#"time="2023-10-03T12:00:00Z" level=info msg="Starting up""#;
    let parsed = parse_line(line, "docker").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Level));
}

#[test]
fn the_docker_mode_drops_a_line_that_is_not_a_docker_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "docker").is_none());
}

#[test]
fn the_kubernetes_mode_parses_a_klog_line() {
    let line = r#"I1003 12:00:01.123456       1 controller.go:123] Starting"#;
    let parsed = parse_line(line, "kubernetes").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Path));
}

#[test]
fn the_kubernetes_mode_drops_a_line_that_is_not_a_kubernetes_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "kubernetes").is_none());
}

#[test]
fn the_systemd_resolved_mode_parses_a_feature_level_change() {
    let line = r#"Oct 03 12:00:01 web01 systemd-resolved[600]: Using degraded feature set UDP instead of UDP+EDNS0"#;
    let parsed = parse_line(line, "systemd-resolved").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Protocol));
}

#[test]
fn the_systemd_resolved_mode_drops_a_line_that_is_not_a_systemd_resolved_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "systemd-resolved").is_none());
}

#[test]
fn the_nginx_error_mode_parses_an_error_line() {
    let line = r#"2023/10/03 12:00:04 [notice] 1200#1200: signal process started"#;
    let parsed = parse_line(line, "nginx-error").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Pid));
}

#[test]
fn the_nginx_error_mode_drops_a_line_that_is_not_an_nginx_error_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "nginx-error").is_none());
}

#[test]
fn the_git_mode_parses_a_trace_line() {
    let line = r#"12:00:01.123456 git.c:463               trace: built-in: git fetch"#;
    let parsed = parse_line(line, "git").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Request));
}

#[test]
fn the_git_mode_drops_a_line_that_is_not_a_git_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "git").is_none());
}

#[test]
fn the_cloud_init_mode_parses_a_log_line() {
    let line = r#"2023-10-03 12:00:01,123 - util.py[DEBUG]: Cloud-init v. 23.3.1"#;
    let parsed = parse_line(line, "cloud-init").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Number));
}

#[test]
fn the_cloud_init_mode_drops_a_line_that_is_not_a_cloud_init_line() {
    assert!(parse_line("the maintenance window moves to 02:00", "cloud-init").is_none());
}

#[test]
fn the_terraform_mode_parses_an_operation() {
    let line = r#"aws_instance.web: Creating..."#;
    let parsed = parse_line(line, "terraform").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Module));
}

#[test]
fn the_terraform_mode_drops_a_line_that_is_not_terraform_output() {
    assert!(parse_line("the maintenance window moves to 02:00", "terraform").is_none());
}

#[test]
fn the_ci_mode_parses_a_build_result() {
    let parsed = parse_line("Finished: SUCCESS", "ci").unwrap();

    assert!(parsed
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Success));
}

#[test]
fn the_ci_mode_keeps_a_line_of_build_output() {
    assert!(parse_line("the maintenance window moves to 02:00", "ci").is_some());
}
