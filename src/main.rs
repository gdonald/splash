use clap::{ArgGroup, CommandFactory, Parser};
use crossterm::event;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute, terminal};
use notify::{Config as WatchConfig, RecommendedWatcher, RecursiveMode, Watcher};
use splash::alert::{Notifier, Patterns, Targets};
use splash::cloudwatch::{self, Connection, Query};
use splash::color;
use splash::completions::{self, Completed, Shell, Values};
use splash::config::{Config, Options, Profiles, Settings};
use splash::correlate;
use splash::dates::DateOptions;
use splash::detect;
use splash::diff;
use splash::discovery::PluginDiscovery;
use splash::external::{self, PluginSet, TomlPlugin};
use splash::filter::{Entry, FilterOptions};
use splash::forward::{Destination, Forwarder};
use splash::learn;
use splash::listen::{self, Received};
use splash::manual::{self, ManOption};
use splash::merge::{self, Log};
use splash::output::{OutputMode, ParsedLine, Token, TokenKind, OUTPUT_NAMES};
use splash::rate::RateLimit;
use splash::registry;
use splash::replay;
use splash::source::{self, Encoding, Follower, LogFile};
use splash::stats::{self, Stats, StatsOptions};
use splash::syslog;
use splash::tail::Tail;
use splash::time;
use splash::tui;
use splash::websocket::{self, Broadcaster};
use splash::zone::Zone;
use splash::{
    closing, opening, plugin_summary, profile_summary, progress, render_contents_counted,
    resolve_file_settings, theme_summary,
};
use std::borrow::Cow;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// How often a followed file checks whether splash was interrupted
const INTERRUPT_POLL: Duration = Duration::from_millis(100);

/// How long the viewer waits for a key before checking the log for new lines
const VIEWER_POLL: Duration = Duration::from_millis(250);

/// The `--path` that names standard input
const STDIN_PATH: &str = "-";

/// Explains why the curses viewer cannot be combined with --websocket
const WEBSOCKET_NEEDS_STDOUT: &str =
    "--websocket sends the output splash writes, and the curses viewer writes none";

/// Explains why the curses viewer cannot be combined with --rate-limit
const RATE_LIMIT_NEEDS_STDOUT: &str = "--rate-limit cannot be combined with the curses viewer";

/// The limit on input lines per second, when --rate-limit is given
static RATE_LIMIT: OnceLock<Mutex<RateLimit>> = OnceLock::new();

/// Explains why the curses viewer cannot be combined with --correlate
const CORRELATE_NEEDS_STDOUT: &str = "--correlate cannot be combined with the curses viewer";

/// Explains why the curses viewer cannot be combined with --diff
const DIFF_NEEDS_STDOUT: &str = "--diff cannot be combined with the curses viewer";

/// Explains why the curses viewer cannot be combined with --replay
const REPLAY_NEEDS_STDOUT: &str = "--replay cannot be combined with the curses viewer";

/// Explains why --diff needs two files
const DIFF_NEEDS_TWO_FILES: &str = "--diff compares two files; give --path two files";

/// Explains why the curses viewer cannot be given several files
const VIEWER_SHOWS_ONE_FILE: &str = "The curses viewer shows one file; give --path one file";

/// Explains why the curses viewer cannot be combined with --forward
const FORWARD_NEEDS_STDOUT: &str =
    "--forward sends the output splash writes, and the curses viewer writes none";

/// Explains why the curses viewer cannot be combined with --alert
const ALERT_NEEDS_STDOUT: &str = "--alert cannot be combined with the curses viewer";

/// The patterns that raise an alert and the thread sending alerts, when
/// --alert is given
static ALERTS: OnceLock<(Patterns, Notifier)> = OnceLock::new();

/// The destination output is also sent to, when --forward is given
static FORWARDER: OnceLock<Forwarder> = OnceLock::new();

/// The WebSocket clients output is also sent to, when --websocket is given
static BROADCASTER: OnceLock<Arc<Broadcaster>> = OnceLock::new();

