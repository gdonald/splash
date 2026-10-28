use splash::replay::{delays, parse_speed};
use std::time::Duration;

#[test]
fn test_each_entry_waits_for_the_time_since_the_last_timestamp() {
    assert_eq!(
        delays(
            &[Some((100, 0)), Some((100, 500_000_000)), Some((103, 0))],
            1.0
        ),
        [
            Duration::ZERO,
            Duration::from_millis(500),
            Duration::from_millis(2500)
        ]
    );
}

#[test]
fn test_a_faster_speed_divides_each_wait() {
    assert_eq!(
        delays(&[Some((100, 0)), Some((110, 0))], 10.0),
        [Duration::ZERO, Duration::from_secs(1)]
    );
}

#[test]
fn test_entries_without_a_timestamp_wait_for_none_and_keep_the_last_timestamp() {
    assert_eq!(
        delays(&[None, Some((100, 0)), None, Some((102, 0))], 1.0),
        [
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_secs(2)
        ]
    );
}

#[test]
fn test_an_entry_written_before_the_last_waits_for_none() {
    assert_eq!(
        delays(&[Some((100, 0)), Some((99, 0))], 1.0),
        [Duration::ZERO, Duration::ZERO]
    );
}

#[test]
fn test_a_speed_is_a_number_above_zero() {
    assert_eq!((parse_speed("1"), parse_speed("2.5")), (Ok(1.0), Ok(2.5)));

    for text in ["0", "-1", "fast", "inf", "NaN"] {
        assert_eq!(
            parse_speed(text),
            Err(format!(
                "'{}' is not a speed (expected a number above zero, such as 1 or 10)",
                text
            ))
        );
    }
}
