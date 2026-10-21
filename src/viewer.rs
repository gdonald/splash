//! Scrollable viewer state for the interactive output mode
//!
//! The viewer holds rendered lines with the log text each came from and
//! decides which of them are on screen. It knows nothing about the terminal,
//! so the scrolling, searching, and filtering rules can be exercised without
//! one.
use regex::Regex;

/// A key press the viewer understands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    /// A letter pressed with Ctrl
    Ctrl(char),
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Escape,
    Enter,
    Backspace,
}

/// Whether the viewer should keep running after a key press
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Quit,
}

/// The keys the viewer responds to, as shown in its status line
pub const KEY_HELP: &str =
    "j/k:scroll  /:search  n/N:next/prev  &:filter  s:split  f:follow  q:quit";

/// The line between the two panes of a split view
pub const SPLIT_RULE: &str = "---- log text ----";

/// One log line: the text as the log wrote it, and its rendering
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub raw: String,
    pub rendered: String,
}

impl Line {
    pub fn new(raw: &str, rendered: &str) -> Line {
        Line {
            raw: raw.to_string(),
            rendered: rendered.to_string(),
        }
    }
}

/// What the text typed at the prompt is for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Search,
    Filter,
}

/// A filter typed at the `&` prompt
struct LineFilter {
    written: String,
    pattern: Regex,
    /// Whether matching lines are kept, or dropped for a pattern written
    /// after `!`
    keep: bool,
}

pub struct Viewer {
    lines: Vec<Line>,
    /// The positions in `lines` of the lines the filters keep
    shown: Vec<usize>,
    filters: Vec<LineFilter>,
    /// The position in `shown` of the first line on screen
    offset: usize,
    height: usize,
    split: bool,
    following: bool,
    search: Option<Regex>,
    /// The position in `shown` of the last line the search found
    found: Option<usize>,
    prompt: Option<(Purpose, String)>,
    /// A count typed before a command, as in `5j`
    count: Option<usize>,
    /// Whether the last key was the first `g` of `gg`
    pending_g: bool,
    /// A notice shown in the status line until the next key
    notice: Option<String>,
}

impl Viewer {
    /// Creates a viewer over rendered lines, showing `height` lines at a
    /// time, with each rendering standing in for its log text
    pub fn new(rendered: Vec<String>, height: usize) -> Self {
        Viewer::with_lines(
            rendered
                .into_iter()
                .map(|text| Line {
                    raw: text.clone(),
                    rendered: text,
                })
                .collect(),
            height,
        )
    }

    /// Creates a viewer showing `height` lines at a time
    pub fn with_lines(lines: Vec<Line>, height: usize) -> Self {
        Self {
            shown: (0..lines.len()).collect(),
            lines,
            filters: Vec::new(),
            offset: 0,
            height: height.max(1),
            split: false,
            following: false,
            search: None,
            found: None,
            prompt: None,
            count: None,
            pending_g: false,
            notice: None,
        }
    }

    /// Number of screen rows the log lines take
    pub fn height(&self) -> usize {
        self.height
    }

    /// Resizes the viewport, keeping the scroll position in range
    pub fn set_height(&mut self, height: usize) {
        self.height = height.max(1);
        self.clamp_offset();
    }

    /// Number of log lines on screen at once, which is the rows of one pane
    /// when the view is split
    pub fn page(&self) -> usize {
        if self.split {
            (self.height.saturating_sub(1) / 2).max(1)
        } else {
            self.height
        }
    }

    /// Index among the shown lines of the first visible line
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Whether the view is split into rendered and log text panes
    pub fn is_split(&self) -> bool {
        self.split
    }

    /// Whether the view stays on the last lines as new ones arrive
    pub fn is_following(&self) -> bool {
        self.following
    }

    /// The number of lines the filters keep
    pub fn shown(&self) -> usize {
        self.shown.len()
    }

    /// The furthest the viewer can scroll before running out of lines
    pub fn max_offset(&self) -> usize {
        self.shown.len().saturating_sub(self.page())
    }

    fn visible_lines(&self) -> impl Iterator<Item = &Line> {
        self.shown
            .iter()
            .skip(self.offset)
            .take(self.page())
            .map(|index| &self.lines[*index])
    }

    /// The renderings of the lines currently on screen
    pub fn visible(&self) -> Vec<&str> {
        self.visible_lines()
            .map(|line| line.rendered.as_str())
            .collect()
    }