/// Set when splash is asked to stop while following a file or reading stdin
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Where splash reads log lines from
enum Input<'a> {
    Stdin,
    Files {
        sources: Sources,
        options: FileOptions,
    },
    Network(&'a str),
    Syslog(&'a str),
    CloudWatch(Query),
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(
    after_help = "--help lists every option.",
    after_long_help = "--man writes the manual page."
)]
#[command(group(ArgGroup::new("followed").args(["path", "cloudwatch"]).multiple(true)))]
#[command(group(ArgGroup::new("alert_targets").args(["alert_webhook", "alert_desktop"]).multiple(true)))]
struct Args {
    /// Log Parsing Mode (apache-error, apm, auth, caddy, ci, clf, cloud-init, cron, distcc, dmesg,
    /// docker, dovecot, elasticsearch, exim, fetchmail, ftpstats, git, haproxy, httpd, icecast,
    /// journalctl (or systemd), kubernetes (or k8s), mongodb, mysql, nginx-error, oops, php,
    /// postfix, postgresql, procmail, proftpd, pure-ftpd, redis, squid, ssh, sudo, sulog, super,
    /// syslog, systemd-resolved, terraform, ulogd, varnish, vsftpd, xferlog, ad-hoc, auto)
    #[arg(short, long, help_heading = "Input")]
    mode: Option<String>,

    /// Detect the log format from the first lines of the input
    #[arg(long, conflicts_with = "mode", help_heading = "Input")]
    auto: bool,

    /// Paths to the log files, or - to read stdin. A file name may hold * and ?
    #[arg(short, long, num_args = 1.., help_heading = "Input")]
    path: Vec<String>,

    /// Keep reading the --path files or the --cloudwatch log group as they grow, as tail -f does
    #[arg(short, long, requires = "followed", help_heading = "Input")]
    follow: bool,

    /// Interleave the lines of several --path files in time order, labeled with their file
    #[arg(
        long,
        requires = "path",
        help_heading = "Input",
        hide_short_help = true
    )]
    merge: bool,

    /// Start each line with where it came from: its file, the address that sent it, or its log stream
    #[arg(long, help_heading = "Input", hide_short_help = true)]
    label: bool,

    /// Read the files in the subdirectories of a --path directory too
    #[arg(
        short,
        long,
        requires = "path",
        help_heading = "Input",
        hide_short_help = true
    )]
    recursive: bool,

    /// Receive log lines over TCP or HTTP on this address, such as :8080 or 127.0.0.1:8080
    #[arg(
        long,
        value_name = "ADDRESS",
        conflicts_with = "path",
        help_heading = "Input",
        hide_short_help = true
    )]
    listen: Option<String>,

    /// Receive syslog messages over UDP and TCP on this address (default :514)
    #[arg(long, value_name = "ADDRESS", num_args = 0..=1,
          default_missing_value = listen::SYSLOG_ADDRESS, conflicts_with_all = ["path", "listen"], help_heading = "Input", hide_short_help = true)]
    syslog: Option<String>,

    /// Read the events of this CloudWatch Logs log group, with the AWS CLI's region and credentials
    #[arg(long, value_name = "GROUP", conflicts_with_all = ["path", "listen", "syslog"], help_heading = "Input", hide_short_help = true)]
    cloudwatch: Option<String>,

    /// Keep at most this many input lines each second, dropping the rest
    #[arg(long, value_name = "LINES", value_parser = clap::value_parser!(u64).range(1..), help_heading = "Input", hide_short_help = true)]
    rate_limit: Option<u64>,

    /// Read the input, and write ansi and plain output, in this encoding, utf-8 or latin1 (default: the locale's)
    #[arg(
        long,
        value_name = "ENCODING",
        help_heading = "Input",
        hide_short_help = true
    )]
    encoding: Option<String>,

    /// Keep only lines at these levels, as LEVEL,LEVEL, or drop a level written !LEVEL (repeatable)
    #[arg(long, value_name = "LEVELS", help_heading = "Filters")]
    level: Vec<String>,

    /// Keep only lines written at or after this time, such as 2h ago or 2024-01-01
    #[arg(long, value_name = "TIME", help_heading = "Filters")]
    since: Option<String>,

    /// Keep only lines written at or before this time
    #[arg(long, value_name = "TIME", help_heading = "Filters")]
    until: Option<String>,

    /// Keep only lines matching this pattern, and highlight the match (repeatable)
    #[arg(long, value_name = "PATTERN", help_heading = "Filters")]
    grep: Vec<String>,

    /// Drop lines matching this pattern (repeatable)
    #[arg(long, value_name = "PATTERN", help_heading = "Filters")]
    exclude: Vec<String>,

    /// Keep only lines holding a field with this value, as KEY=VALUE, or not, as KEY!=VALUE (repeatable)
    #[arg(
        long,
        value_name = "KEY=VALUE",
        help_heading = "Filters",
        hide_short_help = true
    )]
    filter: Vec<String>,

    /// Mask IP addresses, email addresses, and user names
    #[arg(long, help_heading = "Filters", hide_short_help = true)]
    anonymize: bool,

    /// Also send each line of output to WebSocket clients on this address, such as :8081
    #[arg(
        long,
        value_name = "ADDRESS",
        help_heading = "Output",
        hide_short_help = true
    )]
    websocket: Option<String>,

    /// Also send each line of output to tcp://HOST:PORT, udp://HOST:PORT, or http://HOST:PORT/PATH
    #[arg(
        long,
        value_name = "URL",
        help_heading = "Output",
        hide_short_help = true
    )]
    forward: Option<String>,

    /// Output format (ansi, csv, curses, html, json, jsonl, plain)
    #[arg(short, long, help_heading = "Output")]
    output: Option<String>,

    /// A style sheet to add to HTML output after the theme's rules
    #[arg(
        long,
        value_name = "FILE",
        help_heading = "Output",
        hide_short_help = true
    )]
    css: Option<String>,

    /// Start each line of HTML output with its line number in the input
    #[arg(long, help_heading = "Output", hide_short_help = true)]
    line_numbers: bool,

    /// Fields csv and jsonl output write, as FIELD,FIELD (repeatable)
    #[arg(
        long,
        value_name = "FIELDS",
        help_heading = "Output",
        hide_short_help = true
    )]
    fields: Vec<String>,

    /// Worker threads used to render a file (default: one per core)
    #[arg(short, long, help_heading = "Output", hide_short_help = true)]
    jobs: Option<usize>,

    /// Color theme (dark, light, solarized, dracula, colorblind, high-contrast)
    #[arg(long, help_heading = "Colors")]
    theme: Option<String>,

    /// Override one token color, as KEY=COLOR (repeatable)
    #[arg(
        long,
        value_name = "KEY=COLOR",
        help_heading = "Colors",
        hide_short_help = true
    )]
    color: Vec<String>,

    /// Load a saved color profile
    #[arg(long, help_heading = "Colors", hide_short_help = true)]
    profile: Option<String>,

    /// Save the resolved colors as a profile and exit
    #[arg(
        long,
        value_name = "NAME",
        help_heading = "Colors",
        hide_short_help = true
    )]
    save_profile: Option<String>,

    /// List saved color profiles
    #[arg(long, help_heading = "Colors", hide_short_help = true)]
    list_profiles: bool,

    /// List available color themes
    #[arg(long, help_heading = "Colors", hide_short_help = true)]
    list_themes: bool,

    /// Leave keywords such as ERROR, TCP, and SELECT in the style around them
    #[arg(long, help_heading = "Colors", hide_short_help = true)]
    no_wordcolor: bool,

    /// Mark unknown users, and follow ports and addresses with their services, hosts, and locations
    #[arg(
        long,
        conflicts_with = "no_lookups",
        help_heading = "Colors",
        hide_short_help = true
    )]
    lookups: bool,

    /// Turn lookups off, even when the config file turns them on
    #[arg(long, help_heading = "Colors", hide_short_help = true)]
    no_lookups: bool,

    /// Rewrite timestamps written as seconds since the epoch as dates
    #[arg(long, help_heading = "Timestamps", hide_short_help = true)]
    convert_date: bool,

    /// Rewrite every timestamp as RFC 3339
    #[arg(long, help_heading = "Timestamps", hide_short_help = true)]
    normalize_dates: bool,

    /// Rewrite every timestamp in this zone, such as UTC or America/New_York
    #[arg(
        long,
        value_name = "ZONE",
        help_heading = "Timestamps",
        hide_short_help = true
    )]
    tz: Option<String>,

    /// Rewrite every timestamp as its distance from now, such as 2 hours ago
    #[arg(long, help_heading = "Timestamps", hide_short_help = true)]
    relative: bool,

    /// Write statistics on the kept lines in place of the lines
    #[arg(
        long,
        conflicts_with = "histogram",
        help_heading = "Reports",
        hide_short_help = true
    )]
    stats: bool,

    /// Write the line count, error count, and time range after the lines
    #[arg(long, help_heading = "Reports", hide_short_help = true)]
    summary: bool,

    /// Write the number of lines in each period, such as 5m or 1h, in place of the lines
    #[arg(long, value_name = "PERIOD", num_args = 0..=1, default_missing_value = stats::DEFAULT_BUCKET, help_heading = "Reports", hide_short_help = true)]
    histogram: Option<String>,

    /// Write sparklines of the lines and errors in each period, such as 5m or 1h, in place of the lines
    #[arg(long, value_name = "PERIOD", num_args = 0..=1, default_missing_value = stats::DEFAULT_BUCKET,
          conflicts_with_all = ["stats", "histogram"], help_heading = "Reports", hide_short_help = true)]
    graph: Option<String>,

    /// Also draw the average of this field's numbers in each --graph period, such as duration
    #[arg(
        long,
        value_name = "FIELD",
        requires = "graph",
        help_heading = "Reports",
        hide_short_help = true
    )]
    graph_field: Option<String>,

    /// Format of --stats, --summary, --histogram, and --graph (text, json, csv)
    #[arg(
        long,
        value_name = "FORMAT",
        help_heading = "Reports",
        hide_short_help = true
    )]
    stats_format: Option<String>,

    /// Write the lines that share a request, trace, or correlation ID together, under the ID
    #[arg(long, conflicts_with_all = ["follow", "listen", "syslog", "cloudwatch"], help_heading = "Analysis", hide_short_help = true)]
    correlate: bool,

    /// Compare two --path files line by line, as diff -u does, leaving timestamps and IDs out
    #[arg(long, requires = "path", conflicts_with_all = ["follow", "merge", "correlate"], help_heading = "Analysis", hide_short_help = true)]
    diff: bool,

    /// Write the lines at the pace their timestamps give, SPEED times as fast (default 1)
    #[arg(long, value_name = "SPEED", num_args = 0..=1, default_missing_value = "1",
          value_parser = replay::parse_speed,
          conflicts_with_all = ["follow", "correlate", "diff", "listen", "syslog", "cloudwatch"], help_heading = "Analysis", hide_short_help = true)]
    replay: Option<f64>,

    /// Send an alert for each input line matching this pattern (repeatable)
    #[arg(
        long,
        value_name = "PATTERN",
        requires = "alert_targets",
        help_heading = "Analysis",
        hide_short_help = true
    )]
    alert: Vec<String>,

    /// Post each alert as JSON to this http://HOST:PORT/PATH URL
    #[arg(
        long,
        value_name = "URL",
        requires = "alert",
        help_heading = "Analysis",
        hide_short_help = true
    )]
    alert_webhook: Option<String>,

    /// Show each alert as a desktop notification
    #[arg(
        long,
        requires = "alert",
        help_heading = "Analysis",
        hide_short_help = true
    )]
    alert_desktop: bool,

    /// List all available plugins
    #[arg(long, help_heading = "Plugins")]
    list_plugins: bool,

    /// Also load plugin files from this directory (repeatable)
    #[arg(
        long,
        value_name = "DIR",
        help_heading = "Plugins",
        hide_short_help = true
    )]
    plugin_dir: Vec<String>,

    /// Load plugin files again whenever one changes
    #[arg(long, help_heading = "Plugins", hide_short_help = true)]
    reload_plugins: bool,

    /// Report which of a plugin file's patterns read each input line, and exit
    #[arg(
        long,
        value_name = "FILE",
        help_heading = "Plugins",
        hide_short_help = true
    )]
    test_regex: Option<String>,

    /// Write a plugin file named NAME whose patterns read the input lines, and exit
    #[arg(
        long,
        value_name = "NAME",
        help_heading = "Plugins",
        hide_short_help = true
    )]
    learn: Option<String>,

    /// Read the input with this plugin, as --mode does
    #[arg(
        long,
        value_name = "NAME",
        conflicts_with = "mode",
        help_heading = "Plugins",
        hide_short_help = true
    )]
    plugin: Option<String>,

    /// Leave a plugin out of auto detection and refuse its mode (repeatable)
    #[arg(
        long,
        value_name = "NAME",
        help_heading = "Plugins",
        hide_short_help = true
    )]
    disable_plugin: Vec<String>,

    /// Config file to read instead of ~/.splash/config.toml or ~/.splashrc
    #[arg(long, help_heading = "Setup", hide_short_help = true)]
    config: Option<String>,

    /// Write a completion script for this shell (bash, zsh, fish), and exit
    #[arg(
        long,
        value_name = "SHELL",
        help_heading = "Setup",
        hide_short_help = true
    )]
    completions: Option<String>,

    /// Write the manual page in roff, and exit
    #[arg(long, help_heading = "Setup", hide_short_help = true)]
    man: bool,
}

