//! Terminal plumbing for the interactive viewer
//!
//! Everything here works against a `Write` and a supplied event source, so the
//! drawing and the event loop run without a terminal attached.
use crate::viewer::{Key, Line, Outcome, Viewer};
use crossterm::event::{Event, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, queue, terminal};
use std::io::{self, Write};

/// Message shown when interactive output is asked for outside a terminal
pub const NEEDS_TERMINAL: &str =
    "curses output needs a terminal; use --output ansi, html, json, or plain when redirecting";

/// Message shown when interactive output is asked for with no log to show
pub const NEEDS_INPUT: &str = "curses output needs a log; give --path or pipe one in";

/// Builds a viewer over rendered lines, reserving the bottom row for the status line
pub fn viewer_for(lines: Vec<Line>, rows: u16) -> Viewer {
    Viewer::with_lines(lines, rows.saturating_sub(1) as usize)
}

/// Translates a terminal key press into a viewer key
pub fn key_for(event: KeyEvent) -> Option<Key> {
    use crossterm::event::KeyCode;

    match event.code {
        KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => Some(Key::Ctrl(c)),
        KeyCode::Char(c) => Some(Key::Char(c)),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::Home => Some(Key::Home),
        KeyCode::End => Some(Key::End),
        KeyCode::Esc => Some(Key::Escape),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Backspace => Some(Key::Backspace),
        _ => None,
    }
}

/// Paints the current frame, clearing what the previous frame left behind
pub fn draw<W: Write>(out: &mut W, viewer: &Viewer) -> io::Result<()> {
    queue!(out, terminal::Clear(terminal::ClearType::All))?;
    queue!(out, cursor::MoveTo(0, 0))?;

    for line in viewer.frame() {
        queue!(out, terminal::Clear(terminal::ClearType::CurrentLine))?;
        write!(out, "{}\r\n", line)?;
    }

    out.flush()
}

/// Draws and reacts to events until the viewer is quit. `next_event` gives
/// `None` when no event arrived in time, and then `refresh` gives the lines
/// the log gained. A frame is drawn first and after anything that changes it.
pub fn run_loop<W, F, R>(
    out: &mut W,
    viewer: &mut Viewer,
    next_event: &mut F,
    refresh: &mut R,
) -> io::Result<()>
where
    W: Write,
    F: FnMut() -> io::Result<Option<Event>>,
    R: FnMut() -> io::Result<Vec<Line>>,
{
    draw(out, viewer)?;

    loop {
        let changed = match next_event()? {
            Some(Event::Key(key)) if key.kind == KeyEventKind::Press => match key_for(key) {
                Some(key) if viewer.handle(key) == Outcome::Quit => return Ok(()),
                Some(_) => true,
                None => false,
            },
            Some(Event::Resize(_, rows)) => {
                viewer.set_height(rows.saturating_sub(1) as usize);
                true
            }
            Some(_) => false,
            None => take_new_lines(viewer, refresh()?),
        };

        if changed {
            draw(out, viewer)?;
        }
    }
}

/// Adds the lines the log gained, reporting whether there were any
fn take_new_lines(viewer: &mut Viewer, lines: Vec<Line>) -> bool {
    let gained = !lines.is_empty();
    viewer.push(lines);

    gained
}
