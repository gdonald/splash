use clap::Parser;
use crossterm::event;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute, terminal};
use notify::{Config as WatchConfig, RecommendedWatcher, RecursiveMode, Watcher};
use splash::config::{Config, Options, Profiles, Settings};
use splash::dates::DateOptions;
use splash::detect;
use splash::discovery::PluginDiscovery;
use splash::external::{self, PluginSet, TomlPlugin};
use splash::filter::{Entry, FilterOptions};
use splash::forward::{Destination, Forwarder};
use splash::listen::{self, Received};
use splash::merge::{self, Log};
use splash::output::{ParsedLine, Token, TokenKind};
use splash::rate::RateLimit;
use splash::registry;
use splash::source::{self, Follower, LogFile};
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
use std::io::{self, IsTerminal, Read};
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

/// Explains why the curses viewer cannot be given several files
const VIEWER_SHOWS_ONE_FILE: &str = "The curses viewer shows one file; give --path one file";

/// Explains why the curses viewer cannot be combined with --forward
const FORWARD_NEEDS_STDOUT: &str =
    "--forward sends the output splash writes, and the curses viewer writes none";

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
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log Parsing Mode (apache-error, apm, auth, caddy, ci, clf, cloud-init, cron, distcc, dmesg,
    /// docker, dovecot, elasticsearch, exim, fetchmail, ftpstats, git, haproxy, httpd, icecast,
    /// journalctl (or systemd), kubernetes (or k8s), mongodb, mysql, nginx-error, oops, php,
    /// postfix, postgresql, procmail, proftpd, pure-ftpd, redis, squid, ssh, sudo, sulog, super,
    /// syslog, systemd-resolved, terraform, ulogd, varnish, vsftpd, xferlog, ad-hoc, auto)
    #[arg(short, long)]
    mode: Option<String>,

    /// Detect the log format from the first lines of the input
    #[arg(long, conflicts_with = "mode")]
    auto: bool,

    /// Paths to the log files, or - to read stdin. A file name may hold * and ?
    #[arg(short, long, num_args = 1..)]
    path: Vec<String>,

    /// Keep reading the --path files as they grow, as tail -f does
    #[arg(short, long, requires = "path")]
    follow: bool,

    /// Interleave the lines of several --path files in time order, labeled with their file
    #[arg(long, requires = "path")]
    merge: bool,

    /// Start each line with where it came from: its file, or the address that sent it
    #[arg(long)]
    label: bool,

    /// Read the files in the subdirectories of a --path directory too
    #[arg(short, long, requires = "path")]
    recursive: bool,

    /// Receive log lines over TCP or HTTP on this address, such as :8080 or 127.0.0.1:8080
    #[arg(long, value_name = "ADDRESS", conflicts_with = "path")]
    listen: Option<String>,

    /// Receive syslog messages over UDP and TCP on this address (default :514)
    #[arg(long, value_name = "ADDRESS", num_args = 0..=1,
          default_missing_value = listen::SYSLOG_ADDRESS, conflicts_with_all = ["path", "listen"])]
    syslog: Option<String>,

    /// Also send each line of output to WebSocket clients on this address, such as :8081
    #[arg(long, value_name = "ADDRESS")]
    websocket: Option<String>,

    /// Also send each line of output to tcp://HOST:PORT, udp://HOST:PORT, or http://HOST:PORT/PATH
    #[arg(long, value_name = "URL")]
    forward: Option<String>,

    /// Keep at most this many input lines each second, dropping the rest
    #[arg(long, value_name = "LINES", value_parser = clap::value_parser!(u64).range(1..))]
    rate_limit: Option<u64>,

    /// Output format (ansi, csv, curses, html, json, jsonl, plain)
    #[arg(short, long)]
    output: Option<String>,

    /// A style sheet to add to HTML output after the theme's rules
    #[arg(long, value_name = "FILE")]
    css: Option<String>,

    /// Start each line of HTML output with its line number in the input
    #[arg(long)]
    line_numbers: bool,

    /// Fields csv and jsonl output write, as FIELD,FIELD (repeatable)
    #[arg(long, value_name = "FIELDS")]
    fields: Vec<String>,

    /// Worker threads used to render a file (default: one per core)
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Config file to read instead of ~/.splash/config.toml or ~/.splashrc
    #[arg(long)]
    config: Option<String>,

    /// Color theme (dark, light, solarized, dracula)
    #[arg(long)]
    theme: Option<String>,

    /// Override one token color, as KEY=COLOR (repeatable)
    #[arg(long, value_name = "KEY=COLOR")]
    color: Vec<String>,

    /// Load a saved color profile
    #[arg(long)]
    profile: Option<String>,

    /// Save the resolved colors as a profile and exit
    #[arg(long, value_name = "NAME")]
    save_profile: Option<String>,

    /// List saved color profiles
    #[arg(long)]
    list_profiles: bool,

    /// List available color themes
    #[arg(long)]
    list_themes: bool,

    /// List all available plugins
    #[arg(long)]
    list_plugins: bool,

    /// Also load plugin files from this directory (repeatable)
    #[arg(long, value_name = "DIR")]
    plugin_dir: Vec<String>,

    /// Load plugin files again whenever one changes
    #[arg(long)]
    reload_plugins: bool,

    /// Report which of a plugin file's patterns read each input line, and exit
    #[arg(long, value_name = "FILE")]
    test_regex: Option<String>,

    /// Read the input with this plugin, as --mode does
    #[arg(long, value_name = "NAME", conflicts_with = "mode")]
    plugin: Option<String>,

    /// Leave a plugin out of auto detection and refuse its mode (repeatable)
    #[arg(long, value_name = "NAME")]
    disable_plugin: Vec<String>,

    /// Keep only lines at these levels, as LEVEL,LEVEL, or drop a level written !LEVEL (repeatable)
    #[arg(long, value_name = "LEVELS")]
    level: Vec<String>,

    /// Keep only lines written at or after this time, such as 2h ago or 2024-01-01
    #[arg(long, value_name = "TIME")]
    since: Option<String>,

    /// Keep only lines written at or before this time
    #[arg(long, value_name = "TIME")]
    until: Option<String>,

    /// Keep only lines matching this pattern, and highlight the match (repeatable)
    #[arg(long, value_name = "PATTERN")]
    grep: Vec<String>,

    /// Drop lines matching this pattern (repeatable)
    #[arg(long, value_name = "PATTERN")]
    exclude: Vec<String>,

    /// Keep only lines holding a field with this value, as KEY=VALUE, or not, as KEY!=VALUE (repeatable)
    #[arg(long, value_name = "KEY=VALUE")]
    filter: Vec<String>,

    /// Mask IP addresses, email addresses, and user names
    #[arg(long)]
    anonymize: bool,

    /// Rewrite timestamps written as seconds since the epoch as dates
    #[arg(long)]
    convert_date: bool,

    /// Rewrite every timestamp as RFC 3339
    #[arg(long)]
    normalize_dates: bool,

    /// Rewrite every timestamp in this zone, such as UTC or America/New_York
    #[arg(long, value_name = "ZONE")]
    tz: Option<String>,

    /// Rewrite every timestamp as its distance from now, such as 2 hours ago
    #[arg(long)]
    relative: bool,

    /// Leave keywords such as ERROR, TCP, and SELECT in the style around them
    #[arg(long)]
    no_wordcolor: bool,

    /// Mark unknown users, and follow ports and addresses with their services, hosts, and locations
    #[arg(long, conflicts_with = "no_lookups")]
    lookups: bool,

    /// Turn lookups off, even when the config file turns them on
    #[arg(long)]
    no_lookups: bool,

    /// Write statistics on the kept lines in place of the lines
    #[arg(long, conflicts_with = "histogram")]
    stats: bool,

    /// Write the line count, error count, and time range after the lines
    #[arg(long)]
    summary: bool,

    /// Write the number of lines in each period, such as 5m or 1h, in place of the lines
    #[arg(long, value_name = "PERIOD", num_args = 0..=1, default_missing_value = stats::DEFAULT_BUCKET)]
    histogram: Option<String>,

    /// Format of --stats, --summary, and --histogram (text, json, csv)
    #[arg(long, value_name = "FORMAT")]
    stats_format: Option<String>,
}

