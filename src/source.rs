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

/// A file read from where its last read stopped
pub struct Follower {
    path: PathBuf,
    position: u64,
}

impl Follower {
    /// A follower whose first read starts `position` bytes into the file
    pub fn new(path: PathBuf, position: u64) -> Follower {
        Follower { path, position }
    }

    /// Appends the bytes the file gained since the last read to `pending`.
    /// A file shorter than the bytes already read from it was truncated or
    /// replaced, so `pending` is cleared and the file is read from its
    /// beginning.
    pub fn read(&mut self, pending: &mut Vec<u8>) -> io::Result<()> {
        let mut file = File::open(&self.path)?;

        if file.metadata()?.len() < self.position {
            self.position = 0;
            pending.clear();
        }

        file.seek(SeekFrom::Start(self.position))?;
        self.position += file.read_to_end(pending)? as u64;

        Ok(())
    }
}

/// Removes the lines ending in a line break from the front of `pending` and
/// returns them as text, leaving a partial last line for later
pub fn take_complete_lines(pending: &mut Vec<u8>) -> Option<String> {
    let end = pending.iter().rposition(|byte| *byte == b'\n')?;
    let complete: Vec<u8> = pending.drain(..=end).collect();

    Some(String::from_utf8_lossy(&complete).into_owned())
}
