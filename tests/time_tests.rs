use splash::time::{
    civil_from_days, days_from_civil, days_in_month, now, parse_moment, parse_timestamp, weekday,
    BadMoment, Civil, SECONDS_PER_DAY,
};
use splash::zone::{Rule, Zone};

const HOUR: i64 = 3600;

/// 2023-10-03 12:00:01 UTC
const OCTOBER_THIRD: i64 = 1_696_334_401;

/// 2024-03-15 12:00:00 UTC, the clock the relative tests read
const NOW: i64 = 1_710_504_000;

fn central() -> Zone {
    Zone::Rule(Rule::parse("CST6CDT,M3.2.0,M11.1.0").unwrap())
}

fn utc(text: &str) -> Option<i64> {
    parse_timestamp(text, &Zone::Utc, NOW)
}

#[test]
fn the_epoch_is_day_zero() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
}

#[test]
fn days_are_counted_across_leap_years_and_before_the_epoch() {
    assert_eq!(days_from_civil(2000, 3, 1), 11_017);
    assert_eq!(days_from_civil(1969, 12, 31), -1);
}

#[test]
fn a_day_count_reads_back_as_its_date() {
    for (year, month, day) in [
        (1970, 1, 1),
        (2000, 2, 29),
        (2024, 12, 31),
        (1900, 3, 1),
        (1969, 12, 31),
    ] {
        assert_eq!(
            civil_from_days(days_from_civil(year, month, day)),
            (year, month, day)
        );
    }
}

#[test]
fn the_epoch_fell_on_a_thursday() {
    assert_eq!(weekday(0), 4);
    assert_eq!(weekday(-4), 0);
}

#[test]
fn months_have_their_lengths() {
    assert_eq!(
        [
            days_in_month(2023, 1),
            days_in_month(2023, 2),
            days_in_month(2024, 2),
            days_in_month(1900, 2),
            days_in_month(2000, 2),
            days_in_month(2023, 4),
        ],
        [31, 28, 29, 28, 29, 30]
    );
}

#[test]
fn the_clock_reads_after_this_code_was_written() {
    assert!(now() > NOW);
}

#[test]
fn a_civil_time_out_of_range_is_not_built() {
    assert_eq!(Civil::new(2023, 13, 1, 0, 0, 0), None);
    assert_eq!(Civil::new(2023, 2, 29, 0, 0, 0), None);
    assert_eq!(Civil::new(2023, 1, 0, 0, 0, 0), None);
    assert_eq!(Civil::new(2023, 1, 1, 24, 0, 0), None);
    assert_eq!(Civil::new(2023, 1, 1, 0, 60, 0), None);
    assert_eq!(Civil::new(2023, 1, 1, 0, 0, 61), None);
}

#[test]
fn a_civil_time_reads_back_from_its_seconds() {
    let civil = Civil::new(2023, 10, 3, 12, 0, 1).unwrap();

    assert_eq!(civil.seconds(), OCTOBER_THIRD);
    assert_eq!(Civil::from_seconds(OCTOBER_THIRD), civil);
}

#[test]
fn an_rfc_3339_timestamp_is_read_with_its_zone() {
    assert_eq!(utc("2023-10-03T12:00:01Z"), Some(OCTOBER_THIRD));
    assert_eq!(utc("2023-10-03T12:00:01.123456789Z"), Some(OCTOBER_THIRD));
    assert_eq!(utc("2023-10-03T07:00:01-05:00"), Some(OCTOBER_THIRD));
    assert_eq!(utc("2023-10-03T17:30:01+0530"), Some(OCTOBER_THIRD));
}

#[test]
fn an_iso_timestamp_with_a_space_a_comma_or_a_written_zone_is_read() {
    assert_eq!(utc("2023-10-03 12:00:01,123"), Some(OCTOBER_THIRD));
    assert_eq!(utc("2023-10-03 12:00:01.123 UTC"), Some(OCTOBER_THIRD));
    assert_eq!(utc("Tue 2023-10-03 12:00:01 GMT"), Some(OCTOBER_THIRD));
    assert_eq!(utc("2023/10/03 12:00:01"), Some(OCTOBER_THIRD));
}