fn main() {
    stop_quietly_on_a_closed_pipe();

    let args = Args::parse();
    let term = std::env::var("TERM").ok();
    let variable = |name: &str| std::env::var(name).ok();

    source::set_encoding(match args.encoding.as_deref() {
        Some(name) => Encoding::parse(name).unwrap_or_else(|e| fail(&e.to_string())),
        None => Encoding::from_locale(
            variable("LC_ALL").as_deref(),
            variable("LC_CTYPE").as_deref(),
            variable("LANG").as_deref(),
        ),
    });

    if args.man {
        let today = time::format_rfc3339(time::now(), None, &Zone::Utc);

        print!(
            "{}",
            manual::man_page(env!("CARGO_PKG_VERSION"), &today[..10], &man_options())
        );
        return;
    }

    if let Some(name) = args.completions.as_deref() {
        let shell = Shell::parse(name).unwrap_or_else(|e| fail(&e.to_string()));

        print!("{}", completions::script(shell, &completed_options()));
        return;
    }

    if !color::terminal_shows_colors(
        term.as_deref(),
        std::env::var("CLICOLOR_FORCE").ok().as_deref(),
    ) {
        colored::control::set_override(false);
    }

    let config = match load_config(args.config.as_deref()) {
        Ok(config) => config,
        Err(e) => fail(&e.to_string()),
    };

    let mut discovery = PluginDiscovery::new();

    for directory in &args.plugin_dir {
        discovery.add_path(directory);
    }

    let mut plugins = PluginSet::new();

    for failure in plugins.load(&discovery.discover_plugins().unwrap_or_default()) {
        eprintln!("{}", failure);
    }

    disable_plugins(&config, &args.disable_plugin);

    if let Some(file) = args.test_regex.as_deref() {
        std::process::exit(test_regex(file, &args.path));
    }

    if let Some(name) = args.learn.as_deref() {
        let text: String = input_texts(&args.path)
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join("\n");

        match learn::learn(name, &text) {
            Ok(file) => print!("{}", file),
            Err(e) => fail(&e.to_string()),
        }

        return;
    }

    let profiles = Profiles::default_profiles();

    if args.list_themes {
        print!("{}", theme_summary());
        return;
    }

    if args.list_profiles {
        print!("{}", profile_summary(&profiles));
        return;
    }

    if args.list_plugins {
        print!(
            "{}",
            plugin_summary(registry::builtins(), &discovery, &config)
        );
        return;
    }

    let mode = match (args.auto, args.mode.or(args.plugin), args.syslog.is_some()) {
        (true, _, _) => Some(detect::AUTO.to_string()),
        (false, Some(mode), _) => Some(mode),
        (false, None, true) => Some(syslog::NAME.to_string()),
        (false, None, false) => None,
    };

    let options = Options {
        mode,
        output: args.output,
        theme: args.theme,
        profile: args.profile,
        colors: args.color,
        jobs: args.jobs,
        filter: FilterOptions {
            levels: args.level,
            since: args.since,
            until: args.until,
            grep: args.grep,
            exclude: args.exclude,
            fields: args.filter,
        },
        dates: DateOptions {
            convert_epoch: args.convert_date,
            normalize: args.normalize_dates,
            tz: args.tz,
            relative: args.relative,
        },
        no_wordcolor: args.no_wordcolor,
        lookups: match (args.lookups, args.no_lookups) {
            (true, _) => Some(true),
            (_, true) => Some(false),
            _ => None,
        },
        fields: args.fields,
        anonymize: args.anonymize,
        css: args.css.map(PathBuf::from),
        line_numbers: args.line_numbers,
        stats: StatsOptions {
            stats: args.stats,
            summary: args.summary,
            histogram: args.histogram,
            graph: args.graph,
            graph_field: args.graph_field,
            format: args.stats_format,
        },
    };

    let settings = match Settings::resolve(&options, &config, &profiles) {
        Ok(settings) => settings,
        Err(e) => fail(&e.to_string()),
    };

    source::set_output_encoding(match settings.output_mode {
        OutputMode::Ansi | OutputMode::Plain => source::encoding(),
        _ => Encoding::Utf8,
    });

    if let Some(name) = args.save_profile {
        match profiles.save(&name, &settings.theme) {
            Ok(path) => println!("Saved color profile '{}' to {}", name, path.display()),
            Err(e) => fail(&e.to_string()),
        }

        return;
    }

    if registry::builtins().is_disabled(&settings.mode) {
        fail(&format!("Plugin '{}' is disabled", settings.mode));
    }

    let input = match (
        args.cloudwatch.as_deref(),
        args.syslog.as_deref(),
        args.listen.as_deref(),
    ) {
        (Some(group), _, _) => Input::CloudWatch(
            Query::new(group)
                .within(settings.filter.time.as_ref())
                .following(args.follow),
        ),
        (None, Some(address), _) => Input::Syslog(address),
        (None, None, Some(address)) => Input::Network(address),
        _ if args.path.is_empty() || args.path == [STDIN_PATH] => Input::Stdin,
        _ => match expand_paths(&args.path, args.recursive) {
            Ok(sources) => Input::Files {
                sources,
                options: FileOptions {
                    follow: args.follow,
                    merge: args.merge,
                    label: args.label,
                    recursive: args.recursive,
                },
            },
            Err(e) => fail(&e.to_string()),
        },
    };

    if let Some(per_second) = args.rate_limit {
        if settings.output_mode.is_interactive() {
            fail(RATE_LIMIT_NEEDS_STDOUT);
        }

        let _ = RATE_LIMIT.set(Mutex::new(RateLimit::new(per_second as usize)));
    }

    if let Some(url) = args.forward.as_deref() {
        if settings.output_mode.is_interactive() {
            fail(FORWARD_NEEDS_STDOUT);
        }

        match Destination::parse(url) {
            Ok(destination) => {
                let forwarder = Forwarder::start(destination, |failure| eprintln!("{}", failure));
                let _ = FORWARDER.set(forwarder);
            }
            Err(e) => fail(&e.to_string()),
        }
    }

    if let Some(address) = args.websocket.as_deref() {
        if settings.output_mode.is_interactive() {
            fail(WEBSOCKET_NEEDS_STDOUT);
        }

        match websocket::start(address) {
            Ok((bound, broadcaster)) => {
                eprintln!("Sending WebSocket output on {}", bound);
                let _ = BROADCASTER.set(broadcaster);
            }
            Err(e) => fail(&format!("Cannot listen on {}: {}", address, e)),
        }
    }

    if !args.alert.is_empty() {
        if settings.output_mode.is_interactive() {
            fail(ALERT_NEEDS_STDOUT);
        }

        let patterns = Patterns::build(&args.alert).unwrap_or_else(|e| fail(&e.to_string()));
        let targets = Targets::new(args.alert_webhook.as_deref(), args.alert_desktop)
            .unwrap_or_else(|e| fail(&e.to_string()));
        let notifier = Notifier::start(targets, |failure| eprintln!("{}", failure));

        let _ = ALERTS.set((patterns, notifier));
    }

    let _plugin_watcher = args
        .reload_plugins
        .then(|| watch_plugins(discovery, plugins));

    if args.diff {
        if settings.output_mode.is_interactive() {
            fail(DIFF_NEEDS_STDOUT);
        }

        let files = match input {
            Input::Files { ref sources, .. } => sources.files.as_slice(),
            _ => &[],
        };

        if files.len() != 2 {
            fail(DIFF_NEEDS_TWO_FILES);
        }

        if let Err(e) = render_diff(&files[0], &files[1], &settings) {
            fail(&e.to_string());
        }
    } else if let Some(speed) = args.replay {
        if settings.output_mode.is_interactive() {
            fail(REPLAY_NEEDS_STDOUT);
        }

        if let Err(e) = render_replayed(files_of(&input), speed, &settings) {
            fail(&e.to_string());
        }
    } else if args.correlate {
        if settings.output_mode.is_interactive() {
            fail(CORRELATE_NEEDS_STDOUT);
        }

        if let Err(e) = render_correlated(files_of(&input), &settings) {
            fail(&e.to_string());
        }
    } else {
        run(&input, &settings, args.label);
    }

    if let Some(forwarder) = FORWARDER.get() {
        forwarder.finish();
    }

    if let Some((_, notifier)) = ALERTS.get() {
        notifier.finish();
    }
}

