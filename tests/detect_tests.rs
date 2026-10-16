use splash::detect::{self, Detection};
use splash::output::TokenKind;
use std::path::PathBuf;

const POSTFIX: &str = "Oct  3 12:00:01 mail postfix/smtpd[1234]: connect from unknown[10.0.0.5]";

const SSHD: &str =
    "Oct  3 12:00:02 mail sshd[4101]: Accepted publickey for alice from 10.0.0.5 port 52144 ssh2";

const SUDO: &str =
    "Oct  3 12:00:03 web01 sudo:    alice : TTY=pts/0 ; PWD=/home/alice ; USER=root ; COMMAND=/usr/bin/apt update";

const CLF: &str =
    "127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] \"GET /apache_pb.gif HTTP/1.0\" 200 2326";

const COMBINED: &str =
    "10.0.0.5 - - [03/Oct/2023:12:00:01 +0000] \"GET / HTTP/1.1\" 200 512 \"-\" \"curl/8.0\"";

const FREE_TEXT: &str = "maintenance window moves to 02:00";

/// The lines of one of the example log files
fn example(filename: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("examples")
            .join(filename),
    )
    .unwrap()
}

/// The mode detection picks for an example log file
fn detected(filename: &str) -> Option<String> {
    let contents = example(filename);
    let lines: Vec<&str> = contents.lines().collect();

    detect::detect(&lines).map(|detection| detection.mode)
}

