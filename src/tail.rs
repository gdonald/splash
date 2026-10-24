//! New log text for the interactive viewer
//!
//! A tail reads the text a followed file or a stream of lines has gained
//! since it was last read, renders the complete lines, and keeps a partial
//! last line for the next read.
use crate::config::Settings;
use crate::detect;
use crate::filter::Entry;
use crate::render_viewer_lines;
use crate::source::{self, Follower};
use crate::viewer::Line;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

enum Source {
    File(Follower),
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
        Tail::over(Source::File(Follower::new(path, 0)), settings)
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
            Source::File(follower) => follower.read(&mut self.pending)?,
            Source::Stream(lines) => {
                for line in lines.try_iter() {
                    self.pending.extend(line.as_bytes());
                    self.pending.push(b'\n');
                }
            }
        }

        let Some(text) = source::take_complete_lines(&mut self.pending) else {
            return Ok(Vec::new());
        };

        self.settings.mode = detect::resolve_mode(&self.settings.mode, &text);

        Ok(render_viewer_lines(&text, &self.settings, &mut self.entry))
    }
}