fn run(input: &Input, settings: &Settings, label: bool) {
    let mode = settings.mode.as_str();
    let output_mode = settings.output_mode;

    if output_mode.is_interactive() {
        if let Err(e) = view(input, settings) {
            fail(&e.to_string());
        }

        return;
    }

    match *input {
        Input::Files {
            ref sources,
            options,
        } => {
            if let Err(e) = render_paths(sources, options, settings) {
                fail(&format!("{:?}", e));
            }
        }
        Input::Network(address) => match listen_on(address) {
            Ok(lines) => render_received(lines, label, settings),
            Err(e) => fail(&e.to_string()),
        },
        Input::Syslog(address) => match receive_syslog_on(address) {
            Ok(lines) => render_received(lines, label, settings),
            Err(e) => fail(&e.to_string()),
        },
        Input::CloudWatch(ref query) => {
            let (lines, reader) = cloudwatch::start(Connection::Environment, query.clone());

            render_received(lines, label, settings);

            // After an interrupt the reader can still be waiting to ask for
            // newer events, and joining it would wait with no end
            let interrupted = INTERRUPTED.load(Ordering::SeqCst);

            if let Some(Err(e)) = (!interrupted).then(|| reader.join().unwrap()) {
                fail(&e);
            }
        }
        Input::Stdin => {
            catch_interrupts();

            emit(&opening(settings));

            let mut lines = source::lines(io::stdin().lock());
            let mut settings = settings.clone();
            let mut entry = Entry::default();
            let mut stats = Stats::default();

            if mode == detect::AUTO {
                let sample: String = lines
                    .by_ref()
                    .take(detect::SAMPLE_SIZE)
                    .map(|line| line + "\n")
                    .collect();

                settings.mode = detect::resolve_mode(mode, &sample);
                let kept = admitted(&sample);

                emit(&render_contents_counted(
                    &kept, &settings, &mut entry, &mut stats,
                ));
            }

            for line in lines {
                let line = line + "\n";
                let kept = admitted(&line);

                emit(&render_contents_counted(
                    &kept, &settings, &mut entry, &mut stats,
                ));
            }

            emit(&progress(&settings, &stats));
            emit_closing(&settings, &stats);
        }
    }
}

