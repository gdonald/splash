use splash::config::{Config, Options, Profiles, Settings};
use splash::filter::Entry;
use splash::render_viewer_lines;
use splash::tail::Tail;
use splash::viewer::Line;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tempfile::TempDir;

fn settings(mode: &str, output: &str) -> Settings {
    Settings::resolve(
        &Options {
            mode: Some(mode.to_string()),
            output: Some(output.to_string()),
            jobs: Some(1),
            ..Options::default()
        },
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap()
}

fn raw(lines: &[Line]) -> Vec<&str> {
    lines.iter().map(|line| line.raw.as_str()).collect()
}

fn append(path: &Path, text: &str) {
    OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

fn log_file(directory: &TempDir, text: &str) -> PathBuf {
    let path = directory.path().join("app.log");
    fs::write(&path, text).unwrap();

    path
}

#[test]
fn viewer_lines_pair_each_rendering_with_the_log_text() {
    let lines = render_viewer_lines(
        "alpha    beta\n\ngamma\n",
        &settings("ad-hoc", "plain"),
        &mut Entry::default(),
    );

    assert_eq!(
        lines,
        vec![
            Line::new("alpha    beta", "alpha    beta"),
            Line::new("gamma", "gamma")
        ]
    );
}

#[test]
fn a_file_tail_first_reads_the_whole_file() {
    let directory = TempDir::new().unwrap();
    let path = log_file(&directory, "one\ntwo\n");
    let mut tail = Tail::file(path, settings("ad-hoc", "plain"));

    assert_eq!(raw(&tail.read().unwrap()), ["one", "two"]);
}

#[test]
fn a_file_tail_then_reads_only_the_lines_the_file_gains() {
    let directory = TempDir::new().unwrap();
    let path = log_file(&directory, "one\n");
    let mut tail = Tail::file(path.clone(), settings("ad-hoc", "plain"));

    tail.read().unwrap();
    append(&path, "two\nthree\n");

    assert_eq!(raw(&tail.read().unwrap()), ["two", "three"]);
}

#[test]
fn a_file_tail_with_nothing_new_reads_no_lines() {
    let directory = TempDir::new().unwrap();
    let path = log_file(&directory, "one\n");
    let mut tail = Tail::file(path, settings("ad-hoc", "plain"));

    tail.read().unwrap();

    assert!(tail.read().unwrap().is_empty());
}

#[test]
fn a_partial_line_waits_for_its_line_break() {
    let directory = TempDir::new().unwrap();
    let path = log_file(&directory, "one\ntw");
    let mut tail = Tail::file(path.clone(), settings("ad-hoc", "plain"));

    assert_eq!(raw(&tail.read().unwrap()), ["one"]);

    append(&path, "o\n");

    assert_eq!(raw(&tail.read().unwrap()), ["two"]);
}

#[test]
fn a_truncated_file_is_read_again_from_its_beginning() {
    let directory = TempDir::new().unwrap();
    let path = log_file(&directory, "one\ntwo\nthree\n");
    let mut tail = Tail::file(path.clone(), settings("ad-hoc", "plain"));

    tail.read().unwrap();
    fs::write(&path, "four\n").unwrap();

    assert_eq!(raw(&tail.read().unwrap()), ["four"]);
}

#[test]
fn a_missing_file_is_reported() {
    let directory = TempDir::new().unwrap();
    let mut tail = Tail::file(
        directory.path().join("missing.log"),
        settings("ad-hoc", "plain"),
    );

    assert!(tail.read().is_err());
}

#[test]
fn a_stream_tail_reads_the_lines_sent_so_far() {
    let (sender, receiver) = mpsc::channel();
    let mut tail = Tail::stream(receiver, settings("ad-hoc", "plain"));

    sender.send("one".to_string()).unwrap();
    sender.send("two".to_string()).unwrap();

    assert_eq!(raw(&tail.read().unwrap()), ["one", "two"]);
    assert!(tail.read().unwrap().is_empty());
}

#[test]
fn a_tail_in_auto_mode_renders_with_the_mode_its_first_lines_are_written_in() {
    let (sender, receiver) = mpsc::channel();
    let mut tail = Tail::stream(receiver, settings("auto", "json"));

    sender
        .send(r#"10.0.0.5 - - [03/Oct/2023:12:00:01 +0000] "GET / HTTP/1.1" 200 512"#.to_string())
        .unwrap();

    assert!(tail.read().unwrap()[0]
        .rendered
        .contains(r#"{"kind":"client","text":"10.0.0.5"}"#));
}
