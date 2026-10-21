//! New log text for the interactive viewer
//!
//! A tail reads the text a followed file or a stream of lines has gained
//! since it was last read, renders the complete lines, and keeps a partial
//! last line for the next read.
use crate::config::Settings;
use crate::detect;
use crate::filter::Entry;
use crate::render_viewer_lines;
use crate::viewer::Line;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

enum Source {
    File { path: PathBuf, position: u64 },
    Stream(Receiver<String>),
}

pub struct Tail {
    source: Source,
    settings: Settings,
    entry: Entry,
    /// The text read after the last line break
    pending: Vec<u8>,
}

impl Tail {
    /// A tail over a file, starting at its beginning
    pub fn file(path: PathBuf, settings: Settings) -> Tail {
        Tail::over(Source::File { path, position: 0 }, settings)
    }

    /// A tail over the lines a stream sends, such as a pipe read on another
    /// thread
    pub fn stream(lines: Receiver<String>, settings: Settings) -> Tail {
        Tail::over(Source::Stream(lines), settings)
    }

    fn over(source: Source, settings: Settings) -> Tail {
        Tail {
            source,
            settings,
            entry: Entry::default(),
            pending: Vec::new(),
        }
    }

    /// Renders the complete lines read since the last call. A file shorter
    /// than the text already read from it was truncated or replaced, so it
    /// is read again from its beginning. In auto mode, the first lines read
    /// decide the mode.
    pub fn read(&mut self) -> io::Result<Vec<Line>> {
        match &mut self.source {
            Source::File { path, position } => {
                let mut file = File::open(path)?;

                if file.metadata()?.len() < *position {
                    *position = 0;
                    self.pending.clear();
                }

                file.seek(SeekFrom::Start(*position))?;
                *position += file.read_to_end(&mut self.pending)? as u64;
            }
            Source::Stream(lines) => {
                for line in lines.try_iter() {
                    self.pending.extend(line.as_bytes());
                    self.pending.push(b'\n');
                }
            }
        }

        let Some(end) = self.pending.iter().rposition(|byte| *byte == b'\n') else {
            return Ok(Vec::new());
        };

        let complete: Vec<u8> = self.pending.drain(..=end).collect();
        let text = String::from_utf8_lossy(&complete);
        self.settings.mode = detect::resolve_mode(&self.settings.mode, &text);

        Ok(render_viewer_lines(&text, &self.settings, &mut self.entry))
    }
}