/// Each option with a long name, and what follows it on the command line
fn completed_options() -> Vec<Completed> {
    let mut command = Args::command();
    command.build();

    let words = |list: &[&str]| Values::Words(list.iter().map(|word| word.to_string()).collect());

    command
        .get_arguments()
        .filter_map(|arg| {
            let optional = arg
                .get_num_args()
                .is_some_and(|range| range.min_values() == 0);

            let values = match arg.get_id().as_str() {
                _ if !arg.get_action().takes_values() || optional => Values::Nothing,
                "mode" | "plugin" | "disable_plugin" => Values::Words(registry::mode_names()),
                "output" => words(&OUTPUT_NAMES),
                "theme" => words(&splash::theme::PRESETS),
                "encoding" => words(&["utf-8", "latin1"]),
                "stats_format" => words(&["text", "json", "csv"]),
                "completions" => words(&["bash", "zsh", "fish"]),
                "path" | "config" | "css" | "test_regex" => Values::Files,
                "plugin_dir" => Values::Directories,
                _ => Values::Any,
            };

            Some(Completed {
                long: arg.get_long()?.to_string(),
                short: arg.get_short(),
                help: arg.get_help().map(ToString::to_string).unwrap_or_default(),
                values,
            })
        })
        .collect()
}

/// Each option as the manual lists it
fn man_options() -> Vec<ManOption> {
    let mut command = Args::command();
    command.build();

    command
        .get_arguments()
        .filter_map(|arg| {
            let long = arg.get_long()?;
            let names = match arg.get_short() {
                Some(short) => format!("-{}, --{}", short, long),
                None => format!("--{}", long),
            };
            let range = arg.get_num_args();
            let value = arg
                .get_value_names()
                .filter(|_| arg.get_action().takes_values())
                .map(|names| match range {
                    Some(range) if range.min_values() == 0 => format!("[{}]", names[0]),
                    Some(range) if range.max_values() > 1 => format!("{}...", names[0]),
                    _ => names[0].to_string(),
                });

            Some(ManOption {
                names,
                value,
                help: arg.get_help().map(ToString::to_string).unwrap_or_default(),
            })
        })
        .collect()
}

