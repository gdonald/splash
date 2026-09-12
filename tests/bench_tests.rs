use splash::bench::{
    measure, measure_ccze, measure_command, measure_splash, report, sample_log, which, Measurement,
};
use splash::output::OutputMode;
use splash::parser::parse_clf_line;
use splash::theme::Theme;
use std::cell::Cell;
use std::time::Duration;
use tempfile::TempDir;

fn measurement(name: &str, lines: usize, millis: u64) -> Measurement {
    Measurement::new(name, lines, Duration::from_millis(millis))
}

#[test]
fn a_measurement_reports_the_lines_it_rendered_each_second() {
    assert_eq!(measurement("splash", 2000, 2000).lines_per_second(), 1000.0);
}

#[test]
fn a_run_too_short_to_time_reports_no_rate() {
    assert_eq!(
        Measurement::new("splash", 2000, Duration::ZERO).lines_per_second(),
        0.0
    );
}

#[test]
fn a_measurement_compares_its_rate_to_the_baseline() {
    let baseline = measurement("baseline", 1000, 1000);

    assert_eq!(
        measurement("faster", 1000, 500).speed_relative_to(&baseline),
        2.0
    );
}

#[test]
fn a_baseline_too_short_to_time_leaves_nothing_to_compare_against() {
    let baseline = Measurement::new("baseline", 1000, Duration::ZERO);

    assert_eq!(
        measurement("other", 1000, 500).speed_relative_to(&baseline),
        0.0
    );
}

#[test]
fn a_report_line_names_the_run_and_its_numbers() {
    let baseline = measurement("splash clf ansi", 1000, 1000);

    let line = baseline.report_line(&baseline);

    assert!(line.contains("splash clf ansi"), "line was: {}", line);
    assert!(line.contains("1000 lines in"), "line was: {}", line);
    assert!(line.contains("1000 lines/sec"), "line was: {}", line);
    assert!(line.ends_with("1.00x"), "line was: {}", line);
}

#[test]
fn a_report_has_one_line_for_each_measurement() {
    let measurements = vec![
        measurement("first", 1000, 1000),
        measurement("second", 1000, 500),
    ];

    let report = report("Rendering 1000 log lines", &measurements);

    assert!(report.starts_with("Rendering 1000 log lines\n"));
    assert_eq!(report.lines().count(), 3);
    assert!(report.contains("2.00x"));
}

#[test]
fn a_report_without_measurements_says_so() {
    assert_eq!(
        report("Rendering nothing", &[]),
        "Rendering nothing\n  no measurements\n"
    );
}

#[test]
fn measuring_runs_the_work_the_number_of_times_asked_for() {
    let runs = Cell::new(0);

    let measurement = measure("counting", 3, || {
        runs.set(runs.get() + 1);
        7
    });

    assert_eq!(runs.get(), 3);
    assert_eq!(measurement.lines, 7);
    assert_eq!(measurement.name, "counting");
}

#[test]
fn measuring_zero_runs_still_runs_the_work_once() {
    let runs = Cell::new(0);

    measure("counting", 0, || {
        runs.set(runs.get() + 1);
        0
    });

    assert_eq!(runs.get(), 1);
}

#[test]
fn measuring_splash_reports_the_lines_it_rendered() {
    let sample = sample_log(20);

    let measurement = measure_splash(
        "splash clf plain",
        &sample,
        "clf",
        OutputMode::Plain,
        &Theme::default(),
        1,
        2,
    );

    assert_eq!(measurement.lines, 20);
    assert_eq!(measurement.name, "splash clf plain");
}

#[test]
fn a_sample_log_is_the_length_asked_for() {
    assert_eq!(sample_log(50).lines().count(), 50);
}

#[test]
fn every_sample_log_line_parses_as_common_log_format() {
    for line in sample_log(300).lines() {
        assert!(parse_clf_line(line).is_some(), "did not parse: {}", line);
    }
}

#[test]
fn an_installed_program_is_found_on_the_path() {
    assert!(which("sh").is_some());
}

#[test]
fn a_program_that_is_not_installed_is_not_found() {
    assert_eq!(which("splash-no-such-program"), None);
}

#[test]
fn a_program_named_by_path_is_found_where_it_was_named() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("colorizer");
    std::fs::write(&path, "").unwrap();

    assert_eq!(which(path.to_str().unwrap()), Some(path));
}

#[test]
fn a_path_naming_no_file_is_not_found() {
    let directory = TempDir::new().unwrap();

    assert_eq!(
        which(directory.path().join("missing").to_str().unwrap()),
        None
    );
}

#[test]
fn measuring_another_colorizer_counts_the_lines_it_was_given() {
    let measurement = measure_command("cat", "cat", &[], "alpha\nbeta\n", 1).unwrap();

    assert_eq!(measurement.lines, 2);
    assert_eq!(measurement.name, "cat");
}

#[test]
fn a_colorizer_that_is_not_installed_is_not_measured() {
    assert_eq!(
        measure_command("missing", "splash-no-such-program", &[], "alpha\n", 1),
        None
    );
}

#[test]
fn a_colorizer_that_cannot_be_run_is_not_measured() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("colorizer");
    std::fs::write(&path, "not a program").unwrap();

    assert_eq!(
        measure_command("colorizer", path.to_str().unwrap(), &[], "alpha\n", 1),
        None
    );
}

#[test]
fn ccze_is_measured_when_ccze_is_installed() {
    let measurement = measure_ccze("alpha\nbeta\n", 1);

    assert_eq!(measurement.is_some(), which("ccze").is_some());
}
