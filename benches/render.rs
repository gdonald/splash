//! Times splash against itself at different settings, and against ccze when
//! ccze is installed.
//!
//! Run with `cargo bench`.
use splash::bench::{measure_ccze, measure_splash, report, sample_log, Measurement};
use splash::default_jobs;
use splash::output::OutputMode;
use splash::theme::Theme;

const LINES: usize = 20_000;
const RUNS: usize = 5;

fn main() {
    let sample = sample_log(LINES);
    let theme = Theme::default();
    let jobs = default_jobs();

    let mut measurements = vec![
        measure_splash(
            "splash clf ansi",
            &sample,
            "clf",
            OutputMode::Ansi,
            &theme,
            1,
            RUNS,
        ),
        measure_splash(
            &format!("splash clf ansi -j {}", jobs),
            &sample,
            "clf",
            OutputMode::Ansi,
            &theme,
            jobs,
            RUNS,
        ),
        measure_splash(
            "splash clf plain",
            &sample,
            "clf",
            OutputMode::Plain,
            &theme,
            1,
            RUNS,
        ),
        measure_splash(
            "splash ad-hoc ansi",
            &sample,
            "ad-hoc",
            OutputMode::Ansi,
            &theme,
            1,
            RUNS,
        ),
    ];

    match measure_ccze(&sample, RUNS) {
        Some(measurement) => measurements.push(measurement),
        None => println!("ccze is not installed, timing splash only"),
    }

    print_report(&measurements);
}

fn print_report(measurements: &[Measurement]) {
    print!(
        "{}",
        report(&format!("Rendering {} log lines", LINES), measurements)
    );
}
