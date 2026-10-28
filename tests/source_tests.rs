use splash::source::{self, Encoding, Follower, LogFile, UnknownEncoding, MMAP_THRESHOLD};
use std::fs;
use std::io::{self, BufReader, Read};
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

fn stream_lines(bytes: &[u8]) -> Vec<String> {
    source::lines(bytes).collect()
}

#[test]
fn stream_lines_are_read_without_their_line_endings() {
    assert_eq!(
        stream_lines(b"alpha\nbeta\r\ngamma\n"),
        ["alpha", "beta", "gamma"]
    );
}

#[test]
fn a_stream_without_a_trailing_line_break_keeps_its_last_line() {
    assert_eq!(stream_lines(b"alpha\nbeta"), ["alpha", "beta"]);
}

#[test]
fn blank_stream_lines_are_kept() {
    assert_eq!(
        stream_lines(b"alpha\n\n\r\nbeta\n"),
        ["alpha", "", "", "beta"]
    );
}

#[test]
fn an_empty_stream_has_no_lines() {
    assert!(stream_lines(b"").is_empty());
}

#[test]
fn invalid_bytes_in_a_stream_are_replaced_and_reading_continues() {
    assert_eq!(
        stream_lines(b"alpha \xff beta\ngamma\n"),
        ["alpha \u{fffd} beta", "gamma"]
    );
}

struct FailsAfter<'a> {
    bytes: &'a [u8],
}

impl Read for FailsAfter<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.bytes.is_empty() {
            return Err(io::Error::other("device went away"));
        }

        self.bytes.read(buffer)
    }
}

#[test]
fn a_read_error_ends_the_stream_lines() {
    let reader = BufReader::new(FailsAfter {
        bytes: b"alpha\nbeta\n",
    });

    assert_eq!(source::lines(reader).collect::<Vec<_>>(), ["alpha", "beta"]);
}

#[test]
fn a_follower_starts_reading_at_its_position() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "followed.log", b"alpha\nbeta\n");
    let mut pending = Vec::new();

    Follower::new(path, 6).read(&mut pending).unwrap();

    assert_eq!(pending, b"beta\n");
}

#[test]
fn a_follower_reads_only_what_the_file_gained() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "followed.log", b"alpha\n");
    let mut follower = Follower::new(path.clone(), 0);
    let mut pending = Vec::new();
    follower.read(&mut pending).unwrap();
    pending.clear();

    fs::write(&path, b"alpha\nbeta\n").unwrap();
    follower.read(&mut pending).unwrap();

    assert_eq!(pending, b"beta\n");
}

#[test]
fn a_follower_reads_a_truncated_file_from_its_beginning() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "followed.log", b"alpha\nbeta\n");
    let mut follower = Follower::new(path.clone(), 0);
    let mut pending = Vec::new();
    follower.read(&mut pending).unwrap();

    fs::write(&path, b"gamma\n").unwrap();
    follower.read(&mut pending).unwrap();

    assert_eq!(pending, b"gamma\n");
}

#[test]
fn a_follower_of_a_missing_file_reports_it() {
    let directory = TempDir::new().unwrap();
    let mut follower = Follower::new(directory.path().join("missing.log"), 0);

    assert!(follower.read(&mut Vec::new()).is_err());
}

#[test]
fn complete_lines_are_taken_and_a_partial_line_is_left() {
    let mut pending = b"alpha\nbeta\ngam".to_vec();

    assert_eq!(
        source::take_complete_lines(&mut pending).as_deref(),
        Some("alpha\nbeta\n")
    );
    assert_eq!(pending, b"gam");
}

#[test]
fn a_partial_line_alone_has_no_complete_lines() {
    let mut pending = b"gam".to_vec();

    assert_eq!(source::take_complete_lines(&mut pending), None);
    assert_eq!(pending, b"gam");
}

#[test]
fn invalid_bytes_in_complete_lines_are_replaced() {
    let mut pending = b"alpha \xff\n".to_vec();

    assert_eq!(
        source::take_complete_lines(&mut pending).as_deref(),
        Some("alpha \u{fffd}\n")
    );
}

