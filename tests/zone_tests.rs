use splash::time::{days_from_civil, SECONDS_PER_DAY};
use splash::zone::{Change, Day, Daylight, Rule, Table, Zone};
use std::path::Path;

const HOUR: i64 = 3600;

/// Seconds since the epoch at midnight UTC on a date
fn midnight(year: i64, month: u32, day: u32) -> i64 {
    days_from_civil(year, month, day) * SECONDS_PER_DAY
}

/// A TZif version 2 file with an empty version 1 block, local time types
/// holding the given offsets, the given transitions into those types, and a
/// footer rule
fn tzif(transitions: &[(i64, u8)], offsets: &[i32], footer: &str) -> Vec<u8> {
    let mut bytes = Vec::new();

    let header = |bytes: &mut Vec<u8>, transitions: u32, types: u32, characters: u32| {
        bytes.extend_from_slice(b"TZif2");
        bytes.extend_from_slice(&[0; 15]);
        for count in [0, 0, 0, transitions, types, characters] {
            bytes.extend_from_slice(&u32::to_be_bytes(count));
        }
    };

    header(&mut bytes, 0, 0, 0);
    header(
        &mut bytes,
        transitions.len() as u32,
        offsets.len() as u32,
        4,
    );

    for (moment, _) in transitions {
        bytes.extend_from_slice(&i64::to_be_bytes(*moment));
    }
    for (_, index) in transitions {
        bytes.push(*index);
    }
    for offset in offsets {
        bytes.extend_from_slice(&i32::to_be_bytes(*offset));
        bytes.extend_from_slice(&[0, 0]);
    }
    bytes.extend_from_slice(b"ABC\0");
    bytes.extend_from_slice(format!("\n{}\n", footer).as_bytes());

    bytes
}

fn us_central() -> Rule {
    Rule::parse("CST6CDT,M3.2.0,M11.1.0").unwrap()
}

#[test]
fn a_rule_with_no_daylight_saving_holds_one_offset() {
    assert_eq!(
        Rule::parse("JST-9"),
        Some(Rule {
            standard: 9 * HOUR,
            daylight: None,
        })
    );
}

#[test]
fn a_rule_reads_a_quoted_name_and_an_offset_with_minutes() {
    assert_eq!(
        Rule::parse("<+0530>-5:30").unwrap().standard,
        5 * HOUR + 1800
    );
}

#[test]
fn a_rule_reads_an_offset_with_a_plus_sign_and_seconds() {
    assert_eq!(Rule::parse("LMT+0:30:15").unwrap().standard, -(1800 + 15));
}

#[test]
fn a_rule_reads_daylight_saving_dates_by_month_week_and_day() {
    assert_eq!(
        us_central().daylight,
        Some(Daylight {
            offset: -5 * HOUR,
            start: Change {
                day: Day::Weekday {
                    month: 3,
                    week: 2,
                    day: 0,
                },
                time: 2 * HOUR,
            },
            end: Change {
                day: Day::Weekday {
                    month: 11,
                    week: 1,
                    day: 0,
                },
                time: 2 * HOUR,
            },
        })
    );
}

#[test]
fn a_rule_reads_an_explicit_daylight_offset_and_change_times() {
    let rule = Rule::parse("XXX3YYY2,J60/-1,300/26").unwrap();
    let daylight = rule.daylight.unwrap();

    assert_eq!(
        (daylight.offset, daylight.start, daylight.end),
        (
            -2 * HOUR,
            Change {
                day: Day::Julian(60),
                time: -HOUR,
            },
            Change {
                day: Day::Ordinal(300),
                time: 26 * HOUR,
            },
        )
    );
}

#[test]
fn malformed_rules_are_not_read() {
    for text in [
        "",
        "E5",
        "EST",
        "<EST5",
        "EST5EDT",
        "EST5EDT,M3.2.0",
        "EST5EDT,M3.2.0,",
        "EST5EDT,M13.2.0,M11.1.0",
        "EST5EDT,M3.6.0,M11.1.0",
        "EST5EDT,M3.2.7,M11.1.0",
        "EST5EDT,M3,M11.1.0",
        "EST5EDT,M3.2,M11.1.0",
        "EST5EDT,J0,M11.1.0",
        "EST5EDT,J,M11.1.0",
        "EST5EDT,366,M11.1.0",
        "EST5EDT,M3.2.0/x,M11.1.0",
        "EST5EDT,M3.2.0,M11.1.0x",
        "EST5EDTx",
        "EST5:",
    ] {
        assert_eq!(Rule::parse(text), None, "read '{}'", text);
    }
}