    /// The status line: the prompt being typed, or a notice, or the position,
    /// the filters, the modes that are on, and the available keys
    pub fn status(&self) -> String {
        if let Some((purpose, typed)) = &self.prompt {
            let mark = match purpose {
                Purpose::Search => '/',
                Purpose::Filter => '&',
            };

            return format!("{}{}", mark, typed);
        }

        if let Some(notice) = &self.notice {
            return notice.clone();
        }

        let mut status = if self.shown.is_empty() {
            "no log lines".to_string()
        } else {
            format!(
                "lines {}-{} of {}",
                self.offset + 1,
                (self.offset + self.page()).min(self.shown.len()),
                self.shown.len()
            )
        };

        if let Some(found) = self.found_on_screen() {
            status.push_str(&format!(", match at line {}", found + 1));
        }

        if !self.filters.is_empty() {
            let written: Vec<String> = self
                .filters
                .iter()
                .map(|filter| format!("&{}", filter.written))
                .collect();

            status.push_str(&format!(
                " (filtered from {} by {})",
                self.lines.len(),
                written.join(" ")
            ));
        }

        for (on, name) in [(self.split, "split"), (self.following, "follow")] {
            if on {
                status.push_str(&format!(" [{}]", name));
            }
        }

        format!("{}  {}", status, KEY_HELP)
    }

    /// The visible lines followed by the status line. A split view shows
    /// the renderings, a rule, and the log text of the same lines.
    pub fn frame(&self) -> Vec<String> {
        let mut frame: Vec<String> = self.visible().into_iter().map(str::to_string).collect();

        if self.split {
            frame.resize(self.page(), String::new());
            frame.push(SPLIT_RULE.to_string());
            frame.extend(self.visible_lines().map(|line| line.raw.clone()));
        }

        frame.push(self.status());

        frame
    }

    /// Adds lines that arrived after the viewer opened, staying on the last
    /// lines when following
    pub fn push(&mut self, lines: Vec<Line>) {
        for line in lines {
            if self.keeps(&line) {
                self.shown.push(self.lines.len());
            }

            self.lines.push(line);
        }

        if self.following {
            self.to_bottom();
        }
    }

    /// Scrolls down by `amount` lines, stopping at the last screenful
    pub fn scroll_down(&mut self, amount: usize) {
        self.offset = (self.offset + amount).min(self.max_offset());
    }

