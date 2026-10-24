//! Benchmark helpers
//!
//! The `render` benchmark target uses these to time splash against itself at
//! different settings, and against ccze when ccze is installed.
use crate::output::OutputMode;
use crate::render_lines_with_jobs;
use crate::theme::Theme;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
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
    (0..lines).map(sample_line).collect()
}

/// Line `number` of the sample log, with its line break
pub fn sample_line(number: usize) -> String {
    let requests = ["GET /index.html", "POST /login", "GET /assets/app.css"];
    let statuses = [200, 302, 404, 500];

    format!(
        "10.0.{}.{} - user{} [10/Oct/2000:13:55:36 -0700] \"{} HTTP/1.0\" {} {}\n",
        number / 256 % 256,
        number % 256,
        number % 97,
        requests[number % requests.len()],
        statuses[number % statuses.len()],
        512 + number % 4096
    )
}

/// Writes a sample log of `lines` lines to a file a line at a time, so a
/// sample larger than memory can be written
pub fn write_sample_log(path: &Path, lines: usize) -> io::Result<u64> {
    let mut file = BufWriter::new(File::create(path)?);
    let mut written = 0;

    for number in 0..lines {
        let line = sample_line(number);
        file.write_all(line.as_bytes())?;
        written += line.len() as u64;
    }

    file.flush()?;

    Ok(written)
}

/// A size of log file the benchmark times splash on
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeTier {
    pub name: &'static str,
    pub lines: usize,
}

/// The file sizes the benchmark can time: under 1 MB, 1 to 100 MB, 100 MB to
/// 1 GB, and over 1 GB. A sample line is about 85 bytes.
pub const SIZE_TIERS: [SizeTier; 4] = [
    SizeTier {
        name: "small",
        lines: 10_000,
    },
    SizeTier {
        name: "medium",
        lines: 600_000,
    },
    SizeTier {
        name: "large",
        lines: 2_500_000,
    },
    SizeTier {
        name: "huge",
        lines: 13_000_000,
    },
];

/// The tiers named in a comma-separated list such as `small,medium`, or the
/// small and medium tiers when no list is given. A name that is no tier is
/// an error naming it.
pub fn selected_tiers(names: Option<&str>) -> Result<Vec<SizeTier>, String> {
    names
        .unwrap_or("small,medium")
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            SIZE_TIERS
                .iter()
                .find(|tier| tier.name == name)
                .copied()
                .ok_or_else(|| {
                    format!(
                        "Unknown size '{}' (expected small, medium, large, or huge)",
                        name
                    )
                })
        })
        .collect()
}

/// One run of a program: how long it took and the most memory it held
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRun {
    pub elapsed: Duration,
    /// The process's peak resident memory in bytes, where the platform
    /// reports it
    pub peak_memory: Option<u64>,
    pub succeeded: bool,
}

/// Runs a program with its output discarded, timing it and reading its peak
/// memory
pub fn run_process(program: &Path, args: &[&str]) -> io::Result<ProcessRun> {
    let started = Instant::now();
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let (succeeded, peak_memory) = wait_with_peak_memory(child)?;

    Ok(ProcessRun {
        elapsed: started.elapsed(),
        peak_memory,
        succeeded,
    })
}

/// The fastest of `runs` runs of a program, which is the run least disturbed
/// by other load on the machine, with the largest peak memory of them
pub fn fastest_run(program: &Path, args: &[&str], runs: usize) -> io::Result<ProcessRun> {
    let mut fastest = run_process(program, args)?;

    for _ in 1..runs {
        fastest = faster(fastest, run_process(program, args)?);
    }

    Ok(fastest)
}

/// The faster of two runs, carrying the larger peak memory of the two
pub fn faster(best: ProcessRun, run: ProcessRun) -> ProcessRun {
    let peak_memory = best.peak_memory.max(run.peak_memory);

    match best.elapsed <= run.elapsed {
        true => ProcessRun {
            peak_memory,
            ..best
        },
        false => ProcessRun { peak_memory, ..run },
    }
}

/// Waits for a child process, returning whether it succeeded and its peak
/// memory in bytes
#[cfg(unix)]
pub fn wait_with_peak_memory(child: std::process::Child) -> io::Result<(bool, Option<u64>)> {
    let mut status = 0;
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let waited = unsafe { libc::wait4(child.id() as libc::pid_t, &mut status, 0, &mut usage) };

    if waited < 0 {
        return Err(io::Error::last_os_error());
    }

    let succeeded = libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0;

    Ok((succeeded, Some(max_rss_bytes(usage.ru_maxrss as u64))))
}

#[cfg(not(unix))]
pub fn wait_with_peak_memory(mut child: std::process::Child) -> io::Result<(bool, Option<u64>)> {
    Ok((child.wait()?.success(), None))
}

/// `ru_maxrss` in bytes: macOS reports bytes
#[cfg(target_os = "macos")]
pub fn max_rss_bytes(max_rss: u64) -> u64 {
    max_rss
}

/// `ru_maxrss` in bytes: Linux and the BSDs report kilobytes
#[cfg(not(target_os = "macos"))]
pub fn max_rss_bytes(max_rss: u64) -> u64 {
    max_rss * 1024
}

/// A file-size benchmark result as one report line
pub fn tier_report_line(tier: &SizeTier, bytes: u64, run: &ProcessRun) -> String {
    let seconds = run.elapsed.as_secs_f64();
    let rate = |count: f64| match seconds > 0.0 {
        true => count / seconds,
        false => 0.0,
    };

    format!(
        "  {:<8} {:>11} lines {:>9.1} MB in {:>9.3} s  {:>11.0} lines/sec  {:>7.1} MB/sec  {:>8} peak memory",
        tier.name,
        tier.lines,
        bytes as f64 / 1_000_000.0,
        seconds,
        rate(tier.lines as f64),
        rate(bytes as f64 / 1_000_000.0),
        run.peak_memory
            .map(|peak| format!("{:.1} MB", peak as f64 / 1_000_000.0))
            .unwrap_or_else(|| "unknown".to_string())
    )
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
