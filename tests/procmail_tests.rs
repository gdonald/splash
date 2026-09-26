use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::procmail::{self, ProcmailPlugin};

const FROM: &str = "From alice@example.com  Tue Oct  3 12:00:01 2023";

const SUBJECT: &str = " Subject: Quarterly report";

const FOLDER: &str = "  Folder: /home/bob/Mail/inbox\t\t\t\t\t\t   4512";

const MATCH: &str = r#"procmail: Match on "^From:.*alice@example.com""#;

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    procmail::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of the first token of the given kind
fn field(line: &str, kind: TokenKind) -> String {
    procmail::parse_line(line)
        .unwrap()
        .tokens
        .into_iter()
        .find(|token| token.kind == kind)
        .unwrap_or_else(|| panic!("no {} token in '{}'", kind.name(), line))
        .text
        .to_string()
}

#[test]
fn a_from_line_keeps_every_character_of_the_original() {
    assert_eq!(procmail::parse_line(FROM).unwrap().text(), FROM);
}

#[test]
fn a_from_line_colors_the_sender_and_the_date() {
    assert_eq!(field(FROM, TokenKind::Email), "alice@example.com");
    assert_eq!(
        field(FROM, TokenKind::Timestamp),
        "Tue Oct  3 12:00:01 2023"
    );
}

#[test]
fn a_from_line_colors_its_label_as_a_header() {
    assert_eq!(field(FROM, TokenKind::Header), "From");
}

#[test]
fn a_subject_line_keeps_every_character_of_the_original() {
    assert_eq!(procmail::parse_line(SUBJECT).unwrap().text(), SUBJECT);
}

#[test]
fn a_subject_line_colors_the_subject_as_message_text() {
    assert_eq!(field(SUBJECT, TokenKind::Message), " Quarterly report");
}

#[test]
fn an_empty_subject_line_is_only_its_label() {
    assert_eq!(
        procmail::parse_line(" Subject:").unwrap().tokens,
        vec![
            Token::new(" ", TokenKind::Plain),
            Token::new("Subject", TokenKind::Header),
            Token::new(":", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_folder_line_keeps_every_character_of_the_original() {
    assert_eq!(procmail::parse_line(FOLDER).unwrap().text(), FOLDER);
}

#[test]
fn a_folder_line_colors_where_the_mail_went_and_its_size() {
    assert_eq!(field(FOLDER, TokenKind::Path), "/home/bob/Mail/inbox");
    assert_eq!(field(FOLDER, TokenKind::Size), "4512");
}

#[test]
fn a_diagnostic_keeps_every_character_of_the_original() {
    assert_eq!(procmail::parse_line(MATCH).unwrap().text(), MATCH);
}

#[test]
fn a_diagnostic_colors_its_prefix() {
    assert_eq!(field(MATCH, TokenKind::Tag), "procmail");
}

#[test]
fn a_matched_recipe_is_colored_as_a_success() {
    assert_eq!(field(MATCH, TokenKind::Success), "Match on");
}

#[test]
fn an_unmatched_recipe_is_colored_as_a_warning() {
    let line = r#"procmail: No match on "^Subject:.*invoice""#;

    assert_eq!(field(line, TokenKind::Warning), "No match on");
}

#[test]
fn a_quoted_path_is_colored_as_a_path() {
    let line = r#"procmail: Couldn't create "/var/mail/bob""#;

    assert_eq!(field(line, TokenKind::Path), "/var/mail/bob");
}

#[test]
fn a_failure_is_colored_as_a_failure() {
    let line = r#"procmail: Couldn't create "/var/mail/bob""#;

    assert_eq!(field(line, TokenKind::Failure), "Couldn't");
}

#[test]
fn a_quoted_assignment_colors_the_variable_it_sets() {
    let line = r#"procmail: Assigning "MAILDIR=/home/bob/Mail""#;

    assert_eq!(field(line, TokenKind::Header), "MAILDIR");
}

#[test]
fn a_diagnostic_with_a_process_id_colors_it() {
    let line = "procmail: [4321] Tue Oct  3 12:00:01 2023";

    assert_eq!(field(line, TokenKind::Pid), "4321");
}

#[test]
fn a_diagnostic_that_is_only_a_date_colors_it_as_a_timestamp() {
    let line = "procmail: [4321] Tue Oct  3 12:00:01 2023";

    assert_eq!(
        field(line, TokenKind::Timestamp),
        "Tue Oct  3 12:00:01 2023"
    );
}

#[test]
fn a_diagnostic_without_a_process_id_has_no_process_id_token() {
    assert!(!kinds(MATCH).contains(&TokenKind::Pid));
}

#[test]
fn a_line_that_is_not_a_procmail_log_is_not_parsed() {
    assert!(procmail::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(ProcmailPlugin::new().name(), "procmail");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(ProcmailPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        ProcmailPlugin::default().metadata().description,
        "Procmail mail filtering and delivery logs"
    );
}

#[test]
fn the_plugin_parses_a_from_line_into_tokens() {
    let parsed = match ProcmailPlugin::new().parse_line(FROM) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the from line should parse"),
    };

    assert_eq!(parsed.text(), FROM);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        ProcmailPlugin::new().parse_line("not a procmail log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        ProcmailPlugin::new().detect_format(&[FROM, SUBJECT, FOLDER, MATCH]),
        1.0
    );
}

#[test]
fn the_plugin_is_not_confident_about_a_log_it_cannot_read() {
    assert_eq!(
        ProcmailPlugin::new().detect_format(&["hello", "world"]),
        0.0
    );
}
