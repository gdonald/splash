//! Reading log files
//!
//! Small files are read into memory. Files at or above [`MMAP_THRESHOLD`] are
//! memory mapped instead, so splash pages in only the parts of a large log it
//! renders rather than copying the whole file first.
use memmap2::Mmap;
use std::fs::File;
use std::io::{self, BufRead, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Files at least this large are memory mapped rather than read into memory
pub const MMAP_THRESHOLD: u64 = 64 * 1024;

enum Contents {
    Mapped(Mmap),
    Owned(String),
}

/// The text of a log file, however it was read
pub struct LogFile {
    contents: Contents,
}

impl LogFile {
    /// Reads a log file, memory mapping it when it is large enough to pay for
    /// the mapping. Bytes that are not valid UTF-8 are replaced, which needs a
    /// copy, so such a file is never left mapped.
    pub fn open(path: &Path) -> io::Result<LogFile> {
        let file = File::open(path)?;

        if file.metadata()?.len() < MMAP_THRESHOLD {
            return Ok(LogFile::from_bytes(read_bytes(&file)?));
        }

        // Safety: a log file changing underneath the mapping changes what
        // splash renders, which is the same risk as reading the file twice.
        let mapping = unsafe { Mmap::map(&file)? };

        if std::str::from_utf8(&mapping).is_err() {
            return Ok(LogFile::from_bytes(mapping.to_vec()));
        }

        Ok(LogFile {
            contents: Contents::Mapped(mapping),
        })
    }

    /// Holds text that was not read from a file, such as piped input
    pub fn from_text(text: String) -> LogFile {
        LogFile {
            contents: Contents::Owned(text),
        }
    }

    /// The file's text
    pub fn text(&self) -> &str {
        match &self.contents {
            Contents::Mapped(mapping) => std::str::from_utf8(mapping).unwrap_or_default(),
            Contents::Owned(text) => text,
        }
    }

    /// Whether the file was memory mapped rather than read into memory
    pub fn is_mapped(&self) -> bool {
        matches!(self.contents, Contents::Mapped(_))
    }

    /// The number of bytes of text the file holds
    pub fn len(&self) -> usize {
        self.text().len()
    }

    /// Whether the file holds no text
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn from_bytes(bytes: Vec<u8>) -> LogFile {
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
        };

        LogFile::from_text(text)
    }
}

fn read_bytes(mut file: &File) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();

    file.read_to_end(&mut bytes)?;

    Ok(bytes)
}

/// The lines of a stream such as standard input, each without its line
/// ending, read as they arrive. Bytes that are not valid UTF-8 are replaced
/// rather than ending the stream. The lines end with the stream or at the
/// first read error.
pub fn lines<R: BufRead>(mut reader: R) -> impl Iterator<Item = String> {
    let mut bytes = Vec::new();

    std::iter::from_fn(move || read_line(&mut reader, &mut bytes).ok().flatten())
}

/// Reads the next line of a stream without its line ending, replacing bytes
/// that are not valid UTF-8, or `None` at the end of the stream. `bytes` is
/// scratch space reused between calls.
pub fn read_line<R: BufRead>(reader: &mut R, bytes: &mut Vec<u8>) -> io::Result<Option<String>> {
    bytes.clear();

    if reader.read_until(b'\n', bytes)? == 0 {
        return Ok(None);
    }

    Ok(Some(
        String::from_utf8_lossy(without_line_ending(bytes)).into_owned(),
    ))
}

fn without_line_ending(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);

    line.strip_suffix(b"\r").unwrap_or(line)
}

/// A file read from where its last read stopped, followed across log
/// rotation as `tail -F` follows it
pub struct Follower {
    path: PathBuf,
    position: u64,
    /// The file the path named when it was last opened, kept open so the
    /// lines written to it before it was rotated away are still read
    file: Option<File>,
}

impl Follower {
    /// A follower whose first read starts `position` bytes into the file
    pub fn new(path: PathBuf, position: u64) -> Follower {
        let file = File::open(&path).ok();

        Follower {
            path,
            position,
            file,
        }
    }

