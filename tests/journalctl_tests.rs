use splash::journalctl::{self, JournalctlPlugin};
use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};

const STARTED: &str =
    "Oct 03 12:00:02 web01 systemd[1]: Started nginx.service - A high performance web server.";

const EXITED: &str =
    "Oct 03 12:00:04 web01 systemd[1]: app.service: Main process exited, code=exited, status=1/FAILURE";

const FAILED: &str =
    "Oct 03 12:00:06 web01 systemd[1]: Failed to start app.service - Application server.";

const BOOT: &str = "-- Boot 6f1d2c3b4a5968778695a4b3c2d1e0f9 --";

const NO_ENTRIES: &str = "-- No entries --";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    journalctl::parse_line(line)
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
fn an_entry_keeps_every_character_of_the_original() {
    for line in [STARTED, EXITED, FAILED] {
        assert_eq!(journalctl::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_entry_colors_its_header() {
    assert_eq!(field(STARTED, TokenKind::Timestamp), "Oct 03 12:00:02");
    assert_eq!(field(STARTED, TokenKind::Tag), "systemd");
}

#[test]
fn an_entry_colors_a_unit_name() {
    assert_eq!(field(STARTED, TokenKind::Module), "nginx.service");
}

#[test]
fn a_started_unit_is_colored_as_a_success() {
    assert_eq!(field(STARTED, TokenKind::Success), "Started");
}

#[test]
fn a_unit_that_failed_to_start_is_colored_as_a_failure() {
    assert_eq!(field(FAILED, TokenKind::Failure), "Failed to start");
}

#[test]
fn an_exited_main_process_is_colored_as_a_warning() {
    assert_eq!(field(EXITED, TokenKind::Warning), "Main process exited");
}

#[test]
fn an_entry_colors_the_names_of_its_fields() {
    assert_eq!(all(EXITED, TokenKind::Header), vec!["code", "status"]);
}

#[test]
fn a_field_value_colors_the_words_inside_it() {
    let line = "Oct 03 12:00:04 web01 app[3300]: result=failed";

    assert_eq!(field(line, TokenKind::Failure), "failed");
}

#[test]
fn every_unit_type_is_colored_as_a_unit() {
    let line = concat!(
        "Oct 03 12:00:04 web01 systemd[1]: a.socket b.target c.timer d.mount e.automount ",
        "f.path g.slice h.scope i.device j.swap"
    );

    assert_eq!(all(line, TokenKind::Module).len(), 10);
}

#[test]
fn a_boot_marker_colors_the_boot_id() {
    assert_eq!(
        journalctl::parse_line(BOOT).unwrap().tokens,
        vec![
            Token::new("-- ", TokenKind::Punctuation),
            Token::new("Boot", TokenKind::Header),
            Token::new(" ", TokenKind::Plain),
            Token::new("6f1d2c3b4a5968778695a4b3c2d1e0f9", TokenKind::Transaction),
            Token::new(" --", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn another_marker_colors_its_text_as_a_message() {
    assert_eq!(field(NO_ENTRIES, TokenKind::Message), "No entries");
}

#[test]
fn an_empty_marker_is_only_punctuation() {
    assert_eq!(
        journalctl::parse_line("--  --").unwrap().tokens,
        vec![
            Token::new("-- ", TokenKind::Punctuation),
            Token::new(" --", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_line_that_is_not_journal_output_is_not_parsed() {
    assert!(journalctl::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(JournalctlPlugin::new().name(), "journalctl");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(JournalctlPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        JournalctlPlugin::default().metadata().description,
        "systemd journal output from journalctl"
    );
}

#[test]
fn the_plugin_parses_an_entry_into_tokens() {
    let parsed = match JournalctlPlugin::new().parse_line(STARTED) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the entry should parse"),
    };

    assert_eq!(parsed.text(), STARTED);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        JournalctlPlugin::new().parse_line("not journal output"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        JournalctlPlugin::new().detect_format(&[STARTED, EXITED, BOOT, NO_ENTRIES]),
        1.0
    );
}
