use splash::toml::{parse, TomlError};
use std::error::Error;

fn pairs(text: &str) -> Vec<(String, String)> {
    parse(text).unwrap()
}

#[test]
fn a_top_level_pair_keeps_its_bare_key() {
    assert_eq!(
        pairs("mode = \"clf\"\n"),
        vec![("mode".to_string(), "clf".to_string())]
    );
}

#[test]
fn a_pair_inside_a_table_is_prefixed_with_the_table_name() {
    assert_eq!(
        pairs("[colors]\nip = \"cyan\"\n"),
        vec![("colors.ip".to_string(), "cyan".to_string())]
    );
}

#[test]
fn a_nested_table_keeps_every_segment_of_its_name() {
    assert_eq!(
        pairs("[plugins.syslog]\nenabled = false\n"),
        vec![("plugins.syslog.enabled".to_string(), "false".to_string())]
    );
}

#[test]
fn table_names_are_trimmed_around_their_separators() {
    assert_eq!(
        pairs("[ plugins . syslog ]\nenabled = true\n"),
        vec![("plugins.syslog.enabled".to_string(), "true".to_string())]
    );
}

#[test]
fn pairs_are_returned_in_file_order() {
    let keys: Vec<String> = pairs("b = 1\na = 2\n")
        .into_iter()
        .map(|(key, _)| key)
        .collect();

    assert_eq!(keys, vec!["b".to_string(), "a".to_string()]);
}

#[test]
fn blank_lines_and_comment_lines_are_skipped() {
    assert_eq!(pairs("\n# a comment\n\n   \n"), vec![]);
}

#[test]
fn a_comment_after_a_value_is_dropped() {
    assert_eq!(
        pairs("mode = \"clf\"  # the default\n"),
        vec![("mode".to_string(), "clf".to_string())]
    );
}

#[test]
fn a_hash_inside_a_string_is_part_of_the_value() {
    assert_eq!(
        pairs("ip = \"#ff5555\"\n"),
        vec![("ip".to_string(), "#ff5555".to_string())]
    );
}

#[test]
fn an_escaped_quote_inside_a_string_does_not_end_it() {
    assert_eq!(
        pairs("note = \"a \\\" then # not a comment\"\n"),
        vec![("note".to_string(), "a \" then # not a comment".to_string())]
    );
}

#[test]
fn a_quoted_key_loses_its_quotes() {
    assert_eq!(
        pairs("\"user agent\" = \"cyan\"\n"),
        vec![("user agent".to_string(), "cyan".to_string())]
    );
}

#[test]
fn a_bare_value_is_kept_as_written() {
    assert_eq!(
        pairs("enabled = true\nlimit = 42\n"),
        vec![
            ("enabled".to_string(), "true".to_string()),
            ("limit".to_string(), "42".to_string())
        ]
    );
}

#[test]
fn the_string_escapes_are_translated() {
    assert_eq!(
        pairs("note = \"a\\nb\\tc\\rd\\\\e\\\"f\"\n"),
        vec![("note".to_string(), "a\nb\tc\rd\\e\"f".to_string())]
    );
}

#[test]
fn an_unknown_escape_is_rejected() {
    assert_eq!(
        parse("note = \"a\\qb\"\n"),
        Err(TomlError {
            line: 1,
            message: "unknown escape '\\q'".to_string()
        })
    );
}

#[test]
fn an_unterminated_string_is_rejected() {
    assert_eq!(
        parse("note = \"open\n"),
        Err(TomlError {
            line: 1,
            message: "unterminated string".to_string()
        })
    );
}

#[test]
fn a_string_ending_in_a_backslash_is_rejected() {
    assert_eq!(
        parse("note = \"open\\\n"),
        Err(TomlError {
            line: 1,
            message: "unterminated string".to_string()
        })
    );
}

#[test]
fn an_unterminated_table_header_is_rejected() {
    assert_eq!(
        parse("[colors\n"),
        Err(TomlError {
            line: 1,
            message: "unterminated table header".to_string()
        })
    );
}

#[test]
fn an_empty_table_name_is_rejected() {
    assert_eq!(
        parse("[]\n"),
        Err(TomlError {
            line: 1,
            message: "empty table name".to_string()
        })
    );
}

#[test]
fn a_table_name_with_an_empty_segment_is_rejected() {
    assert_eq!(
        parse("[plugins..syslog]\n"),
        Err(TomlError {
            line: 1,
            message: "empty table name".to_string()
        })
    );
}

#[test]
fn a_line_without_an_equals_sign_is_rejected() {
    assert_eq!(
        parse("mode clf\n"),
        Err(TomlError {
            line: 1,
            message: "expected a key = value pair, found 'mode clf'".to_string()
        })
    );
}

#[test]
fn an_empty_key_is_rejected() {
    assert_eq!(
        parse("= \"clf\"\n"),
        Err(TomlError {
            line: 1,
            message: "empty key".to_string()
        })
    );
}

#[test]
fn a_missing_value_is_rejected() {
    assert_eq!(
        parse("mode =\n"),
        Err(TomlError {
            line: 1,
            message: "missing value".to_string()
        })
    );
}

#[test]
fn an_error_reports_the_line_it_was_found_on() {
    let error = parse("mode = \"clf\"\n\nbroken\n").unwrap_err();

    assert_eq!(error.line, 3);
}

#[test]
fn an_error_prints_its_line_and_message() {
    let error = TomlError {
        line: 4,
        message: "empty key".to_string(),
    };

    assert_eq!(error.to_string(), "line 4: empty key");
}

#[test]
fn a_toml_error_is_an_error() {
    let error: Box<dyn Error> = Box::new(TomlError {
        line: 1,
        message: "empty key".to_string(),
    });

    assert!(error.source().is_none());
}