    /// Appends the bytes the file gained since the last read to `pending`.
    /// A file shorter than the bytes already read from it was truncated, so
    /// `pending` is cleared and the file is read from its beginning. When the
    /// path names another file than the one being read, the log was rotated:
    /// the rest of the old file is read, an unfinished last line of it is
    /// ended, and the new file is read from its beginning.
    pub fn read(&mut self, pending: &mut Vec<u8>) -> io::Result<()> {
        let mut file = match self.file.take() {
            Some(file) => file,
            None => File::open(&self.path)?,
        };

        if file.metadata()?.len() < self.position {
            self.position = 0;
            pending.clear();
        }

        file.seek(SeekFrom::Start(self.position))?;
        self.position += file.read_to_end(pending)? as u64;

        match File::open(&self.path) {
            Ok(current) if !same_file(&current, &file)? => {
                if pending.last().is_some_and(|byte| *byte != b'\n') {
                    pending.push(b'\n');
                }

                self.file = Some(current);
                self.position = 0;
                self.read(pending)
            }
            _ => {
                self.file = Some(file);
                Ok(())
            }
        }
    }
}

/// Whether two open files are the same file on disk
#[cfg(unix)]
fn same_file(one: &File, other: &File) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;

    let (one, other) = (one.metadata()?, other.metadata()?);

    Ok(one.dev() == other.dev() && one.ino() == other.ino())
}

/// Whether two open files are the same file on disk. Without inode numbers
/// rotation is not detected, and the file first opened keeps being read.
#[cfg(not(unix))]
fn same_file(_one: &File, _other: &File) -> io::Result<bool> {
    Ok(true)
}

/// Removes the lines ending in a line break from the front of `pending` and
/// returns them as text, leaving a partial last line for later
pub fn take_complete_lines(pending: &mut Vec<u8>) -> Option<String> {
    let end = pending.iter().rposition(|byte| *byte == b'\n')?;
    let complete: Vec<u8> = pending.drain(..=end).collect();

    Some(String::from_utf8_lossy(&complete).into_owned())
}

/// The files a `--path` value names. A value whose file name holds `*` or
/// `?` names the files in its directory that match, in name order, leaving
/// out hidden files unless the pattern starts with a dot. Any other value
/// names itself.
pub fn expand(pattern: &str) -> io::Result<Vec<PathBuf>> {
    let path = Path::new(pattern);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    if !name.contains(['*', '?']) {
        return Ok(vec![path.to_path_buf()]);
    }

    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());

    let mut matches: Vec<PathBuf> = std::fs::read_dir(directory.unwrap_or(Path::new(".")))?
        .filter_map(Result::ok)
        .filter(|entry| {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();

            (name.starts_with('.') || !file_name.starts_with('.'))
                && wildcard_match(name.as_bytes(), file_name.as_bytes())
        })
        .map(|entry| match directory {
            Some(directory) => directory.join(entry.file_name()),
            None => PathBuf::from(entry.file_name()),
        })
        .collect();

    matches.sort();

    if matches.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No files match '{}'", pattern),
        ));
    }

    Ok(matches)
}

/// Whether `name` matches `pattern`, where `*` matches any run of bytes and
/// `?` any one byte
pub fn wildcard_match(pattern: &[u8], name: &[u8]) -> bool {
    match (pattern.first(), name.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            wildcard_match(&pattern[1..], name)
                || (!name.is_empty() && wildcard_match(pattern, &name[1..]))
        }
        (Some(b'?'), Some(_)) => wildcard_match(&pattern[1..], &name[1..]),
        (Some(expected), Some(found)) if expected == found => {
            wildcard_match(&pattern[1..], &name[1..])
        }
        _ => false,
    }
}

/// The files in a directory, in path order, leaving out hidden files and
/// directories, and with `recursive` the files in its subdirectories too
pub fn files_in(directory: &Path, recursive: bool) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;

        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            if recursive {
                files.extend(files_in(&path, recursive)?);
            }
        } else {
            files.push(path);
        }
    }

    files.sort();

    Ok(files)
}

/// The bytes of text splash renders and writes at a time, so a large file
/// is never held rendered in memory all at once
pub const CHUNK_BYTES: usize = 1024 * 1024;

/// Splits text into pieces of about `size` bytes, each ending at a line
/// break, except a last piece the text ends without one, or a line longer
/// than `size`, which is a piece of its own
pub fn line_chunks(text: &str, size: usize) -> impl Iterator<Item = &str> {
    let mut rest = text;

    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }

        let end = match rest.len() <= size {
            true => rest.len(),
            false => rest[..size]
                .rfind('\n')
                .or_else(|| rest[size..].find('\n').map(|found| size + found))
                .map_or(rest.len(), |found| found + 1),
        };

        let (chunk, remaining) = rest.split_at(end);
        rest = remaining;

        Some(chunk)
    })
}
