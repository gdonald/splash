pub mod alert;
pub mod anonymize;
pub mod apache_error;
pub mod apm;
pub mod auth;
pub mod bench;
pub mod caddy;
pub mod ci;
pub mod cloud_init;
pub mod cloudwatch;
pub mod color;
pub mod completions;
pub mod config;
pub mod container;
pub mod correlate;
pub mod cron;
pub mod dates;
pub mod detect;
pub mod diff;
pub mod discovery;
pub mod distcc;
pub mod dmesg;
pub mod docker;
pub mod dovecot;
pub mod elasticsearch;
pub mod exim;
pub mod external;
pub mod fetchmail;
pub mod filter;
pub mod forward;
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
pub mod learn;
pub mod listen;
pub mod logfmt;
pub mod lookups;
pub mod mail;
pub mod manual;
pub mod merge;
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
pub mod rate;
pub mod redis;
pub mod registry;
pub mod replay;
pub mod resolved;
pub mod source;
pub mod squid;
pub mod ssh;
pub mod stats;
pub mod sudo;
pub mod suggest;
pub mod sulog;
pub mod super_log;
pub mod syslog;
pub mod tail;
pub mod terraform;
pub mod theme;
pub mod time;
pub mod toml;
pub mod tui;
pub mod ulogd;
pub mod varnish;
pub mod viewer;
pub mod vsftpd;
pub mod websocket;
pub mod xferlog;
pub mod zone;

use crate::anonymize::Anonymizer;
use crate::config::{Config, Profiles, Settings};
use crate::dates::DateRewrite;
use crate::discovery::PluginDiscovery;
use crate::filter::{Entry, Filter, Judgement};
use crate::keywords::Keywords;
use crate::lookups::Lookups;
use crate::output::{escape_html, OutputMode, ParsedLine, Token, TokenKind};
use crate::registry::PluginRegistry;
use crate::source::LogFile;
use crate::stats::{Facts, Stats, StatsSettings};
use crate::theme::Theme;
use crate::viewer::Line;
use std::collections::BTreeMap;
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
        lookups: &Lookups::none(),
        stats: &StatsSettings::default(),
        fields: &[],
        anonymizer: None,
        source: None,
        marker: None,
        plugin_themes: &BTreeMap::new(),
        line_numbers: false,
    };

    renderer
        .render(contents, jobs, &mut Entry::default(), &mut Stats::default())
        .into_iter()
        .map(|(_, text)| text)
        .collect()
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
    render_settings_counted(contents, settings, entry, &mut Stats::default())
}

/// Renders a chunk of log text with every setting, as one string per kept
/// line, counting the kept lines in `stats` when the settings ask for
/// statistics. No lines are rendered when a report replaces them.
pub fn render_settings_counted(
    contents: &str,
    settings: &Settings,
    entry: &mut Entry,
    stats: &mut Stats,
) -> Vec<String> {
    Renderer::for_settings(settings)
        .render(contents, settings.jobs, entry, stats)
        .into_iter()
        .map(|(_, text)| text)
        .collect()
}

/// Renders a chunk of log text with every setting, pairing each kept line's
/// rendering with the log text it came from
pub fn render_viewer_lines(contents: &str, settings: &Settings, entry: &mut Entry) -> Vec<Line> {
    Renderer::for_settings(settings)
        .render(contents, settings.jobs, entry, &mut Stats::default())
        .into_iter()
        .map(|(raw, text)| Line::new(raw, &text))
        .collect()
}

/// Renders a chunk of log text with every setting, one output line per kept
/// line
pub fn render_contents_settings(contents: &str, settings: &Settings, entry: &mut Entry) -> String {
    join_lines(render_settings(contents, settings, entry))
}

/// Renders a chunk of log text with every setting, one output line per kept
/// line, counting the kept lines in `stats`
pub fn render_contents_counted(
    contents: &str,
    settings: &Settings,
    entry: &mut Entry,
    stats: &mut Stats,
) -> String {
    join_lines(render_settings_counted(contents, settings, entry, stats))
}

/// The text written before the first line: the header of an output format
/// that needs one, unless a report replaces the lines
pub fn opening(settings: &Settings) -> String {
    match settings.output_mode.header_with_css(
        settings.theme_for(&settings.mode),
        &settings.fields,
        settings.css.as_deref(),
    ) {
        Some(header) if settings.stats.shows_lines() => header,
        _ => String::new(),
    }
}

/// The text written each time a chunk of input has been rendered: the
/// report that replaces the lines, if there is one
pub fn progress(settings: &Settings, stats: &Stats) -> String {
    settings.stats.report(stats).unwrap_or_default()
}

