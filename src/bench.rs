//! Benchmark helpers
//!
//! The `render` benchmark target uses these to time splash against itself at
//! different settings, and against ccze when ccze is installed.
use crate::output::OutputMode;
use crate::render_lines_with_jobs;
use crate::theme::Theme;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// One timed run: how long a renderer took over a known number of lines
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
    pub name: String,
    pub lines: usize,
    pub elapsed: Duration,
}

impl Measurement {
    pub fn new(name: &str, lines: usize, elapsed: Duration) -> Self {
        Self {
            name: name.to_string(),
            lines,
            elapsed,
        }
    }

    /// Lines rendered per second, or zero when the run was too short to time
    pub fn lines_per_second(&self) -> f64 {
        let seconds = self.elapsed.as_secs_f64();

        if seconds <= 0.0 {
            return 0.0;
        }

        self.lines as f64 / seconds
    }

    /// The measurement as one report line
    pub fn report_line(&self, baseline: &Measurement) -> String {
        format!(
            "  {:<28} {:>8} lines in {:>9.3} ms  {:>12.0} lines/sec  {:>6.2}x",
            self.name,
            self.lines,
            self.elapsed.as_secs_f64() * 1000.0,
            self.lines_per_second(),
            self.speed_relative_to(baseline)
        )
    }

    /// How many times faster this run was than `baseline`
    pub fn speed_relative_to(&self, baseline: &Measurement) -> f64 {
        let reference = baseline.lines_per_second();

        if reference <= 0.0 {
            return 0.0;
        }

        self.lines_per_second() / reference
    }
}

/// Times `work` over `runs` passes and keeps the fastest, which is the pass
/// least disturbed by other load on the machine
pub fn measure<F>(name: &str, runs: usize, mut work: F) -> Measurement
where
    F: FnMut() -> usize,
{
    let mut best: Option<Measurement> = None;

    for _ in 0..runs.max(1) {
        let started = Instant::now();
        let lines = work();
        let measurement = Measurement::new(name, lines, started.elapsed());

        if best
            .as_ref()
            .is_none_or(|previous| measurement.elapsed < previous.elapsed)
        {
            best = Some(measurement);
        }
    }

    best.unwrap()
}

/// Times splash rendering `contents` with the given settings
pub fn measure_splash(
    name: &str,
    contents: &str,
    mode: &str,
    output_mode: OutputMode,
    theme: &Theme,
    jobs: usize,
    runs: usize,
) -> Measurement {
    measure(name, runs, || {
        render_lines_with_jobs(contents, mode, output_mode, theme, jobs).len()
    })
}

/// Times another colorizer reading `contents` on its standard input, or
/// reports nothing when the program is not installed
pub fn measure_command(
    name: &str,
    program: &str,
    args: &[&str],
    contents: &str,
    runs: usize,
) -> Option<Measurement> {
    which(program)?;

    let lines = contents.lines().count();
    let mut ran = true;

    let measurement = measure(name, runs, || {
        ran &= run_with_input(program, args, contents);
        lines
    });

    ran.then_some(measurement)
}

/// Times ccze over `contents`, or reports nothing when ccze is not installed
pub fn measure_ccze(contents: &str, runs: usize) -> Option<Measurement> {
    measure_command("ccze -A", "ccze", &["-A"], contents, runs)
}

/// Where a program is found, either at the path given or on the PATH
pub fn which(program: &str) -> Option<PathBuf> {
    let named = PathBuf::from(program);

    if named.components().count() > 1 {
        return named.is_file().then_some(named);
    }

    let path = std::env::var_os("PATH").unwrap_or_default();

    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| candidate.is_file())
}

/// A Common Log Format sample of `lines` lines, for timing a known workload
pub fn sample_log(lines: usize) -> String {
    let requests = ["GET /index.html", "POST /login", "GET /assets/app.css"];
    let statuses = [200, 302, 404, 500];
    let mut sample = String::new();

    for line in 0..lines {
        sample.push_str(&format!(
            "10.0.{}.{} - user{} [10/Oct/2000:13:55:36 -0700] \"{} HTTP/1.0\" {} {}\n",
            line / 256 % 256,
            line % 256,
            line % 97,
            requests[line % requests.len()],
            statuses[line % statuses.len()],
            512 + line % 4096
        ));
    }

    sample
}

/// The measurements as a report, with each rate compared to the first
pub fn report(title: &str, measurements: &[Measurement]) -> String {
    let mut report = format!("{}\n", title);

    let baseline = match measurements.first() {
        Some(baseline) => baseline,
        None => {
            report.push_str("  no measurements\n");
            return report;
        }
    };

    for measurement in measurements {
        report.push_str(&measurement.report_line(baseline));
        report.push('\n');
    }

    report
}

/// Runs a program over `contents`, reporting whether it ran at all
fn run_with_input(program: &str, args: &[&str], contents: &str) -> bool {
    let mut child = match Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(contents.as_bytes());
    }

    child.wait().is_ok()
}