fn names(paths: &[std::path::PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect()
}

fn log_directory() -> TempDir {
    let directory = TempDir::new().unwrap();

    for name in ["b.log", "a.log", "notes.txt", ".hidden.log", "a.log.1"] {
        write(directory.path(), name, b"");
    }

    directory
}

#[test]
fn a_path_without_a_pattern_names_itself() {
    assert_eq!(
        source::expand("logs/missing.log").unwrap(),
        [std::path::PathBuf::from("logs/missing.log")]
    );
}

#[test]
fn a_pattern_names_the_matching_files_in_name_order() {
    let directory = log_directory();
    let pattern = directory.path().join("*.log");

    let paths = source::expand(pattern.to_str().unwrap()).unwrap();

    assert_eq!(names(&paths), ["a.log", "b.log"]);
    assert_eq!(paths[0].parent().unwrap(), directory.path());
}

#[test]
fn a_question_mark_matches_one_character() {
    let directory = log_directory();
    let pattern = directory.path().join("?.log*");

    assert_eq!(
        names(&source::expand(pattern.to_str().unwrap()).unwrap()),
        ["a.log", "a.log.1", "b.log"]
    );
}

#[test]
fn a_pattern_starting_with_a_dot_matches_hidden_files() {
    let directory = log_directory();
    let pattern = directory.path().join(".*.log");

    assert_eq!(
        names(&source::expand(pattern.to_str().unwrap()).unwrap()),
        [".hidden.log"]
    );
}

#[test]
fn a_pattern_without_a_directory_matches_in_the_current_one() {
    assert_eq!(
        source::expand("Cargo.t?ml").unwrap(),
        [std::path::PathBuf::from("Cargo.toml")]
    );
}

#[test]
fn a_pattern_matching_nothing_is_an_error() {
    let directory = log_directory();
    let pattern = directory.path().join("*.gz");
    let pattern = pattern.to_str().unwrap();

    let error = source::expand(pattern).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert_eq!(error.to_string(), format!("No files match '{}'", pattern));
}

#[test]
fn a_pattern_in_a_missing_directory_is_an_error() {
    assert!(source::expand("missing-directory/*.log").is_err());
}

#[test]
fn wildcards_match_runs_and_single_bytes() {
    assert!(source::wildcard_match(b"*", b""));
    assert!(source::wildcard_match(b"a*c", b"abbbc"));
    assert!(source::wildcard_match(b"a?c", b"abc"));
    assert!(!source::wildcard_match(b"a?c", b"ac"));
    assert!(!source::wildcard_match(b"a*c", b"abd"));
    assert!(!source::wildcard_match(b"abc", b"ab"));
}

fn nested_directory() -> TempDir {
    let directory = TempDir::new().unwrap();

    fs::create_dir_all(directory.path().join("api/old")).unwrap();
    fs::create_dir_all(directory.path().join(".cache")).unwrap();

    for name in [
        "web.log",
        ".hidden.log",
        "api/api.log",
        "api/old/api.log.1",
        ".cache/cached.log",
    ] {
        write(directory.path(), name, b"");
    }

    directory
}

fn relative(directory: &Path, paths: Vec<std::path::PathBuf>) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.strip_prefix(directory).unwrap().display().to_string())
        .collect()
}

#[test]
fn a_directory_holds_its_files_without_hidden_ones_or_subdirectories() {
    let directory = nested_directory();

    let files = source::files_in(directory.path(), false).unwrap();

    assert_eq!(relative(directory.path(), files), ["web.log"]);
}

#[test]
fn a_directory_read_recursively_holds_the_files_of_its_subdirectories() {
    let directory = nested_directory();

    let files = source::files_in(directory.path(), true).unwrap();

    assert_eq!(
        relative(directory.path(), files),
        ["api/api.log", "api/old/api.log.1", "web.log"]
    );
}

#[test]
fn a_missing_directory_cannot_be_listed() {
    assert!(source::files_in(Path::new("missing-directory"), false).is_err());
}

fn append(path: &Path, bytes: &[u8]) {
    use std::io::Write;

    fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

#[test]
fn a_follower_reads_the_rest_of_a_rotated_file_then_the_new_one() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "app.log", b"alpha\n");
    let mut follower = Follower::new(path.clone(), 6);
    let mut pending = Vec::new();

    append(&path, b"beta\n");
    fs::rename(&path, directory.path().join("app.log.1")).unwrap();
    write(directory.path(), "app.log", b"gamma\n");
    follower.read(&mut pending).unwrap();

    assert_eq!(pending, b"beta\ngamma\n");
}

#[test]
fn an_unfinished_last_line_of_a_rotated_file_is_ended() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "app.log", b"");
    let mut follower = Follower::new(path.clone(), 0);
    let mut pending = Vec::new();

    append(&path, b"beta");
    fs::rename(&path, directory.path().join("app.log.1")).unwrap();
    write(directory.path(), "app.log", b"gamma\n");
    follower.read(&mut pending).unwrap();

    assert_eq!(pending, b"beta\ngamma\n");
}

