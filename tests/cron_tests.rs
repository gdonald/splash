use splash::cron::{self, CronPlugin};
use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};

const COMMAND: &str =
    "Oct  3 12:00:01 web01 CRON[5101]: (root) CMD (cd / && run-parts --report /etc/cron.hourly)";

const ERROR: &str =
    "Oct  3 12:07:00 web01 cron[700]: (CRON) ERROR (getpwnam() failed): No such file or directory";

const SESSION: &str = concat!(
    "Oct  3 12:00:01 web01 CRON[5100]: pam_unix(cron:session): ",
    "session opened for user root(uid=0) by (uid=0)"
);

const JOB: &str = "Oct  3 12:10:00 web01 anacron[5300]: Job `cron.daily' started";

const FILE: &str = "root (10/03-12:00:01-5101) CMD (run-parts /etc/cron.hourly)";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    cron::parse_line(line)
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
    for line in [COMMAND, ERROR, SESSION, JOB, FILE] {
        assert_eq!(cron::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_event_colors_the_user_and_the_event() {
    assert_eq!(field(COMMAND, TokenKind::UserId), "root");
    assert_eq!(field(COMMAND, TokenKind::Method), "CMD");
}

#[test]
fn a_command_event_colors_the_command() {
    assert_eq!(
        field(COMMAND, TokenKind::Request),
        "cd / && run-parts --report /etc/cron.hourly"
    );
}

#[test]
fn an_error_event_colors_the_event_and_its_detail_as_failures() {
    assert_eq!(all(ERROR, TokenKind::Failure), vec!["ERROR", "failed"]);
}

#[test]
fn an_error_keeps_the_text_after_its_detail() {
    assert_eq!(field(ERROR, TokenKind::Message), "getpwnam() ");
    assert!(cron::parse_line(ERROR)
        .unwrap()
        .tokens
        .contains(&Token::new(
            ": No such file or directory",
            TokenKind::Message
        )));
}

#[test]
fn an_event_with_an_empty_user_and_detail_is_only_punctuation() {
    let line = "Oct  3 12:00:00 web01 cron[700]: () INFO ()";

    assert_eq!(cron::parse_line(line).unwrap().text(), line);
    assert!(all(line, TokenKind::UserId).is_empty());
}

#[test]
fn a_pam_session_line_is_colored_as_in_the_auth_mode() {
    assert_eq!(field(SESSION, TokenKind::Module), "pam_unix");
    assert_eq!(field(SESSION, TokenKind::Success), "session opened");
}

#[test]
fn an_anacron_job_is_colored_by_name() {
    assert_eq!(field(JOB, TokenKind::Module), "cron.daily");
}

#[test]
fn an_anacron_job_with_no_name_is_only_punctuation() {
    let line = "Oct  3 12:10:00 web01 anacron[5300]: Job `' started";

    assert_eq!(cron::parse_line(line).unwrap().text(), line);
    assert!(all(line, TokenKind::Module).is_empty());
}

#[test]
fn a_log_file_line_colors_the_user_date_and_process_id() {
    assert_eq!(field(FILE, TokenKind::UserId), "root");
    assert_eq!(field(FILE, TokenKind::Timestamp), "10/03-12:00:01");
    assert_eq!(field(FILE, TokenKind::Pid), "5101");
}

#[test]
fn a_log_file_event_of_two_words_is_one_event() {
    let line = "alice (10/03-12:05:00-701) BEGIN EDIT (alice)";

    assert_eq!(field(line, TokenKind::Method), "BEGIN EDIT");
}

#[test]
fn a_syslog_line_logged_by_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice";

    assert!(cron::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_a_cron_log_is_not_parsed() {
    assert!(cron::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(CronPlugin::new().name(), "cron");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(CronPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        CronPlugin::default().metadata().description,
        "cron and anacron job logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match CronPlugin::new().parse_line(COMMAND) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), COMMAND);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        CronPlugin::new().parse_line("not a cron line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        CronPlugin::new().detect_format(&[COMMAND, ERROR, SESSION, JOB, FILE]),
        1.0
    );
}
