//! Scrollable viewer state for the interactive output mode
//!
//! The viewer holds already rendered lines and decides which of them are on
//! screen. It knows nothing about the terminal, so the scrolling rules can be
//! exercised without one.

/// A key press the viewer understands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Escape,
}

/// Whether the viewer should keep running after a key press
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
}

/// The keys the viewer responds to, as shown in its status line
pub const KEY_HELP: &str = "j/k:scroll  f/b:page  g/G:top/bottom  q:quit";

pub struct Viewer {
    lines: Vec<String>,
    offset: usize,
    height: usize,
}

impl Viewer {
    /// Creates a viewer showing `height` lines at a time
    pub fn new(lines: Vec<String>, height: usize) -> Self {
        Self {
            lines,
            offset: 0,
            height: height.max(1),
        }
    }

    /// Number of log lines on screen at once
    pub fn height(&self) -> usize {
        self.height
    }

    /// Resizes the viewport, keeping the scroll position in range
    pub fn set_height(&mut self, height: usize) {
        self.height = height.max(1);
        self.clamp_offset();
    }

    /// Index of the first visible line
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The furthest the viewer can scroll before running out of lines
    pub fn max_offset(&self) -> usize {
        self.lines.len().saturating_sub(self.height)
    }

    /// The lines currently on screen
    pub fn visible(&self) -> &[String] {
        let end = (self.offset + self.height).min(self.lines.len());

        &self.lines[self.offset.min(end)..end]
    }

    /// The status line describing position and available keys
    pub fn status(&self) -> String {
        if self.lines.is_empty() {
            return format!("no log lines  {}", KEY_HELP);
        }

        let first = self.offset + 1;
        let last = (self.offset + self.height).min(self.lines.len());

        format!(
            "lines {}-{} of {}  {}",
            first,
            last,
            self.lines.len(),
            KEY_HELP
        )
    }

    /// The visible lines followed by the status line
    pub fn frame(&self) -> Vec<String> {
        let mut frame: Vec<String> = self.visible().to_vec();
        frame.push(self.status());

        frame
    }

    /// Scrolls down by `amount` lines, stopping at the last screenful
    pub fn scroll_down(&mut self, amount: usize) {
        self.offset = (self.offset + amount).min(self.max_offset());
    }

    /// Scrolls up by `amount` lines, stopping at the first line
    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
    }

    /// Scrolls to the first line
    pub fn to_top(&mut self) {
        self.offset = 0;
    }

    /// Scrolls to the last screenful
    pub fn to_bottom(&mut self) {
        self.offset = self.max_offset();
    }

    /// Applies a key press, reporting whether the viewer should keep running
    pub fn handle(&mut self, key: Key) -> Outcome {
        match key {
            Key::Char('q') | Key::Escape => return Outcome::Quit,
            Key::Char('j') | Key::Down => self.scroll_down(1),
            Key::Char('k') | Key::Up => self.scroll_up(1),
            Key::Char(' ') | Key::Char('f') | Key::PageDown => self.scroll_down(self.height),
            Key::Char('b') | Key::PageUp => self.scroll_up(self.height),
            Key::Char('g') | Key::Home => self.to_top(),
            Key::Char('G') | Key::End => self.to_bottom(),
            Key::Char(_) => {}
        }

        Outcome::Continue
    }

    fn clamp_offset(&mut self) {
        self.offset = self.offset.min(self.max_offset());
    }
}