#[test]
fn an_iso_timestamp_without_seconds_is_read() {
    assert_eq!(utc("2023-10-03T12:00Z"), Some(OCTOBER_THIRD - 1));
}

#[test]
fn an_iso_timestamp_without_a_zone_is_read_in_the_zone_given() {
    assert_eq!(
        parse_timestamp("2023-10-03 07:00:01", &central(), NOW),
        Some(OCTOBER_THIRD)
    );
}

#[test]
fn a_timestamp_in_brackets_or_quotes_is_read() {
    assert_eq!(utc("[2023-10-03T12:00:01Z]"), Some(OCTOBER_THIRD));
    assert_eq!(utc("\"2023-10-03T12:00:01Z\""), Some(OCTOBER_THIRD));
}

#[test]
fn a_common_log_format_timestamp_is_read_with_its_offset() {
    assert_eq!(utc("[03/Oct/2023:05:00:01 -0700]"), Some(OCTOBER_THIRD));
}

#[test]
fn a_day_month_year_timestamp_with_dashes_or_spaces_is_read() {
    assert_eq!(utc("03-Oct-2023 12:00:01 UTC"), Some(OCTOBER_THIRD));
    assert_eq!(utc("03 Oct 2023 12:00:01.001"), Some(OCTOBER_THIRD));
    assert_eq!(utc("03/Oct/2023:12:00:01.123"), Some(OCTOBER_THIRD));
}

#[test]
fn a_ctime_timestamp_is_read_in_the_zone_given() {
    assert_eq!(utc("Tue Oct  3 12:00:01 2023"), Some(OCTOBER_THIRD));
    assert_eq!(
        parse_timestamp("Tue Oct  3 07:00:01 2023", &central(), NOW),
        Some(OCTOBER_THIRD)
    );
}

#[test]
fn a_syslog_timestamp_is_placed_in_the_current_year() {
    assert_eq!(utc("Mar 15 11:00:00"), Some(NOW - HOUR));
}

#[test]
fn a_syslog_timestamp_later_than_tomorrow_is_placed_in_the_year_before() {
    let a_year_before = Civil::new(2023, 10, 3, 12, 0, 1).unwrap().seconds();

    assert_eq!(utc("Oct  3 12:00:01"), Some(a_year_before));
}

#[test]
fn a_syslog_timestamp_on_a_day_the_current_year_lacks_is_placed_in_the_year_before() {
    let clock = Civil::new(2025, 3, 1, 0, 0, 0).unwrap().seconds();
    let leap_day = Civil::new(2024, 2, 29, 12, 0, 0).unwrap().seconds();

    assert_eq!(
        parse_timestamp("Feb 29 12:00:00", &Zone::Utc, clock),
        Some(leap_day)
    );
}

#[test]
fn a_klog_timestamp_is_placed_in_the_current_year() {
    assert_eq!(utc("0315 11:00:00.123456"), Some(NOW - HOUR));
}

#[test]
fn seconds_and_milliseconds_since_the_epoch_are_read() {
    assert_eq!(utc("1696334401"), Some(OCTOBER_THIRD));
    assert_eq!(utc("1696334401.5241024"), Some(OCTOBER_THIRD));
    assert_eq!(utc("1696334401123"), Some(OCTOBER_THIRD));
}

#[test]
fn text_that_is_not_a_timestamp_is_not_read() {
    for text in [
        "",
        "hello",
        "[ 1234.567890]",
        "12345",
        "2023-13-03T12:00:01Z",
        "31/Foo/2023:12:00:01",
        "31/Feb/2023:12:00:01",
        "Foo  3 12:00:01",
        "Feb 30 12:00:01 2023",
        "1399 12:00:00",
    ] {
        assert_eq!(utc(text), None, "read '{}'", text);
    }
}

#[test]
fn now_is_the_current_moment() {
    assert_eq!(parse_moment("now", &Zone::Utc, NOW), Ok(NOW));
}

#[test]
fn today_and_yesterday_are_local_midnights() {
    let midnight = Civil::new(2024, 3, 15, 5, 0, 0).unwrap().seconds();

    assert_eq!(parse_moment("today", &central(), NOW), Ok(midnight));
    assert_eq!(
        parse_moment("yesterday", &central(), NOW),
        Ok(midnight - SECONDS_PER_DAY)
    );
}