fn load_config(path: Option<&str>) -> Result<Config, splash::config::ConfigError> {
    match path {
        Some(path) => Config::load(&PathBuf::from(path)),
        None => Config::load_default(),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("Error: {}", message);
    std::process::exit(1);
}

/// How the `--path` files are read
#[derive(Debug, Clone, Copy)]
struct FileOptions {
    follow: bool,
    merge: bool,
    label: bool,
    recursive: bool,
}

/// The files the `--path` values name, and the directories among them,
/// whose files are read too
#[derive(Debug, Default)]
struct Sources {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
}

/// Reloads plugin files as they change, reporting each reload on stderr,
/// until the returned watcher is dropped
fn watch_plugins(discovery: PluginDiscovery, plugins: PluginSet) -> RecommendedWatcher {
    external::watch(discovery, plugins, |report| eprintln!("{}", report))
        .unwrap_or_else(|e| fail(&format!("Cannot watch plugin files: {}", e)))
}

/// Reports which of a plugin file's patterns read each line of stdin or of
/// the `--path` files, returning the exit status: 1 when a line was not read
/// Disables the plugins the config turns off, and those named by
/// `--disable-plugin`, which must be loaded
fn disable_plugins(config: &Config, names: &[String]) {
    for name in config.disabled_plugins() {
        let _ = registry::builtins().disable_plugin(registry::canonical_mode(name));
    }

    for name in names {
        if let Err(e) = registry::builtins().disable_plugin(registry::canonical_mode(name)) {
            fail(&e.to_string());
        }
    }
}

fn test_regex(file: &str, values: &[String]) -> i32 {
    let plugin = TomlPlugin::load(Path::new(file))
        .and_then(|plugin| plugin.resolve(registry::builtins()))
        .unwrap_or_else(|e| fail(&format!("Cannot load plugin {}: {}", file, e)));

    let texts = input_texts(values);
    let several = texts.len() > 1;
    let mut status = 0;

    for (path, text) in texts {
        if let Some(path) = path.filter(|_| several) {
            println!("==> {} <==", path.display());
        }

        let report = external::test_patterns(&plugin, &text);
        print!("{}", report.text);

        if report.unmatched > 0 {
            status = 1;
        }
    }

    status
}

/// The text of stdin, or of each file the `--path` values name, with its path
fn input_texts(values: &[String]) -> Vec<(Option<PathBuf>, String)> {
    let mut texts = Vec::new();

    if values.is_empty() || values == [STDIN_PATH] {
        let mut bytes = Vec::new();
        let _ = io::stdin().read_to_end(&mut bytes);
        texts.push((None, source::decode(&bytes).into_owned()));
    } else {
        let sources = expand_paths(values, false).unwrap_or_else(|e| fail(&e.to_string()));

        for path in sources.files {
            let log = LogFile::open(&path).unwrap_or_else(|e| fail(&e.to_string()));
            texts.push((Some(path), log.text().to_string()));
        }
    }

    texts
}

/// The files and directories the `--path` values name, with their patterns
/// expanded and each directory's files listed
fn expand_paths(values: &[String], recursive: bool) -> io::Result<Sources> {
    let mut sources = Sources::default();

    for value in values {
        for path in source::expand(value)? {
            let found = if path.is_dir() {
                let found = source::files_in(&path, recursive)?;
                sources.directories.push(path);
                found
            } else {
                vec![path]
            };

            for file in found {
                if !sources.files.contains(&file) {
                    sources.files.push(file);
                }
            }
        }
    }

    Ok(sources)
}

/// A file splash renders, with the settings its own lines decide in auto
/// mode and the entry its last line belonged to
struct FileInput {
    path: PathBuf,
    settings: Settings,
    entry: Entry,
    follower: Follower,
    /// The text after the last line break, rendered once its line is complete
    pending: Vec<u8>,
}

/// A file to render, read from its beginning
fn file_input(path: &Path, options: FileOptions, settings: &Settings) -> io::Result<FileInput> {
    let settings = Settings {
        source: (options.merge || options.label).then(|| source_label(path)),
        ..resolve_file_settings(path, settings)?
    };

    Ok(FileInput {
        path: path.to_path_buf(),
        settings,
        entry: Entry::default(),
        follower: Follower::new(path.to_path_buf(), 0),
        pending: Vec::new(),
    })
}

/// The `==> PATH <==` header written before lines from one of several
/// files, as tail writes it
struct Headers {
    several: bool,
    last_shown: Option<usize>,
}

impl Headers {
    /// Writes the header for a file when there are several files and the
    /// last lines written came from another one
    fn show(&mut self, index: usize, path: &Path, settings: &Settings) {
        if !self.several || self.last_shown == Some(index) {
            return;
        }

        self.last_shown = Some(index);

        emit_header(&format!("==> {} <==", path.display()), settings);
    }
}

/// Writes a line of text styled as a header
fn emit_header(text: &str, settings: &Settings) {
    let header = Token::owned(text.to_string(), TokenKind::Header);
    let line = ParsedLine::new(vec![header]);

    emit(&format!(
        "{}\n",
        settings.output_mode.render(&line, &settings.theme)
    ));
}

/// Renders log files one after another, or merged in time order, then,
/// when following, the lines they gain until splash is interrupted
fn render_paths(
    sources: &Sources,
    options: FileOptions,
    settings: &Settings,
) -> notify::Result<()> {
    let FileOptions {
        follow,
        merge,
        recursive,
        ..
    } = options;
    let mut files = Vec::new();

    for path in &sources.files {
        files.push(file_input(path, options, settings)?);
    }

    let mut stats = Stats::default();
    let mut headers = Headers {
        several: (files.len() > 1 || !sources.directories.is_empty()) && !merge,
        last_shown: None,
    };

    let (tx, rx) = mpsc::channel();
    let mut watcher = None;

    if follow {
        catch_interrupts();

        let config = WatchConfig::default()
            .with_poll_interval(Duration::from_secs(2))
            .with_compare_contents(true);
        let watching = watcher.insert(RecommendedWatcher::new(tx, config)?);

        // Watching each file's directory, not the file, sees the new file a
        // rotation puts in its place, which a watch on the old file misses
        let mut parents: Vec<&Path> = sources
            .files
            .iter()
            .map(|path| {
                path.parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
            })
            .map(|parent| parent.unwrap_or(Path::new(".")))
            .collect();
        parents.sort();
        parents.dedup();

        for parent in parents {
            watching.watch(parent, RecursiveMode::NonRecursive)?;
        }

        let depth = match recursive {
            true => RecursiveMode::Recursive,
            false => RecursiveMode::NonRecursive,
        };

        for directory in &sources.directories {
            watching.watch(directory, depth)?;
        }
    }

    emit(&opening(settings));

    if merge {
        let mut logs = Vec::new();

        for file in &files {
            logs.push(LogFile::open(&file.path)?);
        }

        let complete: Vec<&str> = logs
            .iter()
            .map(|log| match follow {
                true => &log.text()[..log.text().rfind('\n').map_or(0, |end| end + 1)],
                false => log.text(),
            })
            .collect();
        let modes: Vec<String> = files
            .iter()
            .map(|file| file.settings.mode.clone())
            .collect();
        let merging: Vec<Log> = complete
            .iter()
            .zip(&modes)
            .map(|(text, mode)| Log { text, mode })
            .collect();

        for entry in merge::merge(&merging, &Zone::local(), time::now()) {
            let file = &mut files[entry.log];

            write_lines(entry.text, &file.settings, &mut file.entry, &mut stats);
        }

        emit(&progress(settings, &stats));

        for (index, file) in files.iter_mut().enumerate() {
            file.follower = Follower::new(file.path.clone(), fs::metadata(&file.path)?.len());
            file.pending = logs[index].text().as_bytes()[complete[index].len()..].to_vec();
        }
    } else {
        for (index, file) in files.iter_mut().enumerate() {
            headers.show(index, &file.path, settings);

            let (mut rest, read) = source::read_chunks(
                &file.path,
                source::CHUNK_BYTES,
                source::encoding(),
                |chunk| write_lines(chunk, &file.settings, &mut file.entry, &mut stats),
            )?;

            if let Some(lines) = source::take_complete_lines(&mut rest) {
                write_lines(&lines, &file.settings, &mut file.entry, &mut stats);
            }

            match follow {
                true => {
                    file.follower = Follower::new(file.path.clone(), read);
                    file.pending = rest;
                }
                false => write_lines(
                    &source::decode(&rest),
                    &file.settings,
                    &mut file.entry,
                    &mut stats,
                ),
            }

            emit(&progress(settings, &stats));
        }
    }

    while follow && !INTERRUPTED.load(Ordering::SeqCst) {
        // The watcher holds the sender, so receiving can only time out
        match rx.recv_timeout(INTERRUPT_POLL) {
            // inotify reports the open a read makes, which must not trigger another read
            Ok(Ok(event)) if event.kind.is_access() => {}
            Ok(_) => {
                for directory in &sources.directories {
                    for path in source::files_in(directory, recursive).unwrap_or_default() {
                        if !files.iter().any(|file| file.path == path) {
                            files.push(file_input(&path, options, settings)?);
                        }
                    }
                }

                for file in &mut files {
                    file.follower.read(&mut file.pending)?;
                }
            }
            Err(_) => {}
        }

        for (index, file) in files.iter_mut().enumerate() {
            if let Some(lines) = source::take_complete_lines(&mut file.pending) {
                headers.show(index, &file.path, settings);
                write_rendered(&lines, &file.settings, &mut file.entry, &mut stats);
            }
        }
    }

    for (index, file) in files.iter_mut().enumerate() {
        if !file.pending.is_empty() {
            let rest = source::decode(&file.pending).into_owned();

            headers.show(index, &file.path, settings);
            write_rendered(&rest, &file.settings, &mut file.entry, &mut stats);
        }
    }

    emit_closing(settings, &stats);

    Ok(())
}

/// Renders the lines received over the network or from CloudWatch until
/// splash is interrupted or the lines stop arriving. Lines that arrive
/// together are rendered together, and in auto mode the first of them decide
/// the mode.
fn render_received(lines: mpsc::Receiver<Received>, label: bool, settings: &Settings) {
    let mut settings = settings.clone();
    let mut entry = Entry::default();
    let mut stats = Stats::default();

    catch_interrupts();

    emit(&opening(&settings));

    while !INTERRUPTED.load(Ordering::SeqCst) {
        let first = match lines.recv_timeout(INTERRUPT_POLL) {
            Ok(first) => first,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };

        let arrived: Vec<Received> = std::iter::once(first).chain(lines.try_iter()).collect();

        for group in arrived.chunk_by(|one, next| one.sender == next.sender) {
            let text: String = group
                .iter()
                .map(|received| format!("{}\n", received.line))
                .collect();

            settings.mode = detect::resolve_mode(&settings.mode, &text);

            let labeled = Settings {
                source: label.then(|| group[0].sender.clone()),
                ..settings.clone()
            };

            write_lines(&text, &labeled, &mut entry, &mut stats);
        }

        emit(&progress(&settings, &stats));
    }

    emit_closing(&settings, &stats);
}

/// The `--path` files, or none when splash reads stdin
fn files_of<'a>(input: &'a Input) -> Option<&'a Sources> {
    match input {
        Input::Files { sources, .. } => Some(sources),
        _ => None,
    }
}

