use clap::Parser;
use colored::Colorize;
use notify::{Config, RecommendedWatcher, Watcher, RecursiveMode};
use regex::Regex;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
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
}

struct Log<'a> {
    client: &'a str,
    user_identifier: &'a str,
    userid: &'a str,
    datetime: &'a str,
    method: &'a str,
    request: &'a str,
    protocol: &'a str,
    status: &'a str,
    size: &'a str,
}

fn main() {
    let args = Args::parse();

    let mode: String = match args.mode {
        Some(m) => { m }
        _ => { "ad-hoc".to_string() }
    };

    let path: Option<String> = match args.path {
        Some(p) => { Some(p) },
        _ => { None }
    };

    match path {
        Some(p) => {
            if let Err(e) = watch(p, &mode) {
                eprintln!("Error: {:?}", e);
                std::process::exit(1);
            }
        }
        None => {
            for line in std::io::stdin().lines() {
                print_contents(&line.unwrap(), &mode);
            }
        }
    }
}

fn watch<P: AsRef<Path>>(path: P, mode: &str) -> notify::Result<()> {
    let (tx, rx) = mpsc::channel();

    let config = Config::default()
                    .with_poll_interval(Duration::from_secs(2))
                    .with_compare_contents(true);

    let mut watcher = RecommendedWatcher::new(tx, config)?;

    watcher.watch(path.as_ref(), RecursiveMode::NonRecursive)?;
    
    let mut contents = fs::read_to_string(&path).unwrap();
    let mut pos = contents.len() as u64;

    loop {
        match rx.recv() {
            Ok(_) => {
                let mut f = File::open(&path).unwrap();
                f.seek(SeekFrom::Start(pos)).unwrap();

                pos = f.metadata().unwrap().len();

                contents.clear();
                f.read_to_string(&mut contents).unwrap();

                print_contents(&contents, mode);
            }
            Err(e) => {
                eprintln!("Error: {:?}", e);
                std::process::exit(1);
            }
        }
    }
}

fn print_contents(contents: &str, mode: &str) {
    match mode {
        "clf" => { print_clf(contents) },
        _ => { print_adhoc(contents) }
    }
}

fn print_adhoc(contents: &str) {
    let mut lines = contents.lines();

    while let Some(line) = lines.next() {
        if line.is_empty() {
            continue;
        }

        println!("{} ", line);
    }
}

fn print_clf(contents: &str) {
    // common log format
    let re = Regex::new(
        r#"(?x)
        ([\d]{1,3}\.[\d]{1,3}\.[\d]{1,3}\.[\d]{1,3}) # client
        \s
        (\S+)                                        # user_identifier
        \s
        (\S+)                                        # userid
        \s
        (?:(\[.*?\]))                                # datetime
        \s
        "([A-Z]+)\s(\S+)\s(\S+)"                     # method, request, protocol
        \s
        (\d{3})                                      # status
        \s
        (\d+|-)                                      # size
        "#
    ).unwrap();

    let mut lines = contents.lines();

    while let Some(line) = lines.next() {
        if line.is_empty() {
            continue;
        }

        let fields = re.captures_iter(line).filter_map(|cap| {
            let groups = (
                cap.get(1),
                cap.get(2),
                cap.get(3),
                cap.get(4),
                cap.get(5),
                cap.get(6),
                cap.get(7),
                cap.get(8),
                cap.get(9),
            );
            match groups {
                (
                    Some(client),
                    Some(user_identifier),
                    Some(userid),
                    Some(datetime),
                    Some(method),
                    Some(request),
                    Some(protocol),
                    Some(status),
                    Some(size),
                ) => Some(Log {
                    client: client.as_str(),
                    user_identifier: user_identifier.as_str(),
                    userid: userid.as_str(),
                    datetime: datetime.as_str(),
                    method: method.as_str(),
                    request: request.as_str(),
                    protocol: protocol.as_str(),
                    status: status.as_str(),
                    size: size.as_str(),
                }),
                _ => None,
            }
        });

        for field in fields {
            print!("{} ", field.client.bright_red());
            print!("{} ", field.user_identifier.white());
            print!("{} ", field.userid.white().bold());
            print!("{} ", field.datetime.bright_magenta());
            print!("\"{} {} {}\" ", field.method.bright_cyan(), field.request.cyan(), field.protocol.cyan());
            print!("{} ", field.status.bright_yellow());
            print!("{}",  field.size.bright_green());
            println!();
        }
    }
}
