use splash::rate::{RateLimit, WINDOW};
use std::time::{Duration, Instant};

#[test]
fn lines_within_the_limit_are_kept_with_their_line_endings() {
    let mut limit = RateLimit::new(3);

    let admitted = limit.admit("alpha\nbeta\n", Instant::now());

    assert_eq!(admitted.text, "alpha\nbeta\n");
    assert_eq!(admitted.dropped, None);
}

#[test]
fn lines_over_the_limit_in_one_window_are_dropped() {
    let mut limit = RateLimit::new(2);

    let admitted = limit.admit("alpha\nbeta\ngamma\ndelta\n", Instant::now());

    assert_eq!(admitted.text, "alpha\nbeta\n");
    assert_eq!(limit.finish(), Some(2));
}

#[test]
fn the_limit_counts_lines_across_calls_in_one_window() {
    let mut limit = RateLimit::new(2);
    let start = Instant::now();
    limit.admit("alpha", start);

    let admitted = limit.admit("beta\ngamma\n", start + Duration::from_millis(500));

    assert_eq!(admitted.text, "beta\n");
}

#[test]
fn a_new_window_keeps_lines_again_and_reports_what_the_last_one_dropped() {
    let mut limit = RateLimit::new(1);
    let start = Instant::now();
    limit.admit("alpha\nbeta\ngamma\n", start);

    let admitted = limit.admit("delta\nepsilon\n", start + WINDOW);

    assert_eq!(admitted.text, "delta\n");
    assert_eq!(admitted.dropped, Some(2));
}

#[test]
fn a_window_that_dropped_nothing_reports_nothing() {
    let mut limit = RateLimit::new(5);
    let start = Instant::now();
    limit.admit("alpha\n", start);

    let admitted = limit.admit("beta\n", start + WINDOW);

    assert_eq!(admitted.dropped, None);
}

#[test]
fn finishing_ends_the_window() {
    let mut limit = RateLimit::new(1);
    let start = Instant::now();
    limit.admit("alpha\nbeta\n", start);
    limit.finish();

    let admitted = limit.admit("gamma\n", start + Duration::from_millis(10));

    assert_eq!(admitted.text, "gamma\n");
    assert_eq!(limit.finish(), None);
}

#[test]
fn empty_text_holds_no_lines() {
    let mut limit = RateLimit::new(1);

    assert_eq!(limit.admit("", Instant::now()).text, "");
    assert_eq!(limit.admit("alpha\n", Instant::now()).text, "alpha\n");
}

#[test]
fn the_notice_names_the_count_and_the_limit() {
    let limit = RateLimit::new(100);

    assert_eq!(
        limit.notice(42),
        "Dropped 42 lines over the limit of 100 per second"
    );
}

#[test]
fn the_notice_names_a_single_line() {
    let limit = RateLimit::new(100);

    assert_eq!(
        limit.notice(1),
        "Dropped 1 line over the limit of 100 per second"
    );
}
