use splash::filter::{
    Entry, Field, Filter, FilterError, FilterOptions, Judgement, Level, Pattern, TimeRange,
};
use splash::output::{ParsedLine, Token, TokenKind};
use splash::time::BadMoment;
use splash::zone::Zone;

/// 2023-10-03 12:00:01 UTC
const OCTOBER_THIRD: i64 = 1_696_334_401;

/// A line with a timestamp and a level field
fn entry_line<'a>(timestamp: &'a str, level: &'a str, message: &'a str) -> ParsedLine<'a> {
    ParsedLine::new(vec![
        Token::new(timestamp, TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new(level, TokenKind::Level),
        Token::new(" ", TokenKind::Plain),
        Token::new(message, TokenKind::Message),
    ])
}

/// A line holding nothing but plain text
fn plain_line(text: &str) -> ParsedLine<'_> {
    ParsedLine::new(vec![Token::new(text, TokenKind::Plain)])
}

fn options() -> FilterOptions {
    FilterOptions::default()
}

fn build(options: FilterOptions) -> Filter {
    Filter::build_at(&options, Zone::Utc, OCTOBER_THIRD + 3600).unwrap()
}

fn build_error(options: FilterOptions) -> FilterError {
    Filter::build_at(&options, Zone::Utc, OCTOBER_THIRD).unwrap_err()
}

fn kinds(parsed: &ParsedLine) -> Vec<(String, TokenKind)> {
    parsed
        .tokens
        .iter()
        .map(|token| (token.text.to_string(), token.kind))
        .collect()
}

#[test]
fn level_names_are_read_in_any_case() {
    let names = [
        ("trace", Level::Trace),
        ("DEBUG", Level::Debug),
        ("dbg", Level::Debug),
        ("Info", Level::Info),
        ("information", Level::Info),
        ("notice", Level::Notice),
        ("warn", Level::Warning),
        ("WARNING", Level::Warning),
        ("error", Level::Error),
        ("ERR", Level::Error),
        ("severe", Level::Error),
        ("crit", Level::Critical),
        ("critical", Level::Critical),
        ("FATAL", Level::Critical),
        ("alert", Level::Critical),
        ("emerg", Level::Critical),
        ("emergency", Level::Critical),
        ("panic", Level::Critical),
    ];

    for (name, level) in names {
        assert_eq!(Level::from_name(name), Some(level), "{}", name);
    }
}

#[test]
fn a_word_that_is_not_a_level_name_is_not_a_level() {
    assert_eq!(Level::from_name("failed"), None);
}

#[test]
fn a_level_field_is_read_without_its_brackets_or_numbers() {
    assert_eq!(Level::from_field("[error]"), Some(Level::Error));
    assert_eq!(Level::from_field("WARN "), Some(Level::Warning));
    assert_eq!(Level::from_field("DEBUG2"), Some(Level::Debug));
}

#[test]
fn database_level_names_are_informational() {
    for name in ["LOG", "Note", "System"] {
        assert_eq!(Level::from_field(name), Some(Level::Info), "{}", name);
    }
}

#[test]
fn single_letter_level_fields_are_read() {
    let letters = [
        ("T", Level::Trace),
        ("D1", Level::Debug),
        ("I", Level::Info),
        ("N", Level::Notice),
        ("w", Level::Warning),
        ("E", Level::Error),
        ("C", Level::Critical),
        ("F", Level::Critical),
    ];

    for (letter, level) in letters {
        assert_eq!(Level::from_field(letter), Some(level), "{}", letter);
    }
}

#[test]
fn redis_level_marks_are_read() {
    assert_eq!(
        [".", "-", "*", "#"].map(Level::from_field),
        [
            Some(Level::Debug),
            Some(Level::Debug),
            Some(Level::Notice),
            Some(Level::Warning)
        ]
    );
}

#[test]
fn a_field_that_is_not_a_level_is_not_read() {
    for text in ["", "X", "Q2", "LOGS", "M1x", "System2x"] {
        assert_eq!(Level::from_field(text), None, "{}", text);
    }
}

#[test]
fn a_line_is_at_the_level_of_its_first_level_field() {
    let parsed = entry_line("2023-10-03T12:00:01Z", "ERROR", "disk is fine");

    assert_eq!(Level::of_line(&parsed), Some(Level::Error));
}

#[test]
fn a_line_with_a_warning_or_failure_field_naming_a_level_is_at_that_level() {
    let parsed = ParsedLine::new(vec![
        Token::new("failed", TokenKind::Failure),
        Token::new(" ", TokenKind::Plain),
        Token::new("error", TokenKind::Failure),
    ]);

    assert_eq!(Level::of_line(&parsed), Some(Level::Error));
}