#[test]
fn a_rule_is_on_standard_time_in_winter() {
    let zone = Zone::Rule(us_central());

    assert_eq!(zone.offset_at(midnight(2024, 1, 15)), -6 * HOUR);
}

#[test]
fn a_rule_is_on_daylight_saving_time_in_summer() {
    let zone = Zone::Rule(us_central());

    assert_eq!(zone.offset_at(midnight(2024, 7, 15)), -5 * HOUR);
}

#[test]
fn a_rule_starts_daylight_saving_at_two_in_the_morning_standard_time() {
    let zone = Zone::Rule(us_central());
    let change = midnight(2024, 3, 10) + 8 * HOUR;

    assert_eq!(
        (zone.offset_at(change - 1), zone.offset_at(change)),
        (-6 * HOUR, -5 * HOUR)
    );
}

#[test]
fn a_rule_ends_daylight_saving_on_the_last_week_of_a_month() {
    let zone = Zone::Rule(Rule::parse("CET-1CEST,M3.5.0,M10.5.0/3").unwrap());
    let change = midnight(2024, 10, 27) + HOUR;

    assert_eq!(
        (zone.offset_at(change - 1), zone.offset_at(change)),
        (2 * HOUR, HOUR)
    );
}

#[test]
fn a_southern_rule_is_on_daylight_saving_time_across_the_new_year() {
    let zone = Zone::Rule(Rule::parse("AEST-10AEDT,M10.1.0,M4.1.0/3").unwrap());

    assert_eq!(
        (
            zone.offset_at(midnight(2024, 1, 15)),
            zone.offset_at(midnight(2024, 7, 15))
        ),
        (11 * HOUR, 10 * HOUR)
    );
}

#[test]
fn a_julian_day_skips_february_twenty_ninth() {
    let zone = Zone::Rule(Rule::parse("XXX0YYY,J60/0,J61/0").unwrap());

    assert_eq!(
        (
            zone.offset_at(midnight(2024, 3, 1)),
            zone.offset_at(midnight(2024, 2, 29))
        ),
        (HOUR, 0)
    );
}

#[test]
fn an_ordinal_day_counts_february_twenty_ninth() {
    let zone = Zone::Rule(Rule::parse("XXX0YYY,59/0,60/0").unwrap());

    assert_eq!(zone.offset_at(midnight(2024, 2, 29)), HOUR);
}

#[test]
fn a_zone_file_is_read_with_its_transitions_and_footer_rule() {
    let bytes = tzif(
        &[(1000, 1), (2000, 0)],
        &[-6 * 3600, -5 * 3600],
        "CST6CDT,M3.2.0,M11.1.0",
    );

    assert_eq!(
        Table::parse(&bytes),
        Some(Table {
            transitions: vec![1000, 2000],
            offsets: vec![-5 * HOUR, -6 * HOUR],
            initial: -6 * HOUR,
            rule: Some(us_central()),
        })
    );
}

#[test]
fn a_zone_file_holds_its_first_offset_before_its_first_transition() {
    let zone = Zone::Table(Table::parse(&tzif(&[(1000, 1)], &[100, 200], "")).unwrap());

    assert_eq!(zone.offset_at(999), 100);
}

#[test]
fn a_zone_file_holds_the_offset_of_the_latest_transition() {
    let zone = Zone::Table(Table::parse(&tzif(&[(1000, 1), (2000, 0)], &[100, 200], "")).unwrap());

    assert_eq!((zone.offset_at(1000), zone.offset_at(1999)), (200, 200));
}

#[test]
fn a_zone_file_with_no_footer_rule_holds_its_last_offset() {
    let zone = Zone::Table(Table::parse(&tzif(&[(1000, 1), (2000, 0)], &[100, 200], "")).unwrap());

    assert_eq!(zone.offset_at(5000), 100);
}

#[test]
fn a_zone_file_follows_its_footer_rule_after_its_last_transition() {
    let zone = Zone::Table(
        Table::parse(&tzif(&[(0, 0)], &[-6 * 3600], "CST6CDT,M3.2.0,M11.1.0")).unwrap(),
    );

    assert_eq!(zone.offset_at(midnight(2024, 7, 15)), -5 * HOUR);
}

#[test]
fn malformed_zone_files_are_not_read() {
    let good = tzif(&[(1000, 0)], &[100], "");
    let mut bad_magic = good.clone();
    bad_magic[0] = b'X';
    let mut version_zero = good.clone();
    version_zero[4] = 0;
    let mut bad_index = good.clone();
    bad_index[44 + 44 + 8] = 7;

    for (name, bytes) in [
        ("bad magic", bad_magic),
        ("version zero", version_zero),
        ("truncated header", good[..30].to_vec()),
        ("truncated data", good[..44 + 44 + 4].to_vec()),
        ("type index out of range", bad_index),
        ("no types", tzif(&[], &[], "")),
    ] {
        assert_eq!(Table::parse(&bytes), None, "read a file with {}", name);
    }
}