#[test]
fn an_age_counts_back_from_now() {
    for (text, age) in [
        ("90s ago", 90),
        ("30 minutes ago", 1800),
        ("2h ago", 2 * HOUR),
        ("1 hour ago", HOUR),
        ("3 days ago", 3 * SECONDS_PER_DAY),
        ("1w ago", 7 * SECONDS_PER_DAY),
    ] {
        assert_eq!(
            parse_moment(text, &Zone::Utc, NOW),
            Ok(NOW - age),
            "{}",
            text
        );
    }
}

#[test]
fn a_date_is_its_local_midnight() {
    assert_eq!(
        parse_moment("2024-01-01", &central(), NOW),
        Ok(Civil::new(2024, 1, 1, 6, 0, 0).unwrap().seconds())
    );
}

#[test]
fn a_moment_can_be_a_timestamp() {
    assert_eq!(
        parse_moment(" 2023-10-03T12:00:01Z ", &Zone::Utc, NOW),
        Ok(OCTOBER_THIRD)
    );
}

#[test]
fn an_unreadable_moment_is_reported() {
    for text in [
        "soon",
        "2024-02-30",
        "99999999999999999999 seconds ago",
        "999999999999999999 weeks ago",
    ] {
        assert_eq!(
            parse_moment(text, &Zone::Utc, NOW),
            Err(BadMoment(text.to_string())),
            "{}",
            text
        );
    }
}

#[test]
fn an_unreadable_moment_names_the_forms_splash_reads() {
    assert_eq!(
        BadMoment("soon".to_string()).to_string(),
        "Invalid time 'soon' (expected a timestamp, a date such as 2024-01-01, today, \
         yesterday, now, or an age such as 2h ago)"
    );
}

#[test]
fn a_timestamp_span_leaves_out_brackets_quotes_and_spaces() {
    use splash::time::timestamp_span;

    assert_eq!(timestamp_span("[10/Oct/2000:13:55:36 -0700]"), (1, 27));
    assert_eq!(timestamp_span("\"2023\""), (1, 5));
    assert_eq!(timestamp_span("[]"), (2, 2));
}

#[test]
fn epoch_timestamps_are_recognized() {
    use splash::time::is_epoch;

    assert!(is_epoch("1696334401.123"));
    assert!(is_epoch("[1696334401123]"));
    assert!(!is_epoch("2023-10-03"));
}

#[test]
fn the_fraction_of_a_second_is_read_from_a_timestamp() {
    use splash::time::fraction;

    assert_eq!(fraction("2023-10-03T12:00:01.123Z"), Some("123"));
    assert_eq!(fraction("2023-10-03 12:00:01,5"), Some("5"));
    assert_eq!(fraction("1646861401.5241024"), Some("5241024"));
    assert_eq!(fraction("Oct  3 12:00:01"), None);
}

#[test]
fn a_moment_is_written_as_rfc_3339_in_a_zone() {
    use splash::time::format_rfc3339;

    assert_eq!(
        format_rfc3339(OCTOBER_THIRD, None, &Zone::Utc),
        "2023-10-03T12:00:01Z"
    );
    assert_eq!(
        format_rfc3339(OCTOBER_THIRD, Some("25"), &central()),
        "2023-10-03T07:00:01.25-05:00"
    );
    assert_eq!(
        format_rfc3339(OCTOBER_THIRD, None, &Zone::named("<+0530>-5:30").unwrap()),
        "2023-10-03T17:30:01+05:30"
    );
}

#[test]
fn a_moment_is_written_relative_to_now_in_its_largest_unit() {
    use splash::time::format_relative;

    let cases = [
        (NOW, "just now"),
        (NOW - 1, "1 second ago"),
        (NOW - 45, "45 seconds ago"),
        (NOW - 60, "1 minute ago"),
        (NOW - 2 * HOUR - 59, "2 hours ago"),
        (NOW - 3 * SECONDS_PER_DAY, "3 days ago"),
        (NOW - 800 * SECONDS_PER_DAY, "2 years ago"),
        (NOW + 300, "in 5 minutes"),
        (NOW + HOUR, "in 1 hour"),
    ];

    for (moment, expected) in cases {
        assert_eq!(format_relative(moment, NOW), expected);
    }
}
