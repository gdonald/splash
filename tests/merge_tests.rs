use splash::merge::{self, Entry, Log};
use splash::zone::Zone;

const NOW: i64 = 1_700_000_000;

fn merged<'a>(logs: &[(&'a str, &'a str)]) -> Vec<Entry<'a>> {
    let logs: Vec<Log> = logs.iter().map(|(text, mode)| Log { text, mode }).collect();

    merge::merge(&logs, &Zone::Utc, NOW)
}

fn entry(log: usize, text: &str) -> Entry<'_> {
    Entry { log, text }
}

#[test]
fn entries_of_several_logs_are_ordered_by_time() {
    let api = "2023-10-03T12:00:01Z start\n2023-10-03T12:00:05Z panic\n";
    let worker = "2023-10-03T12:00:03Z job 1\n2023-10-03T12:00:07Z job 2\n";

    assert_eq!(
        merged(&[(api, "docker"), (worker, "docker")]),
        [
            entry(0, "2023-10-03T12:00:01Z start\n"),
            entry(1, "2023-10-03T12:00:03Z job 1\n"),
            entry(0, "2023-10-03T12:00:05Z panic\n"),
            entry(1, "2023-10-03T12:00:07Z job 2\n"),
        ]
    );
}

#[test]
fn lines_without_a_time_stay_with_the_line_before_them() {
    let api = "2023-10-03T12:00:05Z panic: boom\n  at main.go:12\n  at run.go:40\n2023-10-03T12:00:09Z restarted\n";
    let worker = "2023-10-03T12:00:07Z job 2\n";

    assert_eq!(
        merged(&[(api, "docker"), (worker, "docker")]),
        [
            entry(
                0,
                "2023-10-03T12:00:05Z panic: boom\n  at main.go:12\n  at run.go:40\n"
            ),
            entry(1, "2023-10-03T12:00:07Z job 2\n"),
            entry(0, "2023-10-03T12:00:09Z restarted\n"),
        ]
    );
}

#[test]
fn entries_written_at_the_same_time_keep_the_order_the_logs_were_named_in() {
    let first = "2023-10-03T12:00:01Z from the first\n";
    let second = "2023-10-03T12:00:01Z from the second\n";

    assert_eq!(
        merged(&[(second, "docker"), (first, "docker")]),
        [
            entry(0, "2023-10-03T12:00:01Z from the second\n"),
            entry(1, "2023-10-03T12:00:01Z from the first\n"),
        ]
    );
}

#[test]
fn fractions_of_a_second_order_entries() {
    let slow = "2023-10-03T12:00:01.5Z later\n";
    let fast = "2023-10-03T12:00:01.25Z sooner\n";

    assert_eq!(
        merged(&[(slow, "docker"), (fast, "docker")]),
        [
            entry(1, "2023-10-03T12:00:01.25Z sooner\n"),
            entry(0, "2023-10-03T12:00:01.5Z later\n"),
        ]
    );
}

#[test]
fn lines_before_a_logs_first_time_come_first() {
    let api = "starting up\n2023-10-03T12:00:05Z ready\n";
    let worker = "2023-10-03T12:00:01Z job 1\n";

    assert_eq!(
        merged(&[(worker, "docker"), (api, "docker")]),
        [
            entry(1, "starting up\n"),
            entry(0, "2023-10-03T12:00:01Z job 1\n"),
            entry(1, "2023-10-03T12:00:05Z ready\n"),
        ]
    );
}

#[test]
fn a_last_line_without_a_line_break_is_an_entry() {
    assert_eq!(
        merged(&[("2023-10-03T12:00:01Z done", "docker")]),
        [entry(0, "2023-10-03T12:00:01Z done")]
    );
}

#[test]
fn a_moment_holds_seconds_and_nanoseconds() {
    let moment = merge::moment_of(
        "2023-10-03T12:00:01.123456789123Z ready",
        "docker",
        &Zone::Utc,
        NOW,
    );

    assert_eq!(moment, Some((1_696_334_401, 123_456_789)));
}

#[test]
fn a_time_without_a_fraction_has_no_nanoseconds() {
    let moment = merge::moment_of("2023-10-03T12:00:01Z ready", "docker", &Zone::Utc, NOW);

    assert_eq!(moment, Some((1_696_334_401, 0)));
}

#[test]
fn a_line_without_a_time_has_no_moment() {
    assert_eq!(
        merge::moment_of("  at main.go:12", "docker", &Zone::Utc, NOW),
        None
    );
}

#[test]
fn a_line_the_mode_drops_has_no_moment() {
    assert_eq!(merge::moment_of("", "docker", &Zone::Utc, NOW), None);
}
