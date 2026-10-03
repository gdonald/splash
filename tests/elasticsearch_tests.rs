use splash::elasticsearch::{self, ElasticsearchPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const STARTING: &str =
    "[2023-10-03T12:00:01,123][INFO ][o.e.n.Node               ] [node-1] starting ...";

const SLOW: &str = r#"[2023-10-03T12:00:05,567][WARN ][i.s.s.query              ] [node-1] [logs][0] took[2.3s], took_millis[2345], total_hits[10 hits], stats[], source[{"query":{}}], id[abc],"#;

const ERROR: &str = "[2023-10-03T12:00:04,456][ERROR][o.e.b.ElasticsearchUncaughtExceptionHandler] [] uncaught exception in thread [main]";

const JSON: &str = r#"{"type": "server", "timestamp": "2023-10-03T12:00:01,123Z", "level": "INFO", "component": "o.e.n.Node", "cluster.name": "es-prod", "node.name": "node-1", "message": "started", "cluster.uuid": "Xy7AbC12Qz"}"#;

const ECS: &str = r#"{"@timestamp":"2023-10-03T12:00:03.345Z", "log.level":"ERROR", "message":"fatal error in thread [main]", "error.stack_trace":"java.lang.OutOfMemoryError: Java heap space"}"#;

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    elasticsearch::parse_line(line)
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
    for line in [STARTING, SLOW, ERROR, JSON, ECS] {
        assert_eq!(elasticsearch::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_text_line_colors_its_date_level_logger_and_node() {
    assert_eq!(
        field(STARTING, TokenKind::Timestamp),
        "2023-10-03T12:00:01,123"
    );
    assert_eq!(field(STARTING, TokenKind::Level), "INFO");
    assert_eq!(field(STARTING, TokenKind::Module), "o.e.n.Node");
    assert_eq!(field(STARTING, TokenKind::Host), "node-1");
}

#[test]
fn a_warning_is_colored_as_a_warning() {
    assert_eq!(field(SLOW, TokenKind::Warning), "WARN");
}

#[test]
fn an_error_is_colored_as_a_failure() {
    assert_eq!(field(ERROR, TokenKind::Failure), "ERROR");
    assert_eq!(elasticsearch::level_kind("FATAL"), TokenKind::Failure);
}

#[test]
fn a_logger_too_long_for_its_padding_and_an_empty_node_are_read() {
    assert_eq!(
        field(ERROR, TokenKind::Module),
        "o.e.b.ElasticsearchUncaughtExceptionHandler"
    );
    assert!(all(ERROR, TokenKind::Host).is_empty());
}

#[test]
fn a_bracketed_name_in_a_message_is_colored_as_a_module() {
    assert_eq!(
        all(SLOW, TokenKind::Module),
        vec!["i.s.s.query", "logs", "0"]
    );
}

#[test]
fn slow_log_values_are_colored_by_their_names() {
    assert_eq!(all(SLOW, TokenKind::Duration), vec!["2.3s", "2345"]);
    assert_eq!(field(SLOW, TokenKind::Number), "10 hits");
    assert_eq!(field(SLOW, TokenKind::Request), r#"{"query":{}}"#);
    assert_eq!(field(SLOW, TokenKind::Transaction), "abc");
}

#[test]
fn an_empty_slow_log_value_is_only_its_name_and_brackets() {
    assert!(all(SLOW, TokenKind::Header).contains(&"stats".to_string()));
}

#[test]
fn a_slow_log_value_with_an_unknown_name_is_message_text() {
    let line =
        "[2023-10-03T12:00:05,567][WARN ][i.s.s.query] [node-1] search_type[QUERY_THEN_FETCH]";

    assert!(all(line, TokenKind::Message).contains(&"QUERY_THEN_FETCH".to_string()));
}

#[test]
fn a_json_line_colors_its_fields_by_key() {
    assert_eq!(
        field(JSON, TokenKind::Timestamp),
        "2023-10-03T12:00:01,123Z"
    );
    assert_eq!(field(JSON, TokenKind::Level), "INFO");
    assert_eq!(field(JSON, TokenKind::Module), "o.e.n.Node");
    assert_eq!(field(JSON, TokenKind::Host), "node-1");
    assert_eq!(all(JSON, TokenKind::Tag), vec!["server", "es-prod"]);
    assert_eq!(field(JSON, TokenKind::Transaction), "Xy7AbC12Qz");
}

#[test]
fn an_ecs_line_colors_its_level_and_stack_trace_as_failures() {
    assert_eq!(
        all(ECS, TokenKind::Failure),
        vec!["ERROR", "java.lang.OutOfMemoryError: Java heap space"]
    );
}

#[test]
fn json_values_are_colored_by_their_keys() {
    assert_eq!(
        elasticsearch::value_kind("elasticsearch.slowlog.took", "2.3s"),
        Some(TokenKind::Duration)
    );
    assert_eq!(
        elasticsearch::value_kind("elasticsearch.slowlog.source", "{}"),
        Some(TokenKind::Request)
    );
    assert_eq!(elasticsearch::value_kind("ecs.version", "1.2.0"), None);
}

#[test]
fn a_line_that_is_not_an_elasticsearch_log_is_not_parsed() {
    assert!(elasticsearch::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(ElasticsearchPlugin::new().name(), "elasticsearch");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(ElasticsearchPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        ElasticsearchPlugin::default().metadata().description,
        "Elasticsearch server and slow logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match ElasticsearchPlugin::new().parse_line(STARTING) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), STARTING);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        ElasticsearchPlugin::new().parse_line("not an elasticsearch log"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        ElasticsearchPlugin::new().detect_format(&[STARTING, SLOW, JSON, ECS]),
        1.0
    );
}