/// The text written when the input ends: the summary, if one was asked for,
/// and the footer of an output format that needs one
pub fn closing(settings: &Settings, stats: &Stats) -> String {
    let mut text = String::new();

    if let Some(summary) = settings.stats.summary(stats) {
        text.push_str(&match settings.output_mode {
            OutputMode::Html => escape_html(&summary),
            _ => summary,
        });
    }

    if settings.stats.shows_lines() {
        text.push_str(&settings.output_mode.footer().unwrap_or_default());
    }

    text
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
/// mode the start of the file is written in, reading only the lines
/// detection samples
pub fn resolve_file_settings(path: &Path, settings: &Settings) -> io::Result<Settings> {
    let file = io::BufReader::new(std::fs::File::open(path)?);
    let sample: String = source::lines(file)
        .take(detect::SAMPLE_SIZE)
        .map(|line| line + "\n")
        .collect();

    Ok(Settings {
        mode: detect::resolve_mode(&settings.mode, &sample),
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

/// A line's rendering and facts, when the line could be kept, and how the
/// filter judged it
type JudgedLine<'c> = (Judgement, Option<Kept<'c>>);

/// What a line that could be kept leaves for the output
struct Kept<'c> {
    /// The line as the log wrote it
    raw: &'c str,
    /// The rendered line, which is empty when a report replaces the lines
    text: String,
    /// What the statistics need from the line, when the run collects them
    facts: Option<Facts>,
}

/// Everything rendering a line takes besides the line
struct Renderer<'a> {
    mode: &'a str,
    output_mode: OutputMode,
    theme: &'a Theme,
    filter: &'a Filter,
    dates: &'a DateRewrite,
    keywords: &'a Keywords,
    lookups: &'a Lookups,
    stats: &'a StatsSettings,
    fields: &'a [String],
    anonymizer: Option<&'a Anonymizer>,
    source: Option<&'a str>,
    marker: Option<&'a (String, TokenKind)>,
    plugin_themes: &'a BTreeMap<String, Theme>,
    line_numbers: bool,
}

impl<'a> Renderer<'a> {
    fn for_settings(settings: &'a Settings) -> Renderer<'a> {
        Renderer {
            mode: &settings.mode,
            output_mode: settings.output_mode,
            theme: &settings.theme,
            filter: &settings.filter,
            dates: &settings.dates,
            keywords: &settings.keywords,
            lookups: &settings.lookups,
            stats: &settings.stats,
            fields: &settings.fields,
            anonymizer: settings.anonymizer.as_ref(),
            source: settings.source.as_deref(),
            marker: settings.marker.as_ref(),
            plugin_themes: &settings.plugin_themes,
            line_numbers: settings.line_numbers,
        }
    }

    /// Each kept line as the log wrote it, paired with its rendering
    fn render<'c>(
        &self,
        contents: &'c str,
        jobs: usize,
        entry: &mut Entry,
        stats: &mut Stats,
    ) -> Vec<(&'c str, String)> {
        let mode = detect::resolve_mode(self.mode, contents);
        let detected = detect::detected_mode(&mode).unwrap_or(&mode);
        let renderer = Renderer {
            mode: &mode,
            theme: self.plugin_themes.get(detected).unwrap_or(self.theme),
            ..*self
        };
        let lines: Vec<&str> = contents.lines().collect();
        let first_number = entry.number_lines(lines.len());

        let judged = if jobs <= 1 || lines.len() < PARALLEL_THRESHOLD {
            renderer.render_slice(&lines, first_number)
        } else {
            renderer.render_parallel(&lines, jobs, first_number)
        };

        let mut rendered = Vec::new();

        for (judgement, kept) in judged {
            let admitted = entry.admit(judgement);
            let Some(kept) = kept.filter(|_| admitted) else {
                continue;
            };

            if let Some(facts) = kept.facts {
                stats.record(facts);
            }

            if self.stats.shows_lines() {
                rendered.push((kept.raw, kept.text));
            }
        }

        rendered
    }

    /// The line after its marker and the name of the file it came from,
    /// when lines of several files are merged
    fn label<'c>(&self, mut parsed: ParsedLine<'c>) -> ParsedLine<'c> {
        if let Some(source) = self.source {
            parsed.tokens.splice(
                0..0,
                [
                    Token::owned(source.to_string(), TokenKind::Source),
                    Token::owned(" ".to_string(), TokenKind::Plain),
                ],
            );
        }

        if let Some((marker, kind)) = self.marker {
            parsed.tokens.splice(
                0..0,
                [
                    Token::owned(marker.clone(), *kind),
                    Token::owned(" ".to_string(), TokenKind::Plain),
                ],
            );
        }

        parsed
    }

    fn render_parallel<'c>(
        &self,
        lines: &[&'c str],
        jobs: usize,
        first_number: usize,
    ) -> Vec<JudgedLine<'c>> {
        let per_thread = lines.len().div_ceil(jobs);

        thread::scope(|scope| {
            let workers: Vec<thread::ScopedJoinHandle<'_, Vec<JudgedLine<'c>>>> = lines
                .chunks(per_thread)
                .enumerate()
                .map(|(index, chunk)| {
                    let chunk_number = first_number + index * per_thread;

                    scope.spawn(move || self.render_slice(chunk, chunk_number))
                })
                .collect();

            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect()
        })
    }

    /// Parses, judges, and renders each line. A line that cannot be kept
    /// whatever entry it belongs to is not rendered.
    fn render_slice<'c>(&self, lines: &[&'c str], first_number: usize) -> Vec<JudgedLine<'c>> {
        lines
            .iter()
            .zip(first_number..)
            .filter_map(|(raw, number)| {
                parser::parse_line(raw, self.mode).map(|parsed| (*raw, number, parsed))
            })
            .map(|(raw, number, parsed)| match self.anonymizer {
                Some(anonymizer) => (raw, number, anonymizer.apply(parsed)),
                None => (raw, number, parsed),
            })
            .map(|(raw, number, parsed)| {
                let judgement = self.filter.judge(&parsed);
                let kept = (judgement.matches && judgement.entry != Some(false)).then(|| {
                    let facts = self.stats.collects().then(|| self.stats.facts(&parsed));

                    let text = if self.stats.shows_lines() {
                        let rewritten = self
                            .keywords
                            .apply(self.lookups.apply(self.dates.apply(parsed)));

                        let html = self.output_mode.render_with_fields(
                            &self.label(self.filter.highlight(rewritten)),
                            self.theme,
                            self.fields,
                        );

                        match self.line_numbers {
                            true => output::number_html_line(number, &html),
                            false => html,
                        }
                    } else {
                        String::new()
                    };

                    Kept { raw, text, facts }
                });

                (judgement, kept)
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
