use splash::apm::{self, ApmPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin};

const DISCHARGING: &str = "Oct  3 12:00:00 laptop apmd[800]: Battery: 87%, discharging (-0.50%/min over 0:10:00), 1:23:45 (2:54:00) to empty";

const CHARGING: &str = "Oct  3 12:20:00 laptop apmd[800]: Battery: 92%, charging (+0.50%/min over 0:10:00), 3:00:00 to empty (0:16:00 to full)";

const LOW: &str = "Oct  3 12:41:00 laptop apmd[800]: Warning: BATTERY IS LOW";

/// The text of every token of the given kind, in order
fn all(line: &str, kind: TokenKind) -> Vec<String> {
    apm::parse_line(line)
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
    for line in [DISCHARGING, CHARGING, LOW] {
        assert_eq!(apm::parse_line(line).unwrap().text(), line);
    }
}

#[test]
fn a_battery_line_colors_the_charge_rate_and_times() {
    assert_eq!(field(DISCHARGING, TokenKind::Size), "87%");
    assert_eq!(field(DISCHARGING, TokenKind::Number), "-0.50%/min");
    assert_eq!(
        all(DISCHARGING, TokenKind::Duration),
        vec!["0:10:00", "1:23:45", "2:54:00"]
    );
}

#[test]
fn a_discharging_battery_is_colored_as_a_warning() {
    assert_eq!(field(DISCHARGING, TokenKind::Warning), "discharging");
}

#[test]
fn a_charging_battery_is_colored_as_a_success() {
    assert_eq!(field(CHARGING, TokenKind::Success), "charging");
}

#[test]
fn a_battery_not_charging_is_one_warning() {
    let line = "Oct  3 12:43:00 laptop apmd[800]: Battery: ?%, not charging";

    assert_eq!(field(line, TokenKind::Warning), "not charging");
    assert_eq!(field(line, TokenKind::Size), "?%");
}

#[test]
fn an_absent_battery_is_colored_as_a_failure() {
    let line = "Oct  3 12:42:00 laptop apmd[800]: Battery: absent";

    assert_eq!(field(line, TokenKind::Failure), "absent");
}

#[test]
fn a_time_of_more_than_a_day_is_one_duration() {
    let line = "Oct  3 12:30:00 laptop apmd[800]: Battery: 60%, discharging (-5% over 1d+2:00:00)";

    assert_eq!(field(line, TokenKind::Duration), "1d+2:00:00");
    assert_eq!(all(line, TokenKind::Size), vec!["60%", "-5%"]);
}

#[test]
fn a_low_battery_warning_is_colored() {
    assert_eq!(field(LOW, TokenKind::Warning), "Warning");
    assert_eq!(field(LOW, TokenKind::Failure), "BATTERY IS LOW");
}

#[test]
fn a_line_from_another_program_is_not_parsed() {
    let line = "Oct  3 12:00:01 laptop kernel: ACPI: AC Adapter [ADP1] (on-line)";

    assert!(apm::parse_line(line).is_none());
}

#[test]
fn a_line_that_is_not_in_the_format_is_not_parsed() {
    assert!(apm::parse_line("the maintenance window moves to 02:00").is_none());
}

#[test]
fn the_plugin_reports_the_name_the_mode_flag_uses() {
    assert_eq!(ApmPlugin::new().name(), "apm");
}

#[test]
fn the_plugin_reports_its_version() {
    assert_eq!(ApmPlugin::new().version().to_string(), "1.0.0");
}

#[test]
fn the_plugin_describes_the_logs_it_reads() {
    assert_eq!(
        ApmPlugin::default().metadata().description,
        "apmd Advanced Power Management logs"
    );
}

#[test]
fn the_plugin_parses_a_line_into_tokens() {
    let parsed = match ApmPlugin::new().parse_line(DISCHARGING) {
        ParseResult::Parsed(parsed) => parsed,
        _ => panic!("the line should parse"),
    };

    assert_eq!(parsed.text(), DISCHARGING);
}

#[test]
fn the_plugin_reports_no_match_for_a_line_it_does_not_recognize() {
    assert!(matches!(
        ApmPlugin::new().parse_line("not a log line"),
        ParseResult::NoMatch
    ));
}

#[test]
fn the_plugin_is_confident_about_a_log_made_only_of_lines_it_reads() {
    assert_eq!(
        ApmPlugin::new().detect_format(&[DISCHARGING, CHARGING, LOW]),
        1.0
    );
}