fn main() {
    stop_quietly_on_a_closed_pipe();

    let args = Args::parse();

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
            format: args.stats_format,
        },
    };

    let settings = match Settings::resolve(&options, &config, &profiles) {
        Ok(settings) => settings,
        Err(e) => fail(&e.to_string()),
    };

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

    let input = match (args.syslog.as_deref(), args.listen.as_deref()) {
        (Some(address), _) => Input::Syslog(address),
        (None, Some(address)) => Input::Network(address),
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

    let _plugin_watcher = args
        .reload_plugins
        .then(|| watch_plugins(discovery, plugins));

    run(&input, &settings, args.label);

    if let Some(forwarder) = FORWARDER.get() {
        forwarder.finish();
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
        Input::Network(address) => {
            if let Err(e) =
                listen_on(address).and_then(|lines| render_received(lines, label, settings))
            {
                fail(&e.to_string());
            }
        }
        Input::Syslog(address) => {
            if let Err(e) =
                receive_syslog_on(address).and_then(|lines| render_received(lines, label, settings))
            {
                fail(&e.to_string());
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
                let kept = rate_limited(&sample);

                emit(&render_contents_counted(
                    &kept, &settings, &mut entry, &mut stats,
                ));
            }

            for line in lines {
                let line = line + "\n";
                let kept = rate_limited(&line);

                emit(&render_contents_counted(
                    &kept, &settings, &mut entry, &mut stats,
                ));
            }

            emit(&progress(&settings, &stats));
            emit_closing(&settings, &stats);
        }
    }
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

    let mut texts = Vec::new();

    if values.is_empty() || values == [STDIN_PATH] {
        let mut bytes = Vec::new();
        let _ = io::stdin().read_to_end(&mut bytes);
        texts.push((None, String::from_utf8_lossy(&bytes).into_owned()));
    } else {
        let sources = expand_paths(values, false).unwrap_or_else(|e| fail(&e.to_string()));

        for path in sources.files {
            let log = LogFile::open(&path).unwrap_or_else(|e| fail(&e.to_string()));
            texts.push((Some(path), log.text().to_string()));
        }
    }

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

        let header = Token::owned(format!("==> {} <==", path.display()), TokenKind::Header);
        let line = ParsedLine::new(vec![header]);

        emit(&format!(
            "{}\n",
            settings.output_mode.render(&line, &settings.theme)
        ));
    }
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

    if merge {
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
    } else {
        for (index, file) in files.iter_mut().enumerate() {
            headers.show(index, &file.path, settings);

            for chunk in source::line_chunks(complete[index], source::CHUNK_BYTES) {
                write_lines(chunk, &file.settings, &mut file.entry, &mut stats);
            }

            emit(&progress(settings, &stats));
        }
    }

    for (index, file) in files.iter_mut().enumerate() {
        file.follower = Follower::new(file.path.clone(), fs::metadata(&file.path)?.len());
        file.pending = logs[index].text().as_bytes()[complete[index].len()..].to_vec();
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
            let rest = String::from_utf8_lossy(&file.pending).into_owned();

            headers.show(index, &file.path, settings);
            write_rendered(&rest, &file.settings, &mut file.entry, &mut stats);
        }
    }

    emit_closing(settings, &stats);

    Ok(())
}

/// Renders the lines received over the network until splash is
/// interrupted. Lines that arrive together are rendered together, and in
/// auto mode the first of them decide the mode.
fn render_received(
    lines: mpsc::Receiver<Received>,
    label: bool,
    settings: &Settings,
) -> io::Result<()> {
    let mut settings = settings.clone();
    let mut entry = Entry::default();
    let mut stats = Stats::default();

    catch_interrupts();

    emit(&opening(&settings));

    while !INTERRUPTED.load(Ordering::SeqCst) {
        let Ok(first) = lines.recv_timeout(INTERRUPT_POLL) else {
            continue;
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
    let kept = rate_limited(text);

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
    print!("{}", text);

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