#[test]
fn a_rotated_file_keeps_being_read_until_a_new_one_appears() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "app.log", b"");
    let rotated = directory.path().join("app.log.1");
    let mut follower = Follower::new(path.clone(), 0);
    let mut pending = Vec::new();

    fs::rename(&path, &rotated).unwrap();
    append(&rotated, b"late line\n");
    follower.read(&mut pending).unwrap();

    assert_eq!(pending, b"late line\n");
}

#[test]
fn a_follower_reads_a_file_created_after_it_started() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("later.log");
    let mut follower = Follower::new(path.clone(), 0);
    let mut pending = Vec::new();

    write(directory.path(), "later.log", b"first\n");
    follower.read(&mut pending).unwrap();

    assert_eq!(pending, b"first\n");
}

#[test]
fn line_chunks_end_at_line_breaks_near_the_size_asked_for() {
    let chunks: Vec<&str> = splash::source::line_chunks("one\ntwo\nthree\nfour\n", 9).collect();

    assert_eq!(chunks, ["one\ntwo\n", "three\n", "four\n"]);
}

#[test]
fn a_line_longer_than_the_chunk_size_is_a_chunk_of_its_own() {
    let chunks: Vec<&str> = splash::source::line_chunks("a\nlonger line\nb\n", 4).collect();

    assert_eq!(chunks, ["a\n", "longer line\n", "b\n"]);
}

#[test]
fn the_last_chunk_keeps_a_last_line_without_a_line_break() {
    let chunks: Vec<&str> = splash::source::line_chunks("one\ntwo\nlast", 5).collect();

    assert_eq!(chunks, ["one\n", "two\n", "last"]);
}

#[test]
fn a_long_last_line_without_a_line_break_is_the_last_chunk() {
    let chunks: Vec<&str> = splash::source::line_chunks("a\nunfinished line", 4).collect();

    assert_eq!(chunks, ["a\n", "unfinished line"]);
}

#[test]
fn text_shorter_than_the_chunk_size_is_one_chunk() {
    let chunks: Vec<&str> = splash::source::line_chunks("one\ntwo\n", 1024).collect();

    assert_eq!(chunks, ["one\ntwo\n"]);
}

#[test]
fn empty_text_has_no_chunks() {
    assert_eq!(splash::source::line_chunks("", 1024).count(), 0);
}

#[test]
fn the_chunks_join_back_into_the_text() {
    let text = splash::bench::sample_log(5_000);

    assert_eq!(
        splash::source::line_chunks(&text, 4096).collect::<String>(),
        text
    );
}

/// The chunks a file is read in, and the bytes after its last line break
/// with the number of bytes read
fn chunks_of(bytes: &[u8], size: usize) -> (Vec<String>, Vec<u8>, u64) {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "app.log", bytes);
    let mut chunks = Vec::new();

    let (rest, read) = source::read_chunks(&path, size, Encoding::Utf8, |chunk| {
        chunks.push(chunk.to_string())
    })
    .unwrap();

    (chunks, rest, read)
}

#[test]
fn a_file_is_read_in_chunks_of_whole_lines() {
    assert_eq!(
        chunks_of(b"one\ntwo\nthree\nfour", 8),
        (
            vec!["one\ntwo\n".to_string(), "three\n".to_string()],
            b"four".to_vec(),
            18
        )
    );
}

#[test]
fn a_file_smaller_than_a_chunk_is_left_whole_for_the_caller() {
    assert_eq!(
        chunks_of(b"one\ntwo\n", 64),
        (Vec::<String>::new(), b"one\ntwo\n".to_vec(), 8)
    );
}

#[test]
fn a_line_longer_than_a_chunk_is_read_whole() {
    assert_eq!(
        chunks_of(b"a long line\nend", 4),
        (vec!["a long line\n".to_string()], b"end".to_vec(), 15)
    );
}

#[test]
fn invalid_bytes_in_a_chunk_are_replaced() {
    assert_eq!(
        chunks_of(b"caf\xe9\nok\n", 4).0,
        vec!["caf\u{fffd}\nok\n".to_string()]
    );
}

#[test]
fn reading_a_missing_file_in_chunks_reports_it() {
    let directory = TempDir::new().unwrap();

    let missing = directory.path().join("missing.log");

    assert!(source::read_chunks(&missing, 8, Encoding::Utf8, |_| {}).is_err());
}

#[test]
fn encodings_are_named_in_any_case_with_or_without_hyphens() {
    assert_eq!(
        [
            "utf-8",
            "UTF8",
            "latin1",
            "Latin-1",
            "ISO-8859-1",
            "iso_8859_1"
        ]
        .map(|name| Encoding::parse(name).unwrap()),
        [
            Encoding::Utf8,
            Encoding::Utf8,
            Encoding::Latin1,
            Encoding::Latin1,
            Encoding::Latin1,
            Encoding::Latin1
        ]
    );
}