/// The text of stdin, or of each `--path` file, with the settings it is
/// rendered with. The lines of several files are labeled with their file.
fn read_logs(
    sources: Option<&Sources>,
    settings: &Settings,
) -> io::Result<Vec<(Settings, String)>> {
    let mut logs: Vec<(Settings, String)> = Vec::new();

    match sources {
        Some(sources) => {
            for path in &sources.files {
                let settings = Settings {
                    source: (sources.files.len() > 1).then(|| source_label(path)),
                    ..resolve_file_settings(path, settings)?
                };

                logs.push((settings, LogFile::open(path)?.text().to_string()));
            }
        }
        None => {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes)?;

            let text = source::decode(&bytes).into_owned();
            let settings = Settings {
                mode: detect::resolve_mode(&settings.mode, &text),
                ..settings.clone()
            };

            logs.push((settings, text));
        }
    }

    Ok(logs)
}

/// Renders the entries of stdin, or of the `--path` files merged in time
/// order, that share an identifier, each group under a header naming it
fn render_correlated(sources: Option<&Sources>, settings: &Settings) -> io::Result<()> {
    let logs = read_logs(sources, settings)?;
    let merging: Vec<Log> = logs
        .iter()
        .map(|(settings, text)| Log {
            text,
            mode: &settings.mode,
        })
        .collect();
    let entries = merge::merge(&merging, &Zone::local(), time::now());
    let texts: Vec<&str> = entries.iter().map(|entry| entry.text).collect();

    let mut filters: Vec<Entry> = logs.iter().map(|_| Entry::default()).collect();
    let mut stats = Stats::default();

    emit(&opening(settings));

    for group in correlate::groups(&texts) {
        emit_header(&format!("==> {} <==", group.id), settings);

        for member in group.members {
            let entry = &entries[member];

            write_lines(
                entry.text,
                &logs[entry.log].0,
                &mut filters[entry.log],
                &mut stats,
            );
        }
    }

    emit(&progress(settings, &stats));
    emit_closing(settings, &stats);

    Ok(())
}

/// Renders the entries of stdin, or of the `--path` files merged in time
/// order, each after waiting for the time between its timestamp and the
/// last one, divided by `speed`, until splash is interrupted
fn render_replayed(sources: Option<&Sources>, speed: f64, settings: &Settings) -> io::Result<()> {
    let logs = read_logs(sources, settings)?;
    let merging: Vec<Log> = logs
        .iter()
        .map(|(settings, text)| Log {
            text,
            mode: &settings.mode,
        })
        .collect();
    let timed = merge::merge_timed(&merging, &Zone::local(), time::now());
    let moments: Vec<Option<merge::Moment>> = timed.iter().map(|(moment, _)| *moment).collect();

    let mut filters: Vec<Entry> = logs.iter().map(|_| Entry::default()).collect();
    let mut stats = Stats::default();

    catch_interrupts();

    emit(&opening(settings));

    for ((_, entry), delay) in timed.iter().zip(replay::delays(&moments, speed)) {
        if !wait(delay) {
            break;
        }

        write_lines(
            entry.text,
            &logs[entry.log].0,
            &mut filters[entry.log],
            &mut stats,
        );
    }

    emit(&progress(settings, &stats));
    emit_closing(settings, &stats);

    Ok(())
}

/// Waits for `delay`, returning false when splash is interrupted first
fn wait(delay: Duration) -> bool {
    let deadline = Instant::now() + delay;

    while !INTERRUPTED.load(Ordering::SeqCst) {
        let left = deadline.saturating_duration_since(Instant::now());

        if left.is_zero() {
            return true;
        }

        std::thread::sleep(left.min(INTERRUPT_POLL));
    }

    false
}

/// Renders the hunks of lines that differ between two files, each line
/// marked `-` when only the old file holds it, `+` when only the new file
/// does, and with a space when both do
fn render_diff(old_path: &Path, new_path: &Path, settings: &Settings) -> io::Result<()> {
    let marked = |path: &Path, marker: &str, kind: TokenKind| -> io::Result<Settings> {
        Ok(Settings {
            marker: Some((marker.to_string(), kind)),
            ..resolve_file_settings(path, settings)?
        })
    };
    let kept = marked(old_path, " ", TokenKind::Plain)?;
    let removed = marked(old_path, "-", TokenKind::Failure)?;
    let added = marked(new_path, "+", TokenKind::Success)?;

    let old_log = LogFile::open(old_path)?;
    let new_log = LogFile::open(new_path)?;
    let old: Vec<&str> = old_log.text().lines().collect();
    let new: Vec<&str> = new_log.text().lines().collect();
    let hunks = diff::hunks(&diff::changes(&old, &new), diff::CONTEXT);

    let mut entry = Entry::default();
    let mut stats = Stats::default();

    emit(&opening(settings));

    if !hunks.is_empty() {
        emit_header(&format!("--- {}", old_path.display()), settings);
        emit_header(&format!("+++ {}", new_path.display()), settings);
    }

    for hunk in hunks {
        emit_header(&hunk.header(), settings);

        for change in hunk.changes {
            let (line, marked) = match change {
                diff::Change::Same { old: index, .. } => (old[index], &kept),
                diff::Change::Removed { old: index } => (old[index], &removed),
                diff::Change::Added { new: index } => (new[index], &added),
            };

            write_lines(&format!("{}\n", line), marked, &mut entry, &mut stats);
        }
    }

    emit(&progress(settings, &stats));
    emit_closing(settings, &stats);

    Ok(())
}