    /// Scrolls up by `amount` lines, stopping at the first line, and stops
    /// following
    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
        self.following = false;
    }

    /// Scrolls to the first line
    pub fn to_top(&mut self) {
        self.scroll_up(self.offset);
    }

    /// Scrolls to the last screenful
    pub fn to_bottom(&mut self) {
        self.offset = self.max_offset();
    }

    /// Scrolls so that the shown line with this number, counting from 1, is
    /// the first on screen, or as near the top as it can be
    pub fn go_to(&mut self, number: usize) {
        self.following = false;
        self.offset = number.saturating_sub(1).min(self.max_offset());
    }

    /// Applies a key press, reporting whether the viewer should keep running
    pub fn handle(&mut self, key: Key) -> Outcome {
        if self.prompt.is_some() {
            self.edit_prompt(key);
            return Outcome::Continue;
        }

        if let Key::Char(digit @ '0'..='9') = key {
            if digit != '0' || self.count.is_some() {
                let typed = digit.to_digit(10).unwrap() as usize;
                self.count = Some(self.count.unwrap_or(0).saturating_mul(10) + typed);
                return Outcome::Continue;
            }
        }

        let count = self.count.take();
        let times = count.unwrap_or(1);
        let pending_g = std::mem::take(&mut self.pending_g);
        let half = (self.page() / 2).max(1);
        self.notice = None;

        match key {
            Key::Char('q') | Key::Escape => return Outcome::Quit,
            Key::Char('j') | Key::Down | Key::Enter => self.scroll_down(times),
            Key::Char('k') | Key::Up => self.scroll_up(times),
            Key::Char(' ') | Key::Ctrl('f') | Key::PageDown => {
                self.scroll_down(self.page() * times)
            }
            Key::Char('b') | Key::Ctrl('b') | Key::PageUp => self.scroll_up(self.page() * times),
            Key::Ctrl('d') => self.scroll_down(half * times),
            Key::Ctrl('u') => self.scroll_up(half * times),
            Key::Char('g') if pending_g => self.go_to(times),
            Key::Char('g') => {
                self.pending_g = true;
                self.count = count;
            }
            Key::Home => self.to_top(),
            Key::Char('G') => match count {
                Some(number) => self.go_to(number),
                None => self.to_bottom(),
            },
            Key::End => self.to_bottom(),
            Key::Char('/') => self.prompt = Some((Purpose::Search, String::new())),
            Key::Char('&') => self.prompt = Some((Purpose::Filter, String::new())),
            Key::Char('n') => (0..times).for_each(|_| self.find(true)),
            Key::Char('N') => (0..times).for_each(|_| self.find(false)),
            Key::Char('s') => {
                self.split = !self.split;
                self.clamp_offset();
            }
            Key::Char('f') => {
                self.following = !self.following;

                if self.following {
                    self.to_bottom();
                }
            }
            _ => {}
        }

        Outcome::Continue
    }

    fn edit_prompt(&mut self, key: Key) {
        let (purpose, typed) = self.prompt.as_mut().unwrap();

        match key {
            Key::Char(character) => typed.push(character),
            Key::Backspace => {
                typed.pop();
            }
            Key::Enter => {
                let (purpose, typed) = (*purpose, typed.clone());
                self.prompt = None;

                match purpose {
                    Purpose::Search => self.submit_search(&typed),
                    Purpose::Filter => self.submit_filter(&typed),
                }
            }
            Key::Escape => self.prompt = None,
            _ => {}
        }
    }

    /// Searches forward for a pattern, or for the last pattern again when
    /// none is typed
    fn submit_search(&mut self, typed: &str) {
        if !typed.is_empty() {
            match Regex::new(typed) {
                Ok(pattern) => {
                    self.search = Some(pattern);
                    self.found = None;
                }
                Err(_) => {
                    self.notice = Some(format!("Invalid pattern: {}", typed));
                    return;
                }
            }
        }

        self.find(true);
    }

    /// Adds a filter keeping the lines that match a pattern, or dropping
    /// them for a pattern written after `!`. Submitting nothing removes
    /// every filter.
    fn submit_filter(&mut self, typed: &str) {
        if typed.is_empty() {
            self.filters.clear();
        } else {
            let (keep, written) = match typed.strip_prefix('!') {
                Some(rest) => (false, rest),
                None => (true, typed),
            };

            match Regex::new(written) {
                Ok(pattern) => self.filters.push(LineFilter {
                    written: typed.to_string(),
                    pattern,
                    keep,
                }),
                Err(_) => {
                    self.notice = Some(format!("Invalid pattern: {}", written));
                    return;
                }
            }
        }

        self.shown = (0..self.lines.len())
            .filter(|index| self.keeps(&self.lines[*index]))
            .collect();
        self.found = None;
        self.clamp_offset();
    }

    fn keeps(&self, line: &Line) -> bool {
        self.filters
            .iter()
            .all(|filter| filter.pattern.is_match(&line.raw) == filter.keep)
    }

    /// The position in `shown` of the line the search last found, while it
    /// is on screen
    fn found_on_screen(&self) -> Option<usize> {
        self.found
            .filter(|found| (self.offset..self.offset + self.page()).contains(found))
    }

    /// Finds the next shown line whose log text matches the search, after or
    /// before the last line found while it is on screen, or else the first
    /// line on screen, wrapping around the ends, and scrolls it to the top
    fn find(&mut self, forward: bool) {
        let Some(pattern) = &self.search else {
            self.notice = Some("No search pattern".to_string());
            return;
        };

        let start = self.found_on_screen().unwrap_or(self.offset);
        let count = self.shown.len();
        let found = (1..=count)
            .map(|step| {
                if forward {
                    (start + step) % count
                } else {
                    (start + count * 2 - step) % count
                }
            })
            .find(|position| pattern.is_match(&self.lines[self.shown[*position]].raw));

        match found {
            Some(position) => {
                self.go_to(position + 1);
                self.found = Some(position);
            }
            None => self.notice = Some(format!("Pattern not found: {}", pattern.as_str())),
        }
    }

    fn clamp_offset(&mut self) {
        self.offset = self.offset.min(self.max_offset());
    }
}