#[test]
fn a_line_with_no_level_field_is_at_the_level_its_first_level_word_names() {
    assert_eq!(
        Level::of_line(&plain_line("disk warning: 90% full, error soon")),
        Some(Level::Warning)
    );
}

#[test]
fn a_line_naming_no_level_has_none() {
    assert_eq!(Level::of_line(&plain_line("disk is 90% full")), None);
}

#[test]
fn levels_are_ordered_by_severity() {
    assert!(Level::Trace < Level::Debug && Level::Error < Level::Critical);
}

#[test]
fn patterns_are_equal_when_their_sources_are() {
    let mut first = options();
    first.grep = vec!["a+".to_string()];
    let mut second = options();
    second.grep = vec!["b+".to_string()];

    assert_eq!(build(first.clone()).grep, build(first.clone()).grep);
    assert_ne!(build(first).grep, build(second).grep);
}

#[test]
fn a_field_filter_names_a_token_kind_and_a_value() {
    assert_eq!(
        Field::parse("status=404"),
        Ok(Field {
            kinds: vec![TokenKind::Status],
            value: "404".to_string(),
        })
    );
}

#[test]
fn an_ip_field_filter_also_matches_an_access_log_client() {
    assert_eq!(
        Field::parse("ip=10.0.0.5").unwrap().kinds,
        vec![TokenKind::Ip, TokenKind::Client]
    );
}

#[test]
fn a_malformed_field_filter_is_reported() {
    for text in ["status", "=404", "status=", " =404"] {
        assert_eq!(
            Field::parse(text),
            Err(FilterError::BadField(text.to_string())),
            "{}",
            text
        );
    }
}

#[test]
fn a_field_filter_naming_an_unknown_field_is_reported() {
    assert_eq!(
        Field::parse("colour=red"),
        Err(FilterError::UnknownField("colour".to_string()))
    );
}

#[test]
fn levels_are_read_from_comma_separated_lists() {
    let mut given = options();
    given.levels = vec!["error, WARN".to_string(), "fatal,".to_string()];

    assert_eq!(
        build(given).levels,
        vec![Level::Error, Level::Warning, Level::Critical]
    );
}

#[test]
fn an_unknown_level_is_reported() {
    let mut given = options();
    given.levels = vec!["error,loud".to_string()];

    assert_eq!(
        build_error(given),
        FilterError::UnknownLevel("loud".to_string())
    );
}

#[test]
fn since_and_until_make_a_time_range_read_in_the_zone_given() {
    let mut given = options();
    given.since = Some("1h ago".to_string());
    given.until = Some("2023-10-03T12:30:00Z".to_string());

    assert_eq!(
        build(given).time,
        Some(TimeRange {
            since: Some(OCTOBER_THIRD),
            until: Some(OCTOBER_THIRD + 1799),
            zone: Zone::Utc,
            now: OCTOBER_THIRD + 3600,
        })
    );
}

#[test]
fn no_since_or_until_means_no_time_range() {
    assert_eq!(build(options()).time, None);
}

#[test]
fn an_unreadable_time_is_reported() {
    let mut given = options();
    given.until = Some("soon".to_string());

    assert_eq!(
        build_error(given),
        FilterError::Moment(BadMoment("soon".to_string()))
    );
}

#[test]
fn an_invalid_pattern_is_reported() {
    let mut given = options();
    given.exclude = vec!["(".to_string()];

    assert!(matches!(
        build_error(given),
        FilterError::BadPattern { pattern, .. } if pattern == "("
    ));
}

#[test]
fn an_invalid_field_filter_is_reported_when_building() {
    let mut given = options();
    given.fields = vec!["status".to_string()];

    assert_eq!(
        build_error(given),
        FilterError::BadField("status".to_string())
    );
}

#[test]
fn building_with_the_local_clock_reads_times() {
    let mut given = options();
    given.since = Some("now".to_string());

    assert!(Filter::build(&given).unwrap().time.is_some());
}

#[test]
fn building_with_no_times_needs_no_clock() {
    assert_eq!(Filter::build(&options()), Ok(Filter::default()));
}

#[test]
fn every_line_is_kept_when_there_are_no_filters() {
    assert_eq!(
        Filter::default().judge(&plain_line("anything")),
        Judgement {
            matches: true,
            entry: Some(true),
        }
    );
}

#[test]
fn grep_keeps_lines_matching_any_pattern() {
    let mut given = options();
    given.grep = vec!["refused".to_string(), "timed out".to_string()];
    let filter = build(given);

    assert!(filter.judge(&plain_line("connection timed out")).matches);
    assert!(!filter.judge(&plain_line("connection accepted")).matches);
}

