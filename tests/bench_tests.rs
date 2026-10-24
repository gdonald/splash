use splash::bench::{
    faster, fastest_run, max_rss_bytes, measure, measure_ccze, measure_command, measure_splash,
    report, run_process, sample_line, sample_log, selected_tiers, tier_report_line,
    wait_with_peak_memory, which, write_sample_log, Measurement, ProcessRun, SIZE_TIERS,
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

#[test]
fn a_sample_log_is_made_of_its_sample_lines() {
    assert_eq!(sample_log(3), (0..3).map(sample_line).collect::<String>());
}

#[test]
fn a_written_sample_log_holds_the_sample_and_reports_its_size() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("sample.log");

    let written = write_sample_log(&path, 500).unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), sample_log(500));
    assert_eq!(written, sample_log(500).len() as u64);
}

#[test]
fn a_sample_log_cannot_be_written_into_a_missing_directory() {
    assert!(write_sample_log(std::path::Path::new("missing/sample.log"), 1).is_err());
}

#[test]
fn the_size_tiers_cover_small_medium_large_and_huge_files() {
    let bytes = |lines: usize| {
        (0..lines)
            .map(|number| sample_line(number).len())
            .sum::<usize>()
    };
    let sizes: Vec<(&str, usize)> = SIZE_TIERS
        .iter()
        .map(|tier| (tier.name, tier.lines))
        .collect();

    assert_eq!(
        sizes,
        [
            ("small", 10_000),
            ("medium", 600_000),
            ("large", 2_500_000),
            ("huge", 13_000_000)
        ]
    );
    assert!(bytes(10_000) < 1_000_000);
    assert!((1_000_000..100_000_000).contains(&bytes(600_000)));
}

#[test]
fn the_small_and_medium_tiers_are_timed_unless_others_are_asked_for() {
    assert_eq!(selected_tiers(None), Ok(SIZE_TIERS[..2].to_vec()));
}

#[test]
fn tiers_are_chosen_by_name_in_the_order_given() {
    assert_eq!(
        selected_tiers(Some("huge, small,")),
        Ok(vec![SIZE_TIERS[3], SIZE_TIERS[0]])
    );
}

#[test]
fn a_tier_name_that_is_no_tier_is_reported() {
    assert_eq!(
        selected_tiers(Some("small,giant")),
        Err("Unknown size 'giant' (expected small, medium, large, or huge)".to_string())
    );
}

fn run(millis: u64, peak_memory: Option<u64>) -> ProcessRun {
    ProcessRun {
        elapsed: Duration::from_millis(millis),
        peak_memory,
        succeeded: true,
    }
}

#[test]
fn the_faster_run_is_kept_with_the_larger_peak_memory() {
    assert_eq!(faster(run(20, Some(5)), run(10, Some(3))), run(10, Some(5)));
    assert_eq!(faster(run(10, Some(3)), run(20, Some(5))), run(10, Some(5)));
}

#[test]
fn a_tier_report_line_names_the_size_rates_and_peak_memory() {
    assert_eq!(
        tier_report_line(&SIZE_TIERS[0], 2_000_000, &run(2000, Some(24_300_000))),
        "  small          10000 lines       2.0 MB in     2.000 s         5000 lines/sec      1.0 MB/sec   24.3 MB peak memory"
    );
}

#[test]
fn a_tier_report_line_of_a_run_too_short_to_time_reports_no_rate_or_memory() {
    assert_eq!(
        tier_report_line(&SIZE_TIERS[0], 2_000_000, &run(0, None)),
        "  small          10000 lines       2.0 MB in     0.000 s            0 lines/sec      0.0 MB/sec   unknown peak memory"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn peak_memory_is_reported_in_bytes_as_macos_gives_it() {
    assert_eq!(max_rss_bytes(4096), 4096);
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn peak_memory_is_reported_in_bytes_from_the_kilobytes_linux_gives() {
    assert_eq!(max_rss_bytes(4), 4096);
}

#[cfg(unix)]
#[test]
fn a_process_run_reports_its_success_and_peak_memory() {
    let finished = run_process(&which("true").unwrap(), &[]).unwrap();

    assert!(finished.succeeded);
    assert!(finished.peak_memory.unwrap() > 0);
}

#[cfg(unix)]
#[test]
fn a_process_that_fails_is_reported_as_failed() {
    assert!(
        !run_process(&which("false").unwrap(), &[])
            .unwrap()
            .succeeded
    );
}

#[test]
fn a_program_that_cannot_be_started_is_an_error() {
    assert!(run_process(std::path::Path::new("missing/program"), &[]).is_err());
}

#[cfg(unix)]
#[test]
fn the_fastest_run_of_several_is_kept() {
    let fastest = fastest_run(&which("true").unwrap(), &[], 3).unwrap();

    assert!(fastest.succeeded);
}

#[cfg(unix)]
#[test]
fn waiting_for_a_process_already_waited_for_is_an_error() {
    let mut child = std::process::Command::new(which("true").unwrap())
        .spawn()
        .unwrap();
    child.wait().unwrap();

    assert!(wait_with_peak_memory(child).is_err());
}
