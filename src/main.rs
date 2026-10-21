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
use splash::registry::PluginRegistry;
use splash::source::LogFile;
use splash::stats::{self, Stats, StatsOptions};
use splash::tail::Tail;
use splash::tui;
use splash::{
    closing, opening, plugin_summary, profile_summary, progress, render_contents_counted,
    resolve_file_settings, theme_summary,
};
use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

/// How often a followed file checks whether splash was interrupted
const INTERRUPT_POLL: Duration = Duration::from_millis(100);

/// How long the viewer waits for a key before checking the log for new lines
const VIEWER_POLL: Duration = Duration::from_millis(250);

/// Set when splash is asked to stop while following a file
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

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

    /// Path to the log file
    #[arg(short, long)]
    path: Option<String>,

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

    run(args.path.as_deref(), &settings);
}

fn run(path: Option<&str>, settings: &Settings) {
    let mode = settings.mode.as_str();
    let output_mode = settings.output_mode;

    if output_mode.is_interactive() {
        if let Err(e) = view(path, settings) {
            fail(&e.to_string());
        }

        return;
    }

    match path {
        Some(p) => {
            if let Err(e) = watch(p, settings) {
                fail(&format!("{:?}", e));
            }
        }
        None => {
            print!("{}", opening(settings));

            let mut lines = io::stdin().lines().map(|line| line.unwrap());
            let mut settings = settings.clone();
            let mut entry = Entry::default();
            let mut stats = Stats::default();

            if mode == detect::AUTO {
                let sample: Vec<String> = lines.by_ref().take(detect::SAMPLE_SIZE).collect();
                let sample = sample.join("\n");

                settings.mode = detect::resolve_mode(mode, &sample);
                print!(
                    "{}",
                    render_contents_counted(&sample, &settings, &mut entry, &mut stats)
                );
            }

            for line in lines {
                print!(
                    "{}",
                    render_contents_counted(&line, &settings, &mut entry, &mut stats)
                );
            }

            print!("{}", progress(&settings, &stats));
            print!("{}", closing(&settings, &stats));
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

fn watch<P: AsRef<Path>>(path: P, settings: &Settings) -> notify::Result<()> {
    let settings = &resolve_file_settings(path.as_ref(), settings)?;

    catch_interrupts();

    let (tx, rx) = mpsc::channel();

    let config = WatchConfig::default()
        .with_poll_interval(Duration::from_secs(2))
        .with_compare_contents(true);

    let mut watcher = RecommendedWatcher::new(tx, config)?;

    watcher.watch(path.as_ref(), RecursiveMode::NonRecursive)?;

    print!("{}", opening(settings));

    let mut entry = Entry::default();
    let mut stats = Stats::default();
    let log = LogFile::open(path.as_ref())?;
    print!(
        "{}",
        render_contents_counted(log.text(), settings, &mut entry, &mut stats)
    );
    print!("{}", progress(settings, &stats));
    let mut pos = fs::metadata(&path)?.len();
    let mut contents = String::new();

    loop {
        match rx.recv_timeout(INTERRUPT_POLL) {
            Ok(_) => {
                let mut f = File::open(&path).unwrap();
                f.seek(SeekFrom::Start(pos)).unwrap();

                pos = f.metadata().unwrap().len();

                contents.clear();
                f.read_to_string(&mut contents).unwrap();

                if contents.is_empty() {
                    continue;
                }

                print!(
                    "{}",
                    render_contents_counted(&contents, settings, &mut entry, &mut stats)
                );
                print!("{}", progress(settings, &stats));
            }
            Err(RecvTimeoutError::Timeout) if INTERRUPTED.load(Ordering::SeqCst) => {
                print!("{}", closing(settings, &stats));

                return Ok(());
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(e) => fail(&format!("{:?}", e)),
        }
    }
}

#[cfg(unix)]
extern "C" fn note_interrupt(_signal: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Has Ctrl-C or `kill` end the follow loop, so splash writes what it
/// writes when its input ends
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

/// Reads the whole input and shows it in the scrollable viewer
fn view(path: Option<&str>, settings: &Settings) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::other(tui::NEEDS_TERMINAL));
    }

    let mut tail = match path {
        Some(p) => Tail::file(
            PathBuf::from(p),
            resolve_file_settings(Path::new(p), settings)?,
        ),
        None if io::stdin().is_terminal() => return Err(io::Error::other(tui::NEEDS_INPUT)),
        None => Tail::stream(read_lines_in_background(), settings.clone()),
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
        let _ = io::stdin()
            .lines()
            .map_while(Result::ok)
            .try_for_each(|line| sender.send(line));
    });

    receiver
}
