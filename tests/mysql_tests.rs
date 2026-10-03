use splash::mysql::{self, MysqlPlugin};
use splash::output::{Token, TokenKind};
use splash::plugin::{ParseResult, Plugin};

const WARNING: &str = "2023-10-03T12:00:02.234567Z 0 [Warning] [MY-010068] [Server] CA certificate ca.pem is self signed.";

const ERROR: &str =
    "2023-10-03T12:00:05.567890Z 0 [ERROR] [MY-012574] [InnoDB] Unable to lock ./ibdata1 error: 11";

const OLD: &str = "2019-10-03T12:00:06.000001Z 0 [Note] InnoDB: Buffer pool(s) load completed";

const MARIADB: &str = "2023-10-03 12:00:07 0 [Note] mariadbd: ready for connections.";

const TIME: &str = "# Time: 2023-10-03T12:00:01.123456Z";

const USER: &str = "# User@Host: app[app] @ localhost [10.0.0.5]  Id:    42";

const STATS: &str =
    "# Query_time: 2.345678  Lock_time: 0.000123 Rows_sent: 1  Rows_examined: 1000000";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    mysql::parse_line(line)
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
    for line in [WARNING, ERROR, OLD, MARIADB, TIME, USER, STATS] {
        assert_eq!(mysql::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn an_error_log_line_colors_its_time_thread_code_and_subsystem() {
    assert_eq!(
        field(WARNING, TokenKind::Timestamp),
        "2023-10-03T12:00:02.234567Z"
    );
    assert_eq!(field(WARNING, TokenKind::Pid), "0");
    assert_eq!(field(WARNING, TokenKind::Transaction), "MY-010068");
    assert_eq!(field(WARNING, TokenKind::Module), "Server");
}

#[test]
fn a_warning_label_is_colored_as_a_warning() {
    assert_eq!(field(WARNING, TokenKind::Warning), "Warning");
}

#[test]
fn an_error_label_is_colored_as_a_failure() {
    assert_eq!(all(ERROR, TokenKind::Failure), vec!["ERROR", "error"]);
}

#[test]
fn a_note_is_colored_as_a_level() {
    assert_eq!(field(OLD, TokenKind::Level), "Note");
}

#[test]
fn a_line_without_a_code_or_subsystem_is_read() {
    assert!(all(OLD, TokenKind::Transaction).is_empty());
}

#[test]
fn a_mariadb_line_colors_its_date_and_readiness() {
    assert_eq!(field(MARIADB, TokenKind::Timestamp), "2023-10-03 12:00:07");
    assert_eq!(field(MARIADB, TokenKind::Success), "ready for connections");
}

#[test]
fn the_time_of_a_slow_query_is_colored() {
    assert_eq!(
        field(TIME, TokenKind::Timestamp),
        "2023-10-03T12:00:01.123456Z"
    );
}

#[test]
fn the_user_and_host_of_a_slow_query_are_colored() {
    assert_eq!(all(USER, TokenKind::UserId), vec!["app", "app"]);
    assert_eq!(field(USER, TokenKind::Host), "localhost");
    assert_eq!(field(USER, TokenKind::Ip), "10.0.0.5");
    assert_eq!(field(USER, TokenKind::Pid), "42");
}

#[test]
fn a_slow_query_with_empty_names_leaves_them_out() {
    let line = "# User@Host: [app] @  []  Id:    42";

    assert_eq!(mysql::parse_line(line).unwrap().text(), line);
    assert_eq!(all(line, TokenKind::UserId), vec!["app"]);
}

#[test]
fn the_times_of_a_slow_query_are_durations_and_its_counts_are_numbers() {
    assert_eq!(
        all(STATS, TokenKind::Duration),
        vec!["2.345678", "0.000123"]
    );
    assert_eq!(all(STATS, TokenKind::Number), vec!["1", "1000000"]);
}

#[test]
fn text_after_the_last_pair_of_a_slow_query_is_kept() {
    let line = "# Query_time: 2.345678 !";

    assert_eq!(
        mysql::parse_line(line).unwrap().tokens.last(),
        Some(&Token::new(" !", TokenKind::Plain))
    );
}

#[test]
fn a_statement_is_colored_as_a_request() {
    for line in ["use appdb;", "SELECT * FROM orders", "  WHERE id = 1;"] {
        assert_eq!(
            mysql::parse_line(line).unwrap().tokens,
            vec![Token::new(line, TokenKind::Request)]
        );
    }
}

#[test]
fn the_lines_a_slow_query_log_opens_with_are_read() {
    for line in [
        "/usr/sbin/mysqld, Version: 8.0.34 (MySQL Community Server - GPL). started with:",
        "Tcp port: 3306  Unix socket: /var/run/mysqld/mysqld.sock",
        "Time                 Id Command    Argument",
    ] {
        assert_eq!(
            mysql::parse_line(line).unwrap().tokens,
            vec![Token::new(line, TokenKind::Message)]
        );
    }
}

#[test]
fn a_line_that_is_not_a_mysql_log_is_not_parsed() {
    assert!(mysql::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(MysqlPlugin::new().name(), "mysql");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(MysqlPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        MysqlPlugin::default().metadata().description,
        "MySQL and MariaDB error and slow query logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match MysqlPlugin::new().parse_line(WARNING) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), WARNING);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        MysqlPlugin::new().parse_line("not a mysql log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        MysqlPlugin::new().detect_format(&[WARNING, TIME, USER, STATS]),
        1.0
    );
}
