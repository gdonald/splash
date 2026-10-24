//! Times splash against itself at different settings, and against ccze when
//! ccze is installed, then times the splash binary on log files of each size
//! asked for and on an empty input.
//!
//! Run with `cargo bench`. `SPLASH_BENCH_SIZES=small,medium,large,huge`
//! chooses the file sizes, which default to small and medium.
use splash::bench::{
    fastest_run, measure_ccze, measure_splash, report, sample_log, selected_tiers,
    tier_report_line, write_sample_log, Measurement,
};
use splash::default_jobs;
use splash::output::OutputMode;
use splash::theme::Theme;
use std::path::Path;

const LINES: usize = 20_000;
const RUNS: usize = 5;
const STARTUP_RUNS: usize = 20;

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
    time_files();
    time_startup();
}

fn print_report(measurements: &[Measurement]) {
    print!(
        "{}",
        report(&format!("Rendering {} log lines", LINES), measurements)
    );
}

fn binary() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_splash"))
}

fn time_files() {
    let sizes = std::env::var("SPLASH_BENCH_SIZES").ok();
    let tiers = selected_tiers(sizes.as_deref()).unwrap_or_else(|error| panic!("{}", error));
    let directory = tempfile::TempDir::new().unwrap();

    println!("\nRendering log files with the splash binary (clf, ansi, output discarded)");

    for tier in tiers {
        let path = directory.path().join(format!("{}.log", tier.name));
        let bytes = write_sample_log(&path, tier.lines).unwrap();
        let runs = if tier.lines > 1_000_000 { 1 } else { RUNS };
        let run = fastest_run(
            binary(),
            &[
                "--mode",
                "clf",
                "--output",
                "ansi",
                "--path",
                path.to_str().unwrap(),
            ],
            runs,
        )
        .unwrap();

        println!("{}", tier_report_line(&tier, bytes, &run));
    }
}

fn time_startup() {
    let run = fastest_run(
        binary(),
        &["--mode", "clf", "--output", "plain"],
        STARTUP_RUNS,
    )
    .unwrap();

    println!(
        "\nStarting up and exiting on an empty input: {:.2} ms, {:.1} MB peak memory",
        run.elapsed.as_secs_f64() * 1000.0,
        run.peak_memory.unwrap_or_default() as f64 / 1_000_000.0
    );
}
