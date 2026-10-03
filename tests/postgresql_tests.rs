use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};
use splash::postgresql::{self, PostgresqlPlugin};

const READY: &str =
    "2023-10-03 12:00:01.123 UTC [1234] LOG:  database system is ready to accept connections";

const FATAL: &str =
    r#"2023-10-03 12:00:03.345 UTC [1302] FATAL:  password authentication failed for user "app""#;

const STATEMENT: &str = "2023-10-03 12:00:02.234 UTC [1301] STATEMENT:  SELECT * FROM users;";

const SLOW: &str = "2023-10-03 12:00:04.456 UTC [1303] app@appdb LOG:  duration: 12.345 ms  execute <unnamed>: SELECT 1";

const DATABASE: &str = r#"2023-10-03 12:00:05.567 UTC [1304] postgres@postgres FATAL:  database "missing" does not exist"#;

/// The kinds each token of a parsed line was given, in order
fn kinds(line: &str) -> Vec<TokenKind> {
    postgresql::parse_line(line)
        .unwrap()
        .tokens
        .iter()
        .map(|token| token.kind)
        .collect()
}

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    postgresql::parse_line(line)
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
    for line in [READY, FATAL, STATEMENT, SLOW, DATABASE] {
        assert_eq!(postgresql::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_line_colors_its_time_and_process_id() {
    assert_eq!(
        field(READY, TokenKind::Timestamp),
        "2023-10-03 12:00:01.123 UTC"
    );
    assert_eq!(field(READY, TokenKind::Pid), "1234");
}

#[test]
fn a_log_line_colors_its_severity_as_a_level() {
    assert_eq!(field(READY, TokenKind::Level), "LOG");
}

#[test]
fn a_fatal_line_colors_its_severity_and_the_failure_as_failures() {
    assert_eq!(all(FATAL, TokenKind::Failure), vec!["FATAL", "failed"]);
}

#[test]
fn a_warning_is_colored_as_a_warning() {
    let line = "2023-10-03 12:00:05.567 UTC [1304] WARNING:  there is no transaction in progress";

    assert_eq!(field(line, TokenKind::Warning), "WARNING");
}

#[test]
fn a_quoted_user_is_colored_as_a_user() {
    assert_eq!(field(FATAL, TokenKind::UserId), "app");
}

#[test]
fn a_quoted_database_is_colored_as_a_module() {
    assert_eq!(
        all(DATABASE, TokenKind::Module),
        vec!["postgres", "missing"]
    );
}

#[test]
fn an_empty_quoted_name_is_only_its_quotes() {
    let line = r#"2023-10-03 12:00:05.567 UTC [1304] FATAL:  role "" does not exist"#;

    assert_eq!(postgresql::parse_line(line).unwrap().text(), line);
    assert!(!kinds(line).contains(&TokenKind::UserId));
}

#[test]
fn a_statement_is_colored_as_a_request() {
    assert_eq!(field(STATEMENT, TokenKind::Header), "STATEMENT");
    assert_eq!(field(STATEMENT, TokenKind::Request), "SELECT * FROM users;");
}

#[test]
fn an_empty_statement_is_only_its_label() {
    let line = "2023-10-03 12:00:02.234 UTC [1301] QUERY:  ";

    assert!(!kinds(line).contains(&TokenKind::Request));
}

#[test]
fn the_user_and_database_of_the_prefix_are_colored() {
    assert_eq!(field(SLOW, TokenKind::UserId), "app");
    assert_eq!(field(SLOW, TokenKind::Module), "appdb");
}

#[test]
fn an_unknown_user_and_database_are_read() {
    let line = "2023-10-03 12:00:03.345 UTC [1302] @ LOG:  connection received: host=10.0.0.5";

    assert_eq!(postgresql::parse_line(line).unwrap().text(), line);
    assert!(!kinds(line).contains(&TokenKind::UserId));
}

#[test]
fn a_slow_statement_colors_its_duration_and_statement() {
    assert_eq!(field(SLOW, TokenKind::Duration), "12.345 ms");
    assert_eq!(
        all(SLOW, TokenKind::Header),
        vec!["duration", "execute <unnamed>"]
    );
    assert_eq!(field(SLOW, TokenKind::Request), "SELECT 1");
}

#[test]
fn a_duration_alone_is_read() {
    let line = "2023-10-03 12:00:04.456 UTC [1303] LOG:  duration: 0.500 ms";

    assert_eq!(field(line, TokenKind::Duration), "0.500 ms");
}

#[test]
fn a_duration_with_an_empty_statement_has_no_request() {
    let line = "2023-10-03 12:00:04.456 UTC [1303] LOG:  duration: 0.500 ms  statement: ";

    assert!(!kinds(line).contains(&TokenKind::Request));
}

#[test]
fn a_sqlstate_is_colored_as_a_status() {
    let line =
        "2023-10-03 12:00:02.234 UTC [1301] ERROR:  42P01: relation \"users\" does not exist";

    assert_eq!(
        postgresql::parse_line(line).unwrap().tokens[8..10],
        [
            Token::new("42P01", TokenKind::Status),
            Token::new(": ", TokenKind::Punctuation),
        ]
    );
}

#[test]
fn a_line_that_is_not_a_postgresql_log_is_not_parsed() {
    assert!(postgresql::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(PostgresqlPlugin::new().name(), "postgresql");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(PostgresqlPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        PostgresqlPlugin::default().metadata().description,
        "PostgreSQL server logs and slow statements"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match PostgresqlPlugin::new().parse_line(READY) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), READY);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        PostgresqlPlugin::new().parse_line("not a postgresql log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        PostgresqlPlugin::new().detect_format(&[READY, FATAL, STATEMENT, SLOW]),
        1.0
    );
}
