//! Rate limiting for high-volume streams
//!
//! At most a set number of input lines are kept in each one-second window,
//! and the rest are dropped and counted. A window starts with the first line
//! that arrives after the window before it ended.
use std::time::{Duration, Instant};

/// How long each window of the limit lasts
pub const WINDOW: Duration = Duration::from_secs(1);

pub struct RateLimit {
    per_second: usize,
    window_start: Option<Instant>,
    kept: usize,
    dropped: usize,
}

/// The lines a call kept, and the count of lines dropped in a window that
/// ended before the call
pub struct Admitted {
    pub text: String,
    pub dropped: Option<usize>,
}

impl RateLimit {
    pub fn new(per_second: usize) -> RateLimit {
        RateLimit {
            per_second,
            window_start: None,
            kept: 0,
            dropped: 0,
        }
    }

    /// Keeps the lines of `text` that fit in the window holding `now`, each
    /// with its line ending
    pub fn admit(&mut self, text: &str, now: Instant) -> Admitted {
        let dropped = match self.window_start {
            Some(start) if now.duration_since(start) >= WINDOW => self.finish(),
            _ => None,
        };

        self.window_start.get_or_insert(now);

        let mut kept = String::new();

        for line in text.split_inclusive('\n') {
            if self.kept < self.per_second {
                kept.push_str(line);
                self.kept += 1;
            } else {
                self.dropped += 1;
            }
        }

        Admitted {
            text: kept,
            dropped,
        }
    }

    /// Ends the current window, returning the count of lines it dropped when
    /// it dropped any
    pub fn finish(&mut self) -> Option<usize> {
        let dropped = std::mem::take(&mut self.dropped);

        self.window_start = None;
        self.kept = 0;

        (dropped > 0).then_some(dropped)
    }

    /// The message reporting lines a window dropped
    pub fn notice(&self, dropped: usize) -> String {
        let lines = if dropped == 1 { "line" } else { "lines" };

        format!(
            "Dropped {} {} over the limit of {} per second",
            dropped, lines, self.per_second
        )
    }
}
