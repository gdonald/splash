use clap::Parser;
use crossterm::event;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{cursor, execute, terminal};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use splash::discovery::PluginDiscovery;
use splash::output::OutputMode;
use splash::registry::PluginRegistry;
use splash::tui;
use splash::{plugin_summary, render_contents};
use std::fs::{self, File};
use std::io::{self, IsTerminal, Read, Seek, SeekFrom};
use std::path::Path;
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
    #[arg(short, long, default_value = "ansi")]
    output: String,

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

    if args.list_plugins {
        print!(
            "{}",
            plugin_summary(&PluginRegistry::new(), &PluginDiscovery::new())
        );
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

    let output_mode: OutputMode = match args.output.parse() {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let mode: String = args.mode.unwrap_or_else(|| "ad-hoc".to_string());

    if output_mode.is_interactive() {
        if let Err(e) = view(args.path.as_deref(), &mode, output_mode) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }

        return;
    }

    match args.path {
        Some(p) => {
            if let Err(e) = watch(p, &mode, output_mode) {
                eprintln!("Error: {:?}", e);
                std::process::exit(1);
            }
        }
        None => {
            if let Some(header) = output_mode.header() {
                print!("{}", header);
            }

            for line in std::io::stdin().lines() {
                print!("{}", render_contents(&line.unwrap(), &mode, output_mode));
            }

            if let Some(footer) = output_mode.footer() {
                print!("{}", footer);
            }
        }
    }
}

fn watch<P: AsRef<Path>>(path: P, mode: &str, output_mode: OutputMode) -> notify::Result<()> {
    let (tx, rx) = mpsc::channel();

    let config = Config::default()
        .with_poll_interval(Duration::from_secs(2))
        .with_compare_contents(true);

    let mut watcher = RecommendedWatcher::new(tx, config)?;

    watcher.watch(path.as_ref(), RecursiveMode::NonRecursive)?;

    if let Some(header) = output_mode.header() {
        print!("{}", header);
    }

    let mut contents = fs::read_to_string(&path).unwrap();
    print!("{}", render_contents(&contents, mode, output_mode));
    let mut pos = contents.len() as u64;

    loop {
        match rx.recv() {
            Ok(_) => {
                let mut f = File::open(&path).unwrap();
                f.seek(SeekFrom::Start(pos)).unwrap();

                pos = f.metadata().unwrap().len();

                contents.clear();
                f.read_to_string(&mut contents).unwrap();

                print!("{}", render_contents(&contents, mode, output_mode));
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Reads the whole input and shows it in the scrollable viewer
fn view(path: Option<&str>, mode: &str, output_mode: OutputMode) -> io::Result<()> {
    if !io::stdout().is_terminal() {
        return Err(io::Error::other(tui::NEEDS_TERMINAL));
    }

    let contents = match path {
        Some(p) => fs::read_to_string(p)?,
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    let (_, rows) = terminal::size()?;
    let mut viewer = tui::viewer_for(&contents, mode, output_mode, rows);
    let mut out = io::stdout();

    enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, cursor::Hide)?;

    let result = tui::run_loop(&mut out, &mut viewer, &mut || event::read());

    execute!(out, cursor::Show, LeaveAlternateScreen)?;
    disable_raw_mode()?;

    result
}
