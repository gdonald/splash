//! Reading log files
//!
//! Small files are read into memory. Files at or above [`MMAP_THRESHOLD`] are
//! memory mapped instead, so splash pages in only the parts of a large log it
//! renders rather than copying the whole file first.
use memmap2::Mmap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

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