#[test]
fn exclude_drops_lines_matching_any_pattern() {
    let mut given = options();
    given.exclude = vec!["healthz".to_string()];
    let filter = build(given);

    assert!(!filter.judge(&plain_line("GET /healthz 200")).matches);
    assert!(filter.judge(&plain_line("GET /orders 200")).matches);
}

#[test]
fn field_filters_keep_lines_holding_every_field() {
    let mut given = options();
    given.fields = vec!["ip=10.0.0.5".to_string(), "status=404".to_string()];
    let filter = build(given);

    let line = |client: &'static str, status: &'static str| {
        ParsedLine::new(vec![
            Token::new(client, TokenKind::Client),
            Token::new(" ", TokenKind::Plain),
            Token::new(status, TokenKind::Status),
        ])
    };

    assert!(filter.judge(&line("10.0.0.5", "404")).matches);
    assert!(!filter.judge(&line("10.0.0.5", "200")).matches);
    assert!(!filter.judge(&line("10.0.0.6", "404")).matches);
}

#[test]
fn a_field_filter_matches_a_quoted_json_value() {
    let mut given = options();
    given.fields = vec!["level=info".to_string()];

    let parsed = ParsedLine::new(vec![Token::new("\"info\"", TokenKind::Level)]);

    assert!(build(given).judge(&parsed).matches);
}

#[test]
fn a_level_filter_keeps_an_entry_at_a_listed_level() {
    let mut given = options();
    given.levels = vec!["error".to_string()];
    let filter = build(given);

    assert_eq!(
        filter
            .judge(&entry_line("2023-10-03T12:00:01Z", "ERROR", "disk"))
            .entry,
        Some(true)
    );
    assert_eq!(
        filter
            .judge(&entry_line("2023-10-03T12:00:01Z", "INFO", "disk"))
            .entry,
        Some(false)
    );
}

#[test]
fn a_level_filter_drops_an_entry_with_a_timestamp_and_no_level() {
    let mut given = options();
    given.levels = vec!["error".to_string()];

    let parsed = ParsedLine::new(vec![
        Token::new("Oct  3 12:00:01", TokenKind::Timestamp),
        Token::new(" disk is fine", TokenKind::Message),
    ]);

    assert_eq!(build(given).judge(&parsed).entry, Some(false));
}

#[test]
fn a_line_with_no_level_and_no_timestamp_belongs_to_the_entry_above() {
    let mut given = options();
    given.levels = vec!["error".to_string()];

    assert_eq!(
        build(given).judge(&plain_line("    at main.rs:42")).entry,
        None
    );
}

#[test]
fn a_time_range_keeps_an_entry_written_inside_it() {
    let mut given = options();
    given.since = Some("2023-10-03T12:00:00Z".to_string());
    given.until = Some("2023-10-03T12:00:02Z".to_string());
    let filter = build(given);

    let at = |timestamp: &'static str| filter.judge(&entry_line(timestamp, "INFO", "disk")).entry;

    assert_eq!(
        [
            at("2023-10-03T11:59:59Z"),
            at("2023-10-03T12:00:00Z"),
            at("2023-10-03T12:00:02Z"),
            at("2023-10-03T12:00:03Z"),
        ],
        [Some(false), Some(true), Some(true), Some(false)]
    );
}

#[test]
fn a_time_range_with_only_an_end_keeps_everything_before_it() {
    let mut given = options();
    given.until = Some("2023-10-03T12:00:02Z".to_string());

    assert_eq!(
        build(given)
            .judge(&entry_line("2000-01-01T00:00:00Z", "INFO", "disk"))
            .entry,
        Some(true)
    );
}

#[test]
fn a_line_with_no_readable_timestamp_belongs_to_the_entry_above_a_time_range() {
    let mut given = options();
    given.since = Some("2023-10-03T12:00:00Z".to_string());

    let parsed = ParsedLine::new(vec![Token::new("[ 1234.567890]", TokenKind::Timestamp)]);

    assert_eq!(build(given).judge(&parsed).entry, None);
}

#[test]
fn an_entry_outside_the_time_range_is_dropped_whatever_its_level() {
    let mut given = options();
    given.levels = vec!["error".to_string()];
    given.since = Some("2023-10-03T13:00:00Z".to_string());

    assert_eq!(
        build(given)
            .judge(&entry_line("2023-10-03T12:00:01Z", "ERROR", "disk"))
            .entry,
        Some(false)
    );
}

