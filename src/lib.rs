pub mod bench;
pub mod color;
pub mod config;
pub mod discovery;
pub mod output;
pub mod parser;
pub mod plugin;
pub mod registry;
pub mod source;
pub mod theme;
pub mod toml;
pub mod tui;
pub mod viewer;

use crate::config::{Config, Profiles, Settings};
use crate::discovery::PluginDiscovery;
use crate::output::OutputMode;
use crate::registry::PluginRegistry;
use crate::source::LogFile;
use crate::theme::Theme;
use std::io;
use std::path::Path;
use std::thread;

/// Renders a chunk of log text, producing one output line per parsed line.
///
/// Lines the mode has nothing to say about, such as blank lines or non-CLF
/// lines in `clf` mode, are dropped.
pub fn render_contents(
    contents: &str,
    mode: &str,
    output_mode: OutputMode,
    theme: &Theme,
) -> String {
    let mut rendered = String::new();

    for line in render_lines(contents, mode, output_mode, theme) {
        rendered.push_str(&line);
        rendered.push('\n');
    }

    rendered
}

/// Renders a chunk of log text as one string per parsed line.
pub fn render_lines(
    contents: &str,
    mode: &str,
    output_mode: OutputMode,
    theme: &Theme,
) -> Vec<String> {
    contents
        .lines()
        .filter_map(|line| parser::parse_line(line, mode))
        .map(|parsed| output_mode.render(&parsed, theme))
        .collect()
}

/// Inputs shorter than this are rendered on the calling thread, where the
/// work does not pay for handing lines to other threads.
pub const PARALLEL_THRESHOLD: usize = 512;

/// The number of worker threads splash uses when none is configured.
pub fn default_jobs() -> usize {
    thread::available_parallelism()
        .map(|jobs| jobs.get())
        .unwrap_or(1)
}

/// Renders a chunk of log text across `jobs` threads, in input order.
///
/// Short inputs, and a `jobs` of one, are rendered on the calling thread.
pub fn render_lines_with_jobs(
    contents: &str,
    mode: &str,
    output_mode: OutputMode,
    theme: &Theme,
    jobs: usize,
) -> Vec<String> {
    let lines: Vec<&str> = contents.lines().collect();

    if jobs <= 1 || lines.len() < PARALLEL_THRESHOLD {
        return render_slice(&lines, mode, output_mode, theme);
    }

    let per_thread = lines.len().div_ceil(jobs);

    thread::scope(|scope| {
        let workers: Vec<thread::ScopedJoinHandle<'_, Vec<String>>> = lines
            .chunks(per_thread)
            .map(|chunk| scope.spawn(move || render_slice(chunk, mode, output_mode, theme)))
            .collect();

        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect()
    })
}

/// Renders a chunk of log text across `jobs` threads, one output line per
/// parsed line.
pub fn render_contents_with_jobs(
    contents: &str,
    mode: &str,
    output_mode: OutputMode,
    theme: &Theme,
    jobs: usize,
) -> String {
    let mut rendered = String::new();

    for line in render_lines_with_jobs(contents, mode, output_mode, theme, jobs) {
        rendered.push_str(&line);
        rendered.push('\n');
    }

    rendered
}

/// Reads a log file and renders all of it, memory mapping the file when it is
/// large and rendering across the configured workers.
pub fn render_file(path: &Path, settings: &Settings) -> io::Result<String> {
    let log = LogFile::open(path)?;

    Ok(render_contents_with_jobs(
        log.text(),
        &settings.mode,
        settings.output_mode,
        &settings.theme,
        settings.jobs,
    ))
}

fn render_slice(lines: &[&str], mode: &str, output_mode: OutputMode, theme: &Theme) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| parser::parse_line(line, mode))
        .map(|parsed| output_mode.render(&parsed, theme))
        .collect()
}

/// Describes the registered plugins and where splash looks for more of them.
pub fn plugin_summary(
    registry: &PluginRegistry,
    discovery: &PluginDiscovery,
    config: &Config,
) -> String {
    let mut summary = String::from("Available Plugins:\n==================\n");

    match registry.describe_plugins() {
        Ok(plugins) if plugins.is_empty() => {
            summary.push_str("No plugins currently registered.\n");
            summary.push_str("\nBuilt-in modes:\n");
            summary.push_str("  - clf (Common Log Format)\n");
            summary.push_str("  - ad-hoc (General pattern matching)\n");
        }
        Ok(plugins) => {
            for description in plugins {
                summary.push_str(&format!("  {}\n", description));
            }
        }
        Err(e) => summary.push_str(&format!("Error listing plugins: {}\n", e)),
    }

    summary.push_str("\nPlugin discovery paths:\n");
    for path in discovery.search_paths() {
        summary.push_str(&format!("  {}\n", path.display()));
    }

    summary.push_str(&plugin_configuration(config));

    summary
}

/// Describes the per-plugin settings a config file supplies.
fn plugin_configuration(config: &Config) -> String {
    let plugins = config.configured_plugins();

    if plugins.is_empty() {
        return String::new();
    }

    let mut section = String::from("\nPlugin configuration:\n");

    for name in plugins {
        let state = if config.plugin_enabled(name) {
            "enabled"
        } else {
            "disabled"
        };

        section.push_str(&format!("  {} ({})\n", name, state));

        for (key, value) in config.plugin_settings(name).unwrap() {
            section.push_str(&format!("    {} = {}\n", key, value));
        }
    }

    section
}

/// Lists the saved color profiles and where they are stored.
pub fn profile_summary(profiles: &Profiles) -> String {
    let mut summary = String::from("Saved Color Profiles:\n=====================\n");

    let names = profiles.list();

    if names.is_empty() {
        summary.push_str("No color profiles saved.\n");
    } else {
        for name in names {
            summary.push_str(&format!("  {}\n", name));
        }
    }

    summary.push_str(&format!(
        "\nProfile directory:\n  {}\n",
        profiles.root().display()
    ));

    summary
}

/// Lists the themes splash ships with.
pub fn theme_summary() -> String {
    let mut summary = String::from("Available Themes:\n=================\n");

    for name in theme::PRESETS {
        summary.push_str(&format!("  {}\n", name));
    }

    summary
}
