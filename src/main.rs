use clap::Parser;
use crossterm::event;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute, terminal};
use notify::{Config as WatchConfig, RecommendedWatcher, RecursiveMode, Watcher};
use splash::config::{Config, Options, Profiles, Settings};
use splash::discovery::PluginDiscovery;
use splash::registry::PluginRegistry;
use splash::source::LogFile;
use splash::tui;
use splash::{plugin_summary, profile_summary, render_contents, render_file, theme_summary};
use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log Parsing Mode (clf, ad-hoc)
    #[arg(short, long)]
    mode: Option<String>,

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
            plugin_summary(&PluginRegistry::new(), &PluginDiscovery::new(), &config)
        );
        return;
    }

    let options = Options {
        mode: args.mode,
        output: args.output,
        theme: args.theme,
        profile: args.profile,
        colors: args.color,
        jobs: args.jobs,
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
    let theme = &settings.theme;

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
            if let Some(header) = output_mode.header(theme) {
                print!("{}", header);
            }

            for line in std::io::stdin().lines() {
                print!(
                    "{}",
                    render_contents(&line.unwrap(), mode, output_mode, theme)
                );
            }

            if let Some(footer) = output_mode.footer() {
                print!("{}", footer);
            }
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
    let mode = settings.mode.as_str();
    let output_mode = settings.output_mode;
    let theme = &settings.theme;

    let (tx, rx) = mpsc::channel();

    let config = WatchConfig::default()
        .with_poll_interval(Duration::from_secs(2))
        .with_compare_contents(true);

    let mut watcher = RecommendedWatcher::new(tx, config)?;

    watcher.watch(path.as_ref(), RecursiveMode::NonRecursive)?;

    if let Some(header) = output_mode.header(theme) {
        print!("{}", header);
    }

    let rendered = render_file(path.as_ref(), settings).unwrap();
    print!("{}", rendered);
    let mut pos = fs::metadata(&path)?.len();
    let mut contents = String::new();

    loop {
        match rx.recv() {
            Ok(_) => {
                let mut f = File::open(&path).unwrap();
                f.seek(SeekFrom::Start(pos)).unwrap();

                pos = f.metadata().unwrap().len();

                contents.clear();
                f.read_to_string(&mut contents).unwrap();

                print!("{}", render_contents(&contents, mode, output_mode, theme));
            }
            Err(e) => fail(&format!("{:?}", e)),
        }
    }
}

/// Reads the whole input and shows it in the scrollable viewer
fn view(path: Option<&str>, settings: &Settings) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::other(tui::NEEDS_TERMINAL));
    }

    let log = match path {
        Some(p) => LogFile::open(Path::new(p))?,
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            LogFile::from_text(buffer)
        }
    };

    let (_, rows) = terminal::size()?;
    let mut viewer = tui::viewer_for(
        log.text(),
        &settings.mode,
        settings.output_mode,
        &settings.theme,
        rows,
    );
    let mut out = io::stdout();

    enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, cursor::Hide)?;

    let result = tui::run_loop(&mut out, &mut viewer, &mut || event::read());

    execute!(out, cursor::Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    result
}