#[test]
fn an_entry_at_a_listed_level_with_no_readable_timestamp_belongs_to_the_entry_above() {
    let mut given = options();
    given.levels = vec!["error".to_string()];
    given.since = Some("2023-10-03T12:00:00Z".to_string());

    assert_eq!(
        build(given)
            .judge(&entry_line("[ 1.000000]", "ERROR", "disk"))
            .entry,
        None
    );
}

#[test]
fn highlighting_without_grep_leaves_the_line_alone() {
    let parsed = plain_line("connection refused");

    assert_eq!(Filter::default().highlight(parsed.clone()), parsed);
}

#[test]
fn highlighting_marks_the_match_and_its_capture_groups() {
    let mut given = options();
    given.grep = vec![r"user (\w+) from".to_string()];

    let parsed = ParsedLine::new(vec![
        Token::new("login for user", TokenKind::Message),
        Token::new(" ", TokenKind::Plain),
        Token::new("alice", TokenKind::UserId),
        Token::new(" from 10.0.0.5", TokenKind::Message),
    ]);

    assert_eq!(
        kinds(&build(given).highlight(parsed)),
        vec![
            ("login for ".to_string(), TokenKind::Message),
            ("user".to_string(), TokenKind::Match),
            (" ".to_string(), TokenKind::Match),
            ("alice".to_string(), TokenKind::Capture),
            (" from".to_string(), TokenKind::Match),
            (" 10.0.0.5".to_string(), TokenKind::Message),
        ]
    );
}

#[test]
fn highlighting_marks_every_match_of_every_pattern() {
    let mut given = options();
    given.grep = vec!["a".to_string(), "c".to_string(), "(z)?b".to_string()];

    assert_eq!(
        kinds(&build(given).highlight(plain_line("abxcda"))),
        vec![
            ("ab".to_string(), TokenKind::Match),
            ("x".to_string(), TokenKind::Plain),
            ("c".to_string(), TokenKind::Match),
            ("d".to_string(), TokenKind::Plain),
            ("a".to_string(), TokenKind::Match),
        ]
    );
}

#[test]
fn highlighting_splits_text_with_multibyte_characters_at_character_boundaries() {
    let mut given = options();
    given.grep = vec!["é".to_string()];

    assert_eq!(
        kinds(&build(given).highlight(plain_line("café ok"))),
        vec![
            ("caf".to_string(), TokenKind::Plain),
            ("é".to_string(), TokenKind::Match),
            (" ok".to_string(), TokenKind::Plain),
        ]
    );
}

#[test]
fn a_judged_entry_decides_the_lines_that_belong_to_it() {
    let mut entry = Entry::default();
    let keep = Judgement {
        matches: true,
        entry: Some(true),
    };
    let drop = Judgement {
        matches: true,
        entry: Some(false),
    };
    let follow = Judgement {
        matches: true,
        entry: None,
    };

    assert_eq!(
        [
            entry.admit(follow),
            entry.admit(keep),
            entry.admit(follow),
            entry.admit(drop),
            entry.admit(follow),
        ],
        [false, true, true, false, false]
    );
}

#[test]
fn a_line_failing_its_own_filters_is_dropped_from_a_kept_entry() {
    let mut entry = Entry::default();

    entry.admit(Judgement {
        matches: true,
        entry: Some(true),
    });

    assert!(!entry.admit(Judgement {
        matches: false,
        entry: None,
    }));
}

#[test]
fn filter_errors_explain_themselves() {
    assert_eq!(
        FilterError::UnknownLevel("loud".to_string()).to_string(),
        "Unknown log level 'loud' (expected trace, debug, info, notice, warn, error, or critical)"
    );
    assert_eq!(
        FilterError::Moment(BadMoment("soon".to_string())).to_string(),
        BadMoment("soon".to_string()).to_string()
    );
    assert_eq!(
        FilterError::BadPattern {
            pattern: "(".to_string(),
            message: "unclosed group".to_string(),
        }
        .to_string(),
        "Invalid pattern '(': unclosed group"
    );
    assert_eq!(
        FilterError::BadField("status".to_string()).to_string(),
        "Invalid field filter 'status' (expected KEY=VALUE, such as ip=192.168.1.1)"
    );
    assert_eq!(
        FilterError::UnknownField("colour".to_string()).to_string(),
        "Unknown field 'colour'"
    );
}

#[test]
fn a_pattern_holds_its_compiled_regex() {
    let mut given = options();
    given.grep = vec!["a+".to_string()];

    let patterns: Vec<Pattern> = build(given).grep;

    assert_eq!(patterns[0].regex.as_str(), "a+");
}
