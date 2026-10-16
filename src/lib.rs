pub mod apache_error;
pub mod apm;
pub mod auth;
pub mod bench;
pub mod caddy;
pub mod ci;
pub mod cloud_init;
pub mod color;
pub mod config;
pub mod cron;
pub mod dates;
pub mod detect;
pub mod discovery;
pub mod distcc;
pub mod dmesg;
pub mod docker;
pub mod dovecot;
pub mod elasticsearch;
pub mod exim;
pub mod fetchmail;
pub mod filter;
pub mod ftp;
pub mod ftpstats;
pub mod git;
pub mod haproxy;
pub mod httpd;
pub mod icecast;
pub mod journalctl;
pub mod json;
pub mod keywords;
pub mod kubernetes;
pub mod logfmt;
pub mod mail;
pub mod mongodb;
pub mod mysql;
pub mod nginx_error;
pub mod oops;
pub mod output;
pub mod parser;
pub mod php;
pub mod plugin;
pub mod postfix;
pub mod postgresql;
pub mod procmail;
pub mod proftpd;
pub mod pureftpd;
pub mod redis;
pub mod registry;
pub mod resolved;
pub mod source;
pub mod squid;
pub mod ssh;
pub mod sudo;
pub mod sulog;
pub mod super_log;
pub mod syslog;
pub mod terraform;
pub mod theme;
pub mod time;
pub mod toml;
pub mod tui;
pub mod ulogd;
pub mod varnish;
pub mod viewer;
pub mod vsftpd;
pub mod xferlog;
pub mod zone;

use crate::config::{Config, Profiles, Settings};
use crate::dates::DateRewrite;
use crate::discovery::PluginDiscovery;
use crate::filter::{Entry, Filter, Judgement};
use crate::keywords::Keywords;
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
    join_lines(render_lines(contents, mode, output_mode, theme))
}

/// Renders a chunk of log text as one string per parsed line.
pub fn render_lines(
    contents: &str,
    mode: &str,
    output_mode: OutputMode,
    theme: &Theme,
) -> Vec<String> {
    render_lines_with_jobs(contents, mode, output_mode, theme, 1)
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
    let renderer = Renderer {
        mode,
        output_mode,
        theme,
        filter: &Filter::default(),
        dates: &DateRewrite::default(),
        keywords: &Keywords::none(),
    };

    renderer.render(contents, jobs, &mut Entry::default())
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
    join_lines(render_lines_with_jobs(
        contents,
        mode,
        output_mode,
        theme,
        jobs,
    ))
}

/// Renders a chunk of log text with every setting, filters included, as one
/// string per kept line. `entry` carries the entry the last line belonged to
/// over to the next chunk of the same input.
pub fn render_settings(contents: &str, settings: &Settings, entry: &mut Entry) -> Vec<String> {
    let renderer = Renderer {
        mode: &settings.mode,
        output_mode: settings.output_mode,
        theme: &settings.theme,
        filter: &settings.filter,
        dates: &settings.dates,
        keywords: &settings.keywords,
    };

    renderer.render(contents, settings.jobs, entry)
}

/// Renders a chunk of log text with every setting, one output line per kept
/// line
pub fn render_contents_settings(contents: &str, settings: &Settings, entry: &mut Entry) -> String {
    join_lines(render_settings(contents, settings, entry))
}

/// Reads a log file and renders all of it, memory mapping the file when it is
/// large and rendering across the configured workers.
pub fn render_file(path: &Path, settings: &Settings) -> io::Result<String> {
    let log = LogFile::open(path)?;

    Ok(render_contents_settings(
        log.text(),
        settings,
        &mut Entry::default(),
    ))
}

/// The settings to render a log file with, with `auto` mode resolved to the
/// mode the start of the file is written in
pub fn resolve_file_settings(path: &Path, settings: &Settings) -> io::Result<Settings> {
    let log = LogFile::open(path)?;

    Ok(Settings {
        mode: detect::resolve_mode(&settings.mode, log.text()),
        ..settings.clone()
    })
}

fn join_lines(lines: Vec<String>) -> String {
    let mut joined = String::new();

    for line in lines {
        joined.push_str(&line);
        joined.push('\n');
    }

    joined
}

/// A line's rendering, when the line could be kept, and how the filter
/// judged it
type JudgedLine = (Judgement, Option<String>);

/// Everything rendering a line takes besides the line
struct Renderer<'a> {
    mode: &'a str,
    output_mode: OutputMode,
    theme: &'a Theme,
    filter: &'a Filter,
    dates: &'a DateRewrite,
    keywords: &'a Keywords,
}

impl Renderer<'_> {
    fn render(&self, contents: &str, jobs: usize, entry: &mut Entry) -> Vec<String> {
        let mode = detect::resolve_mode(self.mode, contents);
        let renderer = Renderer {
            mode: &mode,
            ..*self
        };
        let lines: Vec<&str> = contents.lines().collect();

        let judged = if jobs <= 1 || lines.len() < PARALLEL_THRESHOLD {
            renderer.render_slice(&lines)
        } else {
            renderer.render_parallel(&lines, jobs)
        };

        judged
            .into_iter()
            .filter_map(|(judgement, rendered)| {
                let kept = entry.admit(judgement);
                rendered.filter(|_| kept)
            })
            .collect()
    }

    fn render_parallel(&self, lines: &[&str], jobs: usize) -> Vec<JudgedLine> {
        let per_thread = lines.len().div_ceil(jobs);

        thread::scope(|scope| {
            let workers: Vec<thread::ScopedJoinHandle<'_, Vec<JudgedLine>>> = lines
                .chunks(per_thread)
                .map(|chunk| scope.spawn(move || self.render_slice(chunk)))
                .collect();

            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect()
        })
    }

    /// Parses, judges, and renders each line. A line that cannot be kept
    /// whatever entry it belongs to is not rendered.
    fn render_slice(&self, lines: &[&str]) -> Vec<JudgedLine> {
        lines
            .iter()
            .filter_map(|line| parser::parse_line(line, self.mode))
            .map(|parsed| {
                let judgement = self.filter.judge(&parsed);
                let rendered = (judgement.matches && judgement.entry != Some(false)).then(|| {
                    let rewritten = self.keywords.apply(self.dates.apply(parsed));

                    self.output_mode
                        .render(&self.filter.highlight(rewritten), self.theme)
                });

                (judgement, rendered)
            })
            .collect()
    }
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
