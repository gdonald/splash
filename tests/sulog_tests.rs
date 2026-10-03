use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};
use splash::sulog::{self, SulogPlugin};

const SUCCESS: &str = "SU 10/03 12:00 + pts/1 alice-root";

const FAILURE: &str = "SU 10/03 12:05 - pts/2 bob-root";

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    sulog::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    sulog::parse_line(line)
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
    for line in [SUCCESS, FAILURE] {
        assert_eq!(sulog::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_line_colors_its_date_and_terminal() {
    assert_eq!(field(SUCCESS, TokenKind::Timestamp), "10/03 12:00");
    assert_eq!(field(SUCCESS, TokenKind::Path), "pts/1");
}

#[test]
fn a_line_colors_both_users() {
    assert_eq!(all(SUCCESS, TokenKind::UserId), vec!["alice", "root"]);
}

#[test]
fn a_successful_switch_is_colored_as_a_success() {
    assert_eq!(field(SUCCESS, TokenKind::Success), "+");
}

#[test]
fn a_failed_switch_is_colored_as_a_failure() {
    assert_eq!(field(FAILURE, TokenKind::Failure), "-");
}

#[test]
fn an_unknown_terminal_is_plain() {
    let line = "SU 10/03 12:15 - ? mallory-root";

    assert!(all(line, TokenKind::Path).is_empty());
}

#[test]
fn a_line_without_a_target_user_colors_only_the_one_who_ran_su() {
    let line = "SU 10/03 12:15 - pts/3 mallory-";

    assert_eq!(sulog::parse_line(line).unwrap().text(), line);
    assert_eq!(all(line, TokenKind::UserId), vec!["mallory"]);
    assert_eq!(kinds(line).last(), Some(&TokenKind::Punctuation));
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(sulog::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(SulogPlugin::new().name(), "sulog");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(SulogPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        SulogPlugin::default().metadata().description,
        "System V su command logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match SulogPlugin::new().parse_line(SUCCESS) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), SUCCESS);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        SulogPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(SulogPlugin::new().detect_format(&[SUCCESS, FAILURE]), 1.0);
}