#[test]
fn a_zone_file_on_disk_is_read() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("Test_Zone");
    std::fs::write(&path, tzif(&[], &[3600], "XXX-1")).unwrap();

    assert_eq!(Zone::from_file(&path).unwrap().offset_at(0), HOUR);
}

#[test]
fn a_missing_zone_file_is_not_read() {
    assert_eq!(Zone::from_file(Path::new("/nonexistent/zone")), None);
}

#[test]
fn the_names_of_utc_are_utc() {
    for name in ["UTC", "Z", "GMT", "Etc/UTC", "UTC0"] {
        assert_eq!(Zone::named(name), Some(Zone::Utc), "{}", name);
    }
}

#[test]
fn a_zone_is_named_by_a_path_to_its_file() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("Test_Zone");
    std::fs::write(&path, tzif(&[], &[3600], "")).unwrap();

    assert_eq!(
        Zone::named(path.to_str().unwrap()).unwrap().offset_at(0),
        HOUR
    );
}

#[test]
fn a_zone_is_named_by_its_installed_name() {
    let zone = Zone::named("America/Chicago").unwrap();

    assert_eq!(zone.offset_at(midnight(2024, 1, 15)), -6 * HOUR);
}

#[test]
fn a_zone_is_named_by_a_posix_rule() {
    assert_eq!(
        Zone::named("CST6CDT,M3.2.0,M11.1.0"),
        Some(Zone::Rule(us_central()))
    );
}

#[test]
fn a_zone_name_reaching_outside_the_zone_directory_is_not_read() {
    assert_eq!(Zone::named("../../etc/passwd"), None);
}

#[test]
fn an_unknown_zone_name_is_not_read() {
    assert_eq!(Zone::named("Nowhere/Special"), None);
}

#[test]
fn the_tz_variable_names_the_local_zone() {
    assert_eq!(
        Zone::from_env(Some(":JST-9"), Path::new("/nonexistent")).offset_at(0),
        9 * HOUR
    );
}

#[test]
fn the_localtime_file_holds_the_local_zone_when_tz_is_unset_or_empty() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("localtime");
    std::fs::write(&path, tzif(&[], &[7200], "")).unwrap();

    assert_eq!(Zone::from_env(None, &path).offset_at(0), 2 * HOUR);
    assert_eq!(Zone::from_env(Some(""), &path).offset_at(0), 2 * HOUR);
}

#[test]
fn the_local_zone_is_utc_when_nothing_names_it() {
    assert_eq!(Zone::from_env(None, Path::new("/nonexistent")), Zone::Utc);
}

#[test]
fn the_local_zone_is_within_a_day_of_utc() {
    assert!(Zone::local().offset_at(0).abs() < SECONDS_PER_DAY);
}

#[test]
fn utc_has_no_offset() {
    assert_eq!(Zone::Utc.offset_at(1_700_000_000), 0);
}

#[test]
fn a_wall_clock_time_is_placed_with_the_offset_in_force_at_it() {
    let zone = Zone::Rule(us_central());

    assert_eq!(
        zone.to_utc(midnight(2024, 7, 15) + 12 * HOUR),
        midnight(2024, 7, 15) + 17 * HOUR
    );
}

#[test]
fn a_wall_clock_time_skipped_by_the_clock_change_reads_with_the_earlier_offset() {
    let zone = Zone::Rule(us_central());

    assert_eq!(
        zone.to_utc(midnight(2024, 3, 10) + 2 * HOUR + 1800),
        midnight(2024, 3, 10) + 8 * HOUR + 1800
    );
}

#[test]
fn a_wall_clock_time_repeated_by_the_clock_change_reads_as_its_first_occurrence() {
    let zone = Zone::Rule(us_central());

    assert_eq!(
        zone.to_utc(midnight(2024, 11, 3) + HOUR + 1800),
        midnight(2024, 11, 3) + 6 * HOUR + 1800
    );
}

#[test]
fn a_wall_clock_time_just_after_the_clock_change_reads_with_the_new_offset() {
    let zone = Zone::Rule(us_central());

    assert_eq!(
        zone.to_utc(midnight(2024, 3, 10) + 3 * HOUR + 1800),
        midnight(2024, 3, 10) + 8 * HOUR + 1800
    );
}
