use notify::{Config, RecommendedWatcher, Watcher, RecursiveMode};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("Argument 1 needs to be a path");
    
    // println!("watching {}", path);
    
    if let Err(e) = watch(path) {
        println!("error: {:?}", e)
    }
}

fn watch<P: AsRef<Path>>(path: P) -> notify::Result<()> {
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
            Ok(_event) => {
                // println!("event: {:?}", event);

                let mut f = File::open(&path).unwrap();
                f.seek(SeekFrom::Start(pos)).unwrap();

                pos = f.metadata().unwrap().len();

                contents.clear();
                f.read_to_string(&mut contents).unwrap();

                print!("{}", contents);
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                std::process::exit(1);
            }
        }
    }
}
