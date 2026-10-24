use splash::dates::{DateOptions, DateRewrite, Selection, UnknownZone};
use splash::output::{ParsedLine, Token, TokenKind};
use splash::zone::{Rule, Zone};

/// 2023-10-03 12:00:01 UTC
const OCTOBER_THIRD: i64 = 1_696_334_401;

fn central() -> Zone {
    Zone::Rule(Rule::parse("CST6CDT,M3.2.0,M11.1.0").unwrap())
}

fn rewrite(options: DateOptions) -> DateRewrite {
    DateRewrite::build_at(&options, central(), OCTOBER_THIRD + 7200).unwrap()
}

/// A line with one timestamp token of the given kind and a message
fn stamped(text: &str, kind: TokenKind) -> ParsedLine<'_> {
    ParsedLine::new(vec![
        Token::new(text, kind),
        Token::new(" connect from 10.0.0.5", TokenKind::Message),
    ])
}

fn rewritten(rewrite: &DateRewrite, text: &str) -> String {
    rewrite.apply(stamped(text, TokenKind::Timestamp)).text()
}

#[test]
fn no_date_options_rewrite_nothing() {
    assert_eq!(
        DateRewrite::build(&DateOptions::default()),
        Ok(DateRewrite::default())
    );
    assert_eq!(DateRewrite::default().selection, Selection::None);
}

#[test]
fn a_rewrite_with_no_selection_leaves_a_line_alone() {
    let line = stamped("1696334401", TokenKind::Timestamp);

    assert_eq!(DateRewrite::default().apply(line.clone()), line);
}

#[test]
fn convert_date_selects_epoch_timestamps() {
    let options = DateOptions {
        convert_epoch: true,
        ..DateOptions::default()
    };

    assert_eq!(rewrite(options).selection, Selection::Epoch);
}

#[test]
fn normalizing_a_zone_or_relative_times_select_every_timestamp() {
    for options in [
        DateOptions {
            normalize: true,
            ..DateOptions::default()
        },
        DateOptions {
            tz: Some("UTC".to_string()),
            ..DateOptions::default()
        },
        DateOptions {
            relative: true,
            ..DateOptions::default()
        },
    ] {
        assert_eq!(
            rewrite(options.clone()).selection,
            Selection::All,
            "{:?}",
            options
        );
    }
}

#[test]
fn building_with_the_local_clock_reads_the_options() {
    let options = DateOptions {
        tz: Some("UTC".to_string()),
        ..DateOptions::default()
    };

    assert_eq!(DateRewrite::build(&options).unwrap().display, Zone::Utc);
}

#[test]
fn an_unknown_zone_is_reported() {
    let options = DateOptions {
        tz: Some("Mars/Base".to_string()),
        ..DateOptions::default()
    };

    assert_eq!(
        DateRewrite::build_at(&options, Zone::Utc, 0),
        Err(UnknownZone("Mars/Base".to_string()))
    );
}

#[test]
fn an_unknown_zone_names_the_forms_splash_reads() {
    assert_eq!(
        UnknownZone("Mars/Base".to_string()).to_string(),
        "Unknown time zone 'Mars/Base' (expected UTC, a name such as America/New_York, or a POSIX TZ rule)"
    );
}

#[test]
fn convert_date_writes_an_epoch_timestamp_in_the_local_zone_with_its_fraction() {
    let options = DateOptions {
        convert_epoch: true,
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "1696334401.524"),
        "2023-10-03T07:00:01.524-05:00 connect from 10.0.0.5"
    );
}

#[test]
fn convert_date_leaves_a_timestamp_that_is_not_an_epoch_alone() {
    let options = DateOptions {
        convert_epoch: true,
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "Oct  3 12:00:01"),
        "Oct  3 12:00:01 connect from 10.0.0.5"
    );
}

#[test]
fn a_zone_rewrites_every_timestamp_in_it_keeping_its_brackets() {
    let options = DateOptions {
        tz: Some("UTC".to_string()),
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "[03/Oct/2023:05:00:01 -0700]"),
        "[2023-10-03T12:00:01Z] connect from 10.0.0.5"
    );
}

#[test]
fn normalizing_reads_a_timestamp_with_no_zone_in_the_local_zone() {
    let options = DateOptions {
        normalize: true,
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "2023-10-03 07:00:01,123"),
        "2023-10-03T07:00:01.123-05:00 connect from 10.0.0.5"
    );
}

#[test]
fn relative_times_are_written_as_their_distance_from_now() {
    let options = DateOptions {
        relative: true,
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "2023-10-03T12:00:01Z"),
        "2 hours ago connect from 10.0.0.5"
    );
}

#[test]
fn a_datetime_token_is_rewritten() {
    let options = DateOptions {
        tz: Some("UTC".to_string()),
        ..DateOptions::default()
    };

    assert_eq!(
        rewrite(options)
            .apply(stamped("10/Oct/2000:13:55:36", TokenKind::DateTime))
            .tokens[0]
            .kind,
        TokenKind::DateTime
    );
}

#[test]
fn a_timestamp_that_cannot_be_read_is_left_alone() {
    let options = DateOptions {
        normalize: true,
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "[ 1234.567890]"),
        "[ 1234.567890] connect from 10.0.0.5"
    );
}

#[test]
fn tokens_that_are_not_timestamps_are_left_alone() {
    let options = DateOptions {
        normalize: true,
        ..DateOptions::default()
    };

    assert_eq!(
        rewrite(options)
            .apply(stamped("1696334401", TokenKind::Number))
            .text(),
        "1696334401 connect from 10.0.0.5"
    );
}

#[test]
fn building_at_a_clock_with_no_options_selects_nothing() {
    assert_eq!(rewrite(DateOptions::default()).selection, Selection::None);
}

#[test]
fn a_zone_writes_a_syslog_timestamp_with_its_year_and_offset() {
    let options = DateOptions {
        tz: Some("UTC".to_string()),
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "Oct  3 07:00:01"),
        "2023-10-03T12:00:01Z connect from 10.0.0.5"
    );
}

#[test]
fn a_zone_keeps_the_fraction_of_a_second() {
    let options = DateOptions {
        tz: Some("UTC".to_string()),
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "2023-10-03T07:00:01.123456-05:00"),
        "2023-10-03T12:00:01.123456Z connect from 10.0.0.5"
    );
}

#[test]
fn a_relative_time_in_the_future_is_written_ahead_of_now() {
    let options = DateOptions {
        relative: true,
        ..DateOptions::default()
    };

    assert_eq!(
        rewritten(&rewrite(options), "2023-10-03T15:00:01Z"),
        "in 1 hour connect from 10.0.0.5"
    );
}