#[test]
fn an_unknown_encoding_is_refused() {
    let error = Encoding::parse("ebcdic").unwrap_err();

    assert_eq!(
        (error.clone(), error.to_string()),
        (
            UnknownEncoding("ebcdic".to_string()),
            "Unknown encoding 'ebcdic' (expected utf-8 or latin1)".to_string()
        )
    );
}

#[test]
fn the_locale_names_the_encoding_from_lc_all_then_lc_ctype_then_lang() {
    assert_eq!(
        [
            Encoding::from_locale(Some("de_DE.ISO-8859-1"), Some("en_US.UTF-8"), None),
            Encoding::from_locale(None, Some("de_DE.ISO-8859-1"), Some("en_US.UTF-8")),
            Encoding::from_locale(Some(""), None, Some("fr_FR.ISO8859-1@euro")),
            Encoding::from_locale(None, None, Some("en_US.UTF-8")),
            Encoding::from_locale(None, None, Some("C")),
            Encoding::from_locale(None, None, Some("ja_JP.EUC-JP")),
            Encoding::from_locale(None, None, None),
        ],
        [
            Encoding::Latin1,
            Encoding::Latin1,
            Encoding::Latin1,
            Encoding::Utf8,
            Encoding::Utf8,
            Encoding::Utf8,
            Encoding::Utf8
        ]
    );
}

#[test]
fn latin1_reads_one_character_per_byte() {
    assert_eq!(
        (
            Encoding::Latin1.decode(b"caf\xe9"),
            Encoding::Latin1.decode(b"plain"),
            Encoding::Utf8.decode(b"caf\xe9"),
        ),
        (
            std::borrow::Cow::Owned::<str>("café".to_string()),
            std::borrow::Cow::Borrowed("plain"),
            std::borrow::Cow::Owned::<str>("caf\u{fffd}".to_string()),
        )
    );
}

#[test]
fn latin1_writes_a_character_it_has_no_byte_for_as_a_question_mark() {
    assert_eq!(
        (
            Encoding::Latin1.encode("café ▁ 日"),
            Encoding::Latin1.encode("plain"),
            Encoding::Utf8.encode("é"),
        ),
        (
            std::borrow::Cow::Owned::<[u8]>(b"caf\xe9 ? ?".to_vec()),
            std::borrow::Cow::Borrowed(&b"plain"[..]),
            std::borrow::Cow::Borrowed("é".as_bytes()),
        )
    );
}

#[test]
fn a_latin1_file_is_decoded_whatever_its_size() {
    let directory = TempDir::new().unwrap();
    let small = write(directory.path(), "small.log", b"caf\xe9\n");
    let mut large_bytes = large_text().into_bytes();
    large_bytes.extend_from_slice(b"caf\xe9\n");
    let large = write(directory.path(), "large.log", &large_bytes);
    let ascii = write(directory.path(), "ascii.log", large_text().as_bytes());

    let small_log = LogFile::open_with(&small, Encoding::Latin1).unwrap();
    let large_log = LogFile::open_with(&large, Encoding::Latin1).unwrap();
    let ascii_log = LogFile::open_with(&ascii, Encoding::Latin1).unwrap();

    assert_eq!(
        (
            small_log.text(),
            large_log.text().ends_with("café\n"),
            large_log.is_mapped(),
            ascii_log.is_mapped()
        ),
        ("café\n", true, false, true)
    );
}

#[test]
fn a_utf8_file_read_as_latin1_is_read_one_character_per_byte() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "app.log", "café\n".as_bytes());

    assert_eq!(
        LogFile::open_with(&path, Encoding::Latin1).unwrap().text(),
        "cafÃ©\n"
    );
}

#[test]
fn chunks_are_decoded_in_the_encoding_given() {
    let directory = TempDir::new().unwrap();
    let path = write(directory.path(), "app.log", b"caf\xe9\nok\n");
    let mut chunks = Vec::new();

    source::read_chunks(&path, 4, Encoding::Latin1, |chunk| {
        chunks.push(chunk.to_string())
    })
    .unwrap();

    assert_eq!(chunks, ["café\nok\n"]);
}

#[test]
fn the_encodings_chosen_for_a_run_are_kept() {
    source::set_encoding(Encoding::Utf8);
    source::set_output_encoding(Encoding::Utf8);

    assert_eq!(
        (source::encoding(), source::output_encoding()),
        (Encoding::Utf8, Encoding::Utf8)
    );
}
