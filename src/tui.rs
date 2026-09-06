//! Terminal plumbing for the interactive viewer
//!
//! Everything here works against a `Write` and a supplied event source, so the
//! drawing and the event loop run without a terminal attached.
use crate::output::OutputMode;
use crate::render_lines;
use crate::viewer::{Key, Outcome, Viewer};
use crossterm::event::{Event, KeyEvent, KeyEventKind};
use crossterm::{cursor, queue, terminal};
use std::io::{self, Write};

/// Message shown when interactive output is asked for outside a terminal
pub const NEEDS_TERMINAL: &str =
    "curses output needs a terminal; use --output ansi, html, json, or plain when redirecting";

/// Builds a viewer over rendered log text, reserving the bottom row for the status line
pub fn viewer_for(contents: &str, mode: &str, output_mode: OutputMode, rows: u16) -> Viewer {
    Viewer::new(
        render_lines(contents, mode, output_mode),
        rows.saturating_sub(1) as usize,
    )
}

/// Translates a terminal key press into a viewer key
pub fn key_for(event: KeyEvent) -> Option<Key> {
    use crossterm::event::KeyCode;

    match event.code {
        KeyCode::Char(c) => Some(Key::Char(c)),
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::PageUp => Some(Key::PageUp),
        KeyCode::PageDown => Some(Key::PageDown),
        KeyCode::Home => Some(Key::Home),
        KeyCode::End => Some(Key::End),
        KeyCode::Esc => Some(Key::Escape),
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

/// Draws and reacts to events until the viewer is quit
pub fn run_loop<W, F>(out: &mut W, viewer: &mut Viewer, next_event: &mut F) -> io::Result<()>
where
    W: Write,
    F: FnMut() -> io::Result<Event>,
{
    loop {
        draw(out, viewer)?;
        let event = next_event()?;

        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if let Some(key) = key_for(key) {
                    if viewer.handle(key) == Outcome::Quit {
                        return Ok(());
                    }
                }
            }
            Event::Resize(_, rows) => viewer.set_height(rows.saturating_sub(1) as usize),
            _ => {}
        }
    }
}