/// Starts listening on `address`, and names the address bound on stderr,
/// which gives the port chosen for port 0
fn listen_on(address: &str) -> io::Result<mpsc::Receiver<Received>> {
    let (bound, lines) = listen::start(address)
        .map_err(|e| io::Error::other(format!("Cannot listen on {}: {}", address, e)))?;

    eprintln!("Listening on {}", bound);

    Ok(lines)
}

/// Starts receiving syslog messages on `address`, and names the address
/// bound on stderr
fn receive_syslog_on(address: &str) -> io::Result<mpsc::Receiver<Received>> {
    let (bound, lines) = listen::start_syslog(address)
        .map_err(|e| io::Error::other(format!("Cannot listen on {}: {}", address, e)))?;

    eprintln!("Receiving syslog messages on {} (TCP and UDP)", bound);

    Ok(lines)
}

/// Writes the rendered text, followed by the report that replaces the lines
/// when there is one
fn write_rendered(text: &str, settings: &Settings, entry: &mut Entry, stats: &mut Stats) {
    write_lines(text, settings, entry, stats);
    emit(&progress(settings, stats));
}

/// Writes the rendered text alone
fn write_lines(text: &str, settings: &Settings, entry: &mut Entry, stats: &mut Stats) {
    let kept = admitted(text);

    emit(&render_contents_counted(&kept, settings, entry, stats));
}

/// The name a merged file's lines are labeled with: its file name
fn source_label(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

#[cfg(unix)]
extern "C" fn note_interrupt(_signal: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);

    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_DFL);
        libc::signal(libc::SIGTERM, libc::SIG_DFL);
    }
}

/// Has Ctrl-C or `kill` end the follow loop, so splash writes what it
/// writes when its input ends. Reading stdin, splash keeps reading until the
/// pipe closes, which it does when Ctrl-C also stops the program writing it.
/// The first interrupt restores the default action, so a second one stops
/// splash at once.
#[cfg(unix)]
fn catch_interrupts() {
    let handler: extern "C" fn(libc::c_int) = note_interrupt;

    unsafe {
        libc::signal(libc::SIGINT, handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, handler as libc::sighandler_t);
    }
}

#[cfg(not(unix))]
fn catch_interrupts() {}

/// The lines of `text` the rate limit keeps. When a window that has ended
/// dropped lines, the count is reported on stderr, keeping stdout in the
/// output format.
fn rate_limited(text: &str) -> Cow<'_, str> {
    let Some(limit) = RATE_LIMIT.get() else {
        return Cow::Borrowed(text);
    };

    let mut limit = limit.lock().unwrap();
    let admitted = limit.admit(text, Instant::now());

    if let Some(dropped) = admitted.dropped {
        eprintln!("{}", limit.notice(dropped));
    }

    Cow::Owned(admitted.text)
}

/// The lines of `text` the rate limit keeps, after queuing an alert for each
/// one that matches an alert pattern
fn admitted(text: &str) -> Cow<'_, str> {
    let kept = rate_limited(text);

    if let Some((patterns, notifier)) = ALERTS.get() {
        for alert in patterns.alerts(&kept) {
            notifier.send(alert);
        }
    }

    kept
}

/// Writes what splash writes when its input ends, after reporting the lines
/// the last window of the rate limit dropped
fn emit_closing(settings: &Settings, stats: &Stats) {
    if let Some(limit) = RATE_LIMIT.get() {
        let mut limit = limit.lock().unwrap();

        if let Some(dropped) = limit.finish() {
            eprintln!("{}", limit.notice(dropped));
        }
    }

    emit(&closing(settings, stats));
}

/// Writes output to stdout, to any WebSocket clients, and to the forwarding
/// destination
fn emit(text: &str) {
    if let Err(e) = io::stdout().write_all(&source::output_encoding().encode(text)) {
        fail(&e.to_string());
    }

    if let Some(broadcaster) = BROADCASTER.get() {
        broadcaster.send(text);
    }

    if let Some(forwarder) = FORWARDER.get() {
        forwarder.send(text);
    }
}

/// Rust programs ignore SIGPIPE, which makes `print!` panic once the program
/// reading splash's output, such as `head`, exits. With the default action,
/// splash stops silently, as `cat` does.
#[cfg(unix)]
fn stop_quietly_on_a_closed_pipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn stop_quietly_on_a_closed_pipe() {}

/// Reads the whole input and shows it in the scrollable viewer
fn view(input: &Input, settings: &Settings) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::other(tui::NEEDS_TERMINAL));
    }

    if std::env::var("TERM").is_ok_and(|term| term == color::DUMB_TERMINAL) {
        return Err(io::Error::other(tui::NEEDS_CURSOR));
    }

    let mut tail = match *input {
        Input::Files { ref sources, .. } if sources.files.len() != 1 => {
            return Err(io::Error::other(VIEWER_SHOWS_ONE_FILE))
        }
        Input::Files { ref sources, .. } => Tail::file(
            sources.files[0].clone(),
            resolve_file_settings(&sources.files[0], settings)?,
        ),
        Input::Network(address) => Tail::stream(lines_of(listen_on(address)?), settings.clone()),
        Input::Syslog(address) => {
            Tail::stream(lines_of(receive_syslog_on(address)?), settings.clone())
        }
        Input::CloudWatch(ref query) => Tail::stream(
            lines_of(cloudwatch::start(Connection::Environment, query.clone()).0),
            settings.clone(),
        ),
        Input::Stdin if io::stdin().is_terminal() => {
            return Err(io::Error::other(tui::NEEDS_INPUT))
        }
        Input::Stdin => Tail::stream(read_lines_in_background(), settings.clone()),
    };

    let (_, rows) = terminal::size()?;
    let mut viewer = tui::viewer_for(tail.read()?, rows);
    let mut out = io::stdout();

    enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, cursor::Hide)?;

    let result = tui::run_loop(
        &mut out,
        &mut viewer,
        &mut || match event::poll(VIEWER_POLL)? {
            true => event::read().map(Some),
            false => Ok(None),
        },
        &mut || tail.read(),
    );

    execute!(out, cursor::Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    result
}

/// The lines received over the network, without the address that sent
/// each one
fn lines_of(received: mpsc::Receiver<Received>) -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let _ = received
            .iter()
            .try_for_each(|received| sender.send(received.line));
    });

    receiver
}

/// Sends each line of standard input as it arrives, so the viewer can show
/// a pipe that is still being written
fn read_lines_in_background() -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let _ = source::lines(io::stdin().lock()).try_for_each(|line| sender.send(line));
    });

    receiver
}
