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
use splash::filter::{Entry, FilterOptions};
use splash::listen;
use splash::rate::RateLimit;
use splash::registry::PluginRegistry;
use splash::source::{self, Follower, LogFile};
use splash::stats::{self, Stats, StatsOptions};
use splash::tail::Tail;
use splash::tui;
use splash::websocket::{self, Broadcaster};
use splash::{
    closing, opening, plugin_summary, profile_summary, progress, render_contents_counted,
    resolve_file_settings, theme_summary,
};
use std::borrow::Cow;
use std::fs;
use std::io::{self, IsTerminal};
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

/// The WebSocket clients output is also sent to, when --websocket is given
static BROADCASTER: OnceLock<Arc<Broadcaster>> = OnceLock::new();

/// Set when splash is asked to stop while following a file or reading stdin
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Where splash reads log lines from
enum Input<'a> {
    Stdin,
    File { path: &'a str, follow: bool },
    Network(&'a str),
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log Parsing Mode (apache-error, apm, auth, caddy, ci, clf, cloud-init, cron, distcc, dmesg,
    /// docker, dovecot, elasticsearch, exim, fetchmail, ftpstats, git, haproxy, httpd, icecast,
    /// journalctl, kubernetes, mongodb, mysql, nginx-error, oops, php, postfix, postgresql, procmail,
    /// proftpd, pure-ftpd, redis, squid, ssh, sudo, sulog, super, syslog, systemd-resolved,
    /// terraform, ulogd, varnish, vsftpd, xferlog, ad-hoc, auto)
    #[arg(short, long)]
    mode: Option<String>,

    /// Detect the log format from the first lines of the input
    #[arg(long, conflicts_with = "mode")]
    auto: bool,

    /// Path to the log file, or - to read stdin
    #[arg(short, long)]
    path: Option<String>,

    /// Keep reading the --path file as it grows, as tail -f does
    #[arg(short, long, requires = "path")]
    follow: bool,

    /// Receive log lines over TCP or HTTP on this address, such as :8080 or 127.0.0.1:8080
    #[arg(long, value_name = "ADDRESS", conflicts_with = "path")]
    listen: Option<String>,

    /// Also send each line of output to WebSocket clients on this address, such as :8081
    #[arg(long, value_name = "ADDRESS")]
    websocket: Option<String>,

    /// Keep at most this many input lines each second, dropping the rest
    #[arg(long, value_name = "LINES", value_parser = clap::value_parser!(u64).range(1..))]
    rate_limit: Option<u64>,

    /// Output format (ansi, curses, html, json, plain)
    #[arg(short, long)]
    output: Option<String>,

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

    /// Use a specific plugin by name
    #[arg(long)]
    plugin: Option<String>,

    /// Disable a specific plugin by name
    #[arg(long)]
    disable_plugin: Option<String>,

    /// Keep only lines at these levels, as LEVEL,LEVEL (repeatable)
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

    /// Keep only lines holding a field with this value, as KEY=VALUE (repeatable)
    #[arg(long, value_name = "KEY=VALUE")]
    filter: Vec<String>,

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
            plugin_summary(
                &PluginRegistry::with_builtins(),
                &PluginDiscovery::new(),
                &config
            )
        );
        return;
    }

    let mode = if args.auto {
        Some(detect::AUTO.to_string())
    } else {
        args.mode
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

    if let Some(plugin_name) = args.disable_plugin {
        println!("Disabling plugin: {}", plugin_name);
        println!("Note: Plugin disable functionality will be available in a future version");
        return;
    }

    if let Some(plugin_name) = args.plugin {
        println!("Using plugin: {}", plugin_name);
        println!("Note: Specific plugin selection will be available in a future version");
    }

    let input = match (args.listen.as_deref(), args.path.as_deref()) {
        (Some(address), _) => Input::Network(address),
        (None, Some(path)) if path != STDIN_PATH => Input::File {
            path,
            follow: args.follow,
        },
        _ => Input::Stdin,
    };

    if let Some(per_second) = args.rate_limit {
        if settings.output_mode.is_interactive() {
            fail(RATE_LIMIT_NEEDS_STDOUT);
        }

        let _ = RATE_LIMIT.set(Mutex::new(RateLimit::new(per_second as usize)));
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

    run(&input, &settings);
}

fn run(input: &Input, settings: &Settings) {
    let mode = settings.mode.as_str();
    let output_mode = settings.output_mode;

    if output_mode.is_interactive() {
        if let Err(e) = view(input, settings) {
            fail(&e.to_string());
        }

        return;
    }

    match *input {
        Input::File { path, follow } => {
            if let Err(e) = render_path(path, follow, settings) {
                fail(&format!("{:?}", e));
            }
        }
        Input::Network(address) => {
            if let Err(e) = render_network(address, settings) {
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
                let sample: Vec<String> = lines.by_ref().take(detect::SAMPLE_SIZE).collect();
                let sample = sample.join("\n");

                settings.mode = detect::resolve_mode(mode, &sample);
                let kept = rate_limited(&sample);

                emit(&render_contents_counted(
                    &kept, &settings, &mut entry, &mut stats,
                ));
            }

            for line in lines {
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

/// Renders a log file, then, when following, the lines it gains until
/// splash is interrupted
fn render_path<P: AsRef<Path>>(path: P, follow: bool, settings: &Settings) -> notify::Result<()> {
    let settings = &resolve_file_settings(path.as_ref(), settings)?;
    let mut entry = Entry::default();
    let mut stats = Stats::default();

    if !follow {
        let log = LogFile::open(path.as_ref())?;

        emit(&opening(settings));
        write_rendered(log.text(), settings, &mut entry, &mut stats);
        emit_closing(settings, &stats);

        return Ok(());
    }

    catch_interrupts();

    let (tx, rx) = mpsc::channel();

    let config = WatchConfig::default()
        .with_poll_interval(Duration::from_secs(2))
        .with_compare_contents(true);

    let mut watcher = RecommendedWatcher::new(tx, config)?;

    watcher.watch(path.as_ref(), RecursiveMode::NonRecursive)?;

    emit(&opening(settings));

    let log = LogFile::open(path.as_ref())?;
    let text = log.text();
    let complete = text.rfind('\n').map_or(0, |end| end + 1);

    write_rendered(&text[..complete], settings, &mut entry, &mut stats);

    let mut follower = Follower::new(path.as_ref().to_path_buf(), fs::metadata(&path)?.len());
    let mut pending = text.as_bytes()[complete..].to_vec();

    while !INTERRUPTED.load(Ordering::SeqCst) {
        // The watcher holds the sender, so receiving can only time out
        match rx.recv_timeout(INTERRUPT_POLL) {
            // inotify reports the open a read makes, which must not trigger another read
            Ok(Ok(event)) if event.kind.is_access() => {}
            Ok(_) => follower.read(&mut pending)?,
            Err(_) => {}
        }

        if let Some(lines) = source::take_complete_lines(&mut pending) {
            write_rendered(&lines, settings, &mut entry, &mut stats);
        }
    }

    if !pending.is_empty() {
        let rest = String::from_utf8_lossy(&pending);

        write_rendered(&rest, settings, &mut entry, &mut stats);
    }

    emit_closing(settings, &stats);

    Ok(())
}

/// Renders the lines connections send until splash is interrupted. Lines
/// that arrive together are rendered together, and in auto mode the first
/// of them decide the mode.
fn render_network(address: &str, settings: &Settings) -> io::Result<()> {
    let lines = listen_on(address)?;
    let mut settings = settings.clone();
    let mut entry = Entry::default();
    let mut stats = Stats::default();

    catch_interrupts();

    emit(&opening(&settings));

    while !INTERRUPTED.load(Ordering::SeqCst) {
        let Ok(first) = lines.recv_timeout(INTERRUPT_POLL) else {
            continue;
        };

        let arrived: Vec<String> = std::iter::once(first).chain(lines.try_iter()).collect();
        let text = arrived.join("\n");

        settings.mode = detect::resolve_mode(&settings.mode, &text);
        write_rendered(&text, &settings, &mut entry, &mut stats);
    }

    emit_closing(&settings, &stats);

    Ok(())
}

/// Starts listening on `address`, and names the address bound on stderr,
/// which gives the port chosen for port 0
fn listen_on(address: &str) -> io::Result<mpsc::Receiver<String>> {
    let (bound, lines) = listen::start(address)
        .map_err(|e| io::Error::other(format!("Cannot listen on {}: {}", address, e)))?;

    eprintln!("Listening on {}", bound);

    Ok(lines)
}

/// Writes the rendered text, followed by the report that replaces the lines
/// when there is one
fn write_rendered(text: &str, settings: &Settings, entry: &mut Entry, stats: &mut Stats) {
    let kept = rate_limited(text);
    let rendered = render_contents_counted(&kept, settings, entry, stats);

    emit(&rendered);
    emit(&progress(settings, stats));
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

/// Writes output to stdout and to any WebSocket clients
fn emit(text: &str) {
    print!("{}", text);

    if let Some(broadcaster) = BROADCASTER.get() {
        broadcaster.send(text);
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
        Input::File { path, .. } => Tail::file(
            PathBuf::from(path),
            resolve_file_settings(Path::new(path), settings)?,
        ),
        Input::Network(address) => Tail::stream(listen_on(address)?, settings.clone()),
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

/// Sends each line of standard input as it arrives, so the viewer can show
/// a pipe that is still being written
fn read_lines_in_background() -> mpsc::Receiver<String> {
    let (sender, receiver) = mpsc::channel();

    std::thread::spawn(move || {
        let _ = source::lines(io::stdin().lock()).try_for_each(|line| sender.send(line));
    });

    receiver
}