/// The kinds each token of an auto mode parse was given, in order
fn auto_kinds(line: &str, mode: &str) -> Vec<TokenKind> {
    detect::parse_auto_line(line, mode)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The kinds each token of a parse with one mode was given, in order
fn kinds_as(line: &str, mode: &str) -> Vec<TokenKind> {
    detect::parse_as(mode, line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

#[test]
fn the_candidate_modes_are_every_plugin_and_the_common_log_format_in_name_order() {
    let modes = detect::candidate_modes();
    let mut sorted = modes.clone();
    sorted.sort();

    assert_eq!(modes, sorted);
    assert_eq!(modes.len(), splash::registry::builtins().count() + 1);
    assert!(modes.contains(&"clf".to_string()));
}

#[test]
fn a_mode_scores_the_lines_it_reads_and_the_fields_it_picks_out() {
    let detection = detect::score_mode("clf", &[CLF, FREE_TEXT]);

    assert_eq!(
        detection,
        Detection {
            mode: "clf".to_string(),
            matched: 1,
            fields: 9,
            confidence: 0.5,
        }
    );
}

#[test]
fn a_plugin_mode_takes_its_confidence_from_the_plugin() {
    let detection = detect::score_mode("postfix", &[POSTFIX, POSTFIX, FREE_TEXT, FREE_TEXT]);

    assert_eq!((detection.matched, detection.confidence), (2, 0.5));
}

#[test]
fn the_common_log_format_has_no_confidence_about_an_empty_sample() {
    assert_eq!(detect::score_mode("clf", &[]).confidence, 0.0);
}

#[test]
fn a_mode_splash_does_not_know_has_no_confidence() {
    let detection = detect::score_mode("not-a-mode", &[POSTFIX]);

    assert_eq!((detection.matched, detection.confidence), (0, 0.0));
}

#[test]
fn ranking_puts_the_mode_reading_the_most_lines_first() {
    let ranking = detect::rank(&[CLF, CLF, POSTFIX]);

    assert_eq!((ranking[0].mode.as_str(), ranking[0].matched), ("clf", 2));
}

#[test]
fn ranking_ignores_blank_lines() {
    let ranking = detect::rank(&[POSTFIX, "", "   "]);

    assert_eq!(ranking[0].confidence, 1.0);
}

#[test]
fn ranking_puts_a_syslog_family_mode_below_a_mode_reading_the_same_lines() {
    let ranking = detect::rank(&[SUDO]);
    let position = |mode: &str| ranking.iter().position(|d| d.mode == mode).unwrap();

    assert!(position("sudo") < position("auth"));
}

#[test]
fn ranking_puts_the_mode_picking_out_more_fields_first_among_modes_reading_the_same_lines() {
    let ranking = detect::rank(&[COMBINED]);
    let position = |mode: &str| ranking.iter().position(|d| d.mode == mode).unwrap();

    assert!(position("httpd") < position("clf"));
}

#[test]
fn ranking_falls_back_to_the_mode_name_when_everything_else_ties() {
    let ranking = detect::rank(&[CLF]);
    let position = |mode: &str| ranking.iter().position(|d| d.mode == mode).unwrap();

    assert_eq!(
        ranking[position("clf")].fields,
        ranking[position("httpd")].fields
    );
    assert!(position("clf") < position("httpd"));
}

#[test]
fn detection_is_unclear_when_no_mode_reads_enough_of_the_sample() {
    assert_eq!(detect::detect(&[POSTFIX, FREE_TEXT, FREE_TEXT]), None);
}

#[test]
fn detection_is_unclear_for_an_empty_sample() {
    assert_eq!(detect::detect(&[]), None);
}

#[test]
fn detection_accepts_a_mode_reading_the_minimum_share_of_the_sample() {
    let detection = detect::detect(&[POSTFIX, FREE_TEXT]).unwrap();

    assert_eq!(
        (detection.mode.as_str(), detection.confidence),
        ("postfix", detect::MIN_CONFIDENCE)
    );
}

#[test]
fn detection_picks_the_format_of_each_single_format_example_log() {
    let expected = [
        ("apache_error.log", "apache-error"),
        ("apm_apmd.log", "apm"),
        ("auth_sshd.log", "ssh"),
        ("caddy_access.log", "caddy"),
        ("ci_github.log", "ci"),
        ("ci_jenkins.log", "ci"),
        ("clf_basic.log", "clf"),
        ("cloud_init.log", "cloud-init"),
        ("cron_syslog.log", "cron"),
        ("distcc_logfile.log", "distcc"),
        ("dmesg_default.log", "dmesg"),
        ("docker_json.log", "docker"),
        ("docker_timestamps.log", "docker"),
        ("dovecot_syslog.log", "dovecot"),
        ("elasticsearch_json.log", "elasticsearch"),
        ("exim_mainlog.log", "exim"),
        ("fetchmail_syslog.log", "fetchmail"),
        ("ftpstats_transfers.log", "ftpstats"),
        ("git_daemon.log", "git"),
        ("haproxy_http.log", "haproxy"),
        ("httpd_combined.log", "httpd"),
        ("icecast_error.log", "icecast"),
        ("journalctl_short.log", "journalctl"),
        ("kubernetes_cri.log", "kubernetes"),
        ("mongodb_json.log", "mongodb"),
        ("mysql_slow.log", "mysql"),
        ("nginx_error.log", "nginx-error"),
        ("oops_kernel.log", "oops"),
        ("php_error.log", "php"),
        ("postfix_delivery.log", "postfix"),
        ("postgresql_default.log", "postgresql"),
        ("procmail_verbose.log", "procmail"),
        ("proftpd_system.log", "proftpd"),
        ("pureftpd_altlog.log", "pure-ftpd"),
        ("redis_server.log", "redis"),
        ("resolved_journal.log", "systemd-resolved"),
        ("squid_native.log", "squid"),
        ("ssh_sshd.log", "ssh"),
        ("sudo_syslog.log", "sudo"),
        ("sulog_solaris.log", "sulog"),
        ("super_logfile.log", "super"),
        ("syslog_rfc5424.log", "syslog"),
        ("syslog_traditional.log", "syslog"),
        ("terraform_log.log", "terraform"),
        ("ulogd_kernel.log", "ulogd"),
        ("varnish_transaction.log", "varnish"),
        ("vsftpd_file.log", "vsftpd"),
        ("xferlog_transfers.log", "xferlog"),
    ];

    let wrong: Vec<String> = expected
        .iter()
        .filter(|(filename, mode)| detected(filename).as_deref() != Some(*mode))
        .map(|(filename, mode)| {
            format!(
                "{} detected as {:?}, not {}",
                filename,
                detected(filename),
                mode
            )
        })
        .collect();

    assert!(wrong.is_empty(), "{:#?}", wrong);
}

#[test]
fn detection_finds_no_format_in_free_text() {
    assert_eq!(detected("adhoc_mixed.log"), None);
}

#[test]
fn both_spellings_of_auto_mode_are_auto() {
    assert!(detect::is_auto("auto"));
    assert!(detect::is_auto("auto:postfix"));
    assert!(!detect::is_auto("postfix"));
}

#[test]
fn a_detected_mode_carries_the_mode_detection_chose() {
    assert_eq!(detect::detected_mode("auto:postfix"), Some("postfix"));
    assert_eq!(detect::detected_mode("auto"), None);
}

#[test]
fn resolving_leaves_a_named_mode_alone() {
    assert_eq!(detect::resolve_mode("syslog", POSTFIX), "syslog");
}

#[test]
fn resolving_auto_carries_the_detected_mode() {
    assert_eq!(
        detect::resolve_mode("auto", &example("postfix_delivery.log")),
        "auto:postfix"
    );
}

#[test]
fn resolving_auto_keeps_auto_when_the_format_is_unclear() {
    assert_eq!(detect::resolve_mode("auto", FREE_TEXT), "auto");
}

#[test]
fn resolving_auto_reads_only_the_first_lines_of_the_input() {
    let mut contents = format!("{}\n", FREE_TEXT).repeat(detect::SAMPLE_SIZE);
    contents.push_str(&format!("{}\n", CLF).repeat(detect::SAMPLE_SIZE * 2));

    assert_eq!(detect::resolve_mode("auto", &contents), "auto");
}

#[test]
fn resolving_a_detected_mode_leaves_it_alone() {
    assert_eq!(detect::resolve_mode("auto:postfix", CLF), "auto:postfix");
}

#[test]
fn auto_mode_has_nothing_to_emit_for_a_blank_line() {
    assert!(detect::parse_auto_line("   ", "auto:postfix").is_none());
}

#[test]
fn auto_mode_parses_a_line_with_the_detected_mode() {
    assert_eq!(
        auto_kinds(POSTFIX, "auto:postfix"),
        kinds_as(POSTFIX, "postfix")
    );
}

#[test]
fn auto_mode_detects_a_line_the_detected_mode_does_not_read() {
    assert_eq!(auto_kinds(SSHD, "auto:postfix"), kinds_as(SSHD, "ssh"));
}

#[test]
fn auto_mode_detects_every_line_when_the_format_was_unclear() {
    assert_eq!(auto_kinds(CLF, "auto"), kinds_as(CLF, "clf"));
}

#[test]
fn auto_mode_falls_back_to_ad_hoc_for_a_line_no_mode_reads() {
    let kinds = auto_kinds("GET 10.0.0.5 404", "auto:postfix");

    assert_eq!(
        kinds,
        vec![
            TokenKind::HttpVerb,
            TokenKind::Plain,
            TokenKind::Ip,
            TokenKind::Plain,
            TokenKind::Number,
        ]
    );
}

#[test]
fn parsing_with_one_mode_reads_the_common_log_format() {
    assert!(detect::parse_as("clf", CLF).is_some());
}

#[test]
fn parsing_with_one_mode_reads_nothing_it_does_not_match() {
    assert!(detect::parse_as("postfix", FREE_TEXT).is_none());
}

#[test]
fn parsing_with_a_mode_splash_does_not_know_reads_nothing() {
    assert!(detect::parse_as("not-a-mode", POSTFIX).is_none());
}

#[test]
fn detection_prefers_a_program_mode_reading_enough_of_the_sample_over_the_syslog_family() {
    let detection = detect::detect(&[POSTFIX, POSTFIX, SSHD]).unwrap();

    assert_eq!(detection.mode, "postfix");
}

#[test]
fn detection_picks_the_syslog_family_when_no_program_mode_reads_enough_of_the_sample() {
    let detection = detect::detect(&[POSTFIX, SSHD, SUDO]).unwrap();

    assert_eq!(detection.mode, "auth");
}
