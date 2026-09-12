use splash::source::{LogFile, MMAP_THRESHOLD};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn large_text() -> String {
    let line = "10.0.0.1 - - [10/Oct/2000:13:55:36 -0700] \"GET /index.html HTTP/1.0\" 200 2326\n";

    line.repeat((MMAP_THRESHOLD as usize / line.len()) + 1)
}

fn write(directory: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let path = directory.join(name);

    fs::write(&path, bytes).unwrap();

    path
}

#[test]
fn text_given_directly_is_held_as_it_is() {
    let log = LogFile::from_text("alpha\nbeta\n".to_string());

    assert_eq!(log.text(), "alpha\nbeta\n");
    assert!(!log.is_mapped());
}

#[test]
fn a_small_file_is_read_into_memory() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "small.log", b"alpha\nbeta\n");

    let log = LogFile::open(&path).unwrap();

    assert_eq!(log.text(), "alpha\nbeta\n");
    assert!(!log.is_mapped());
    assert_eq!(log.len(), 11);
    assert!(!log.is_empty());
}

#[test]
fn an_empty_file_holds_no_text() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "empty.log", b"");

    let log = LogFile::open(&path).unwrap();

    assert!(log.is_empty());
    assert_eq!(log.text(), "");
}

#[test]
fn a_large_file_is_memory_mapped() {
    let directory = TempDir::new().unwrap();
    let text = large_text();
    let path = write(directory.path(), "large.log", text.as_bytes());

    let log = LogFile::open(&path).unwrap();

    assert!(log.is_mapped());
    assert_eq!(log.text(), text);
    assert_eq!(log.len(), text.len());
}

#[test]
fn a_file_just_under_the_threshold_is_read_into_memory() {
    let directory = TempDir::new().unwrap();
    let text = "a".repeat(MMAP_THRESHOLD as usize - 1);
    let path = write(directory.path(), "under.log", text.as_bytes());

    let log = LogFile::open(&path).unwrap();

    assert!(!log.is_mapped());
    assert_eq!(log.len(), text.len());
}

#[test]
fn invalid_bytes_in_a_small_file_are_replaced() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "invalid.log", b"alpha \xff beta");

    let log = LogFile::open(&path).unwrap();

    assert_eq!(log.text(), "alpha \u{fffd} beta");
}

#[test]
fn a_large_file_with_invalid_bytes_is_read_into_memory_instead() {
    let directory = TempDir::new().unwrap();
    let mut bytes = large_text().into_bytes();
    bytes.extend_from_slice(b"\xff\n");
    let path = write(directory.path(), "invalid_large.log", &bytes);

    let log = LogFile::open(&path).unwrap();

    assert!(!log.is_mapped());
    assert!(log.text().ends_with("\u{fffd}\n"));
}

#[test]
fn a_missing_file_is_reported() {
    let directory = TempDir::new().unwrap();

    assert!(LogFile::open(&directory.path().join("missing.log")).is_err());
}
