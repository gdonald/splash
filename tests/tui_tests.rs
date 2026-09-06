use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use splash::output::OutputMode;
use splash::tui::{draw, key_for, run_loop, viewer_for, NEEDS_TERMINAL};
use splash::viewer::{Key, Viewer};
use std::io;

fn press(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn release(code: KeyCode) -> Event {
    let mut key = KeyEvent::new(code, KeyModifiers::NONE);
    key.kind = KeyEventKind::Release;

    Event::Key(key)
}

fn scripted(events: Vec<Event>) -> impl FnMut() -> io::Result<Event> {
    let mut remaining = events.into_iter();

    move || {
        remaining
            .next()
            .ok_or_else(|| io::Error::other("no more events"))
    }
}

fn numbered_viewer(count: usize, height: usize) -> Viewer {
    Viewer::new(
        (1..=count)
            .map(|number| format!("line {}", number))
            .collect(),
        height,
    )
}

fn rendered(viewer: &Viewer) -> String {
    let mut buffer: Vec<u8> = Vec::new();
    draw(&mut buffer, viewer).unwrap();

    String::from_utf8(buffer).unwrap()
}

#[test]
fn the_terminal_requirement_names_the_other_output_modes() {
    assert_eq!(
        NEEDS_TERMINAL,
        "curses output needs a terminal; use --output ansi, html, json, or plain when redirecting"
    );
}

#[test]
fn a_viewer_reserves_the_bottom_row_for_the_status_line() {
    let viewer = viewer_for("a\nb\nc\nd\n", "ad-hoc", OutputMode::Plain, 3);

    assert_eq!(viewer.height(), 2);
    assert_eq!(viewer.visible(), ["a", "b"]);
}

#[test]
fn a_viewer_on_a_terminal_with_no_rows_still_shows_one_line() {
    let viewer = viewer_for("a\nb\n", "ad-hoc", OutputMode::Plain, 0);

    assert_eq!(viewer.height(), 1);
}

#[test]
fn character_keys_map_to_viewer_keys() {
    assert_eq!(
        key_for(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)),
        Some(Key::Char('j'))
    );
}

#[test]
fn navigation_keys_map_to_viewer_keys() {
    let mappings = [
        (KeyCode::Up, Key::Up),
        (KeyCode::Down, Key::Down),
        (KeyCode::PageUp, Key::PageUp),
        (KeyCode::PageDown, Key::PageDown),
        (KeyCode::Home, Key::Home),
        (KeyCode::End, Key::End),
        (KeyCode::Esc, Key::Escape),
    ];

    for (code, expected) in mappings {
        assert_eq!(
            key_for(KeyEvent::new(code, KeyModifiers::NONE)),
            Some(expected),
            "{:?} should map to {:?}",
            code,
            expected
        );
    }
}

#[test]
fn an_unused_key_maps_to_nothing() {
    assert_eq!(
        key_for(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        None
    );
}

#[test]
fn drawing_writes_the_visible_lines_and_the_status_line() {
    let output = rendered(&numbered_viewer(5, 2));

    assert!(output.contains("line 1\r\n"));
    assert!(output.contains("line 2\r\n"));
    assert!(output.contains("lines 1-2 of 5"));
    assert!(!output.contains("line 3"));
}

#[test]
fn drawing_clears_the_screen_first() {
    let output = rendered(&numbered_viewer(1, 1));

    assert!(output.starts_with("\u{1b}[2J"), "frame was: {:?}", output);
}

#[test]
fn the_loop_ends_when_the_viewer_is_quit() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![press(KeyCode::Char('j')), press(KeyCode::Char('q'))]);

    assert!(run_loop(&mut buffer, &mut viewer, &mut events).is_ok());
    assert_eq!(viewer.offset(), 1);
}

#[test]
fn the_loop_draws_a_frame_for_every_event() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![press(KeyCode::Char('j')), press(KeyCode::Char('q'))]);

    run_loop(&mut buffer, &mut viewer, &mut events).unwrap();
    let output = String::from_utf8(buffer).unwrap();

    assert_eq!(output.matches("lines ").count(), 2);
}

#[test]
fn the_loop_resizes_the_viewport_on_a_resize_event() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![Event::Resize(80, 6), press(KeyCode::Char('q'))]);

    run_loop(&mut buffer, &mut viewer, &mut events).unwrap();

    assert_eq!(viewer.height(), 5);
}

#[test]
fn the_loop_ignores_key_releases() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![release(KeyCode::Char('j')), press(KeyCode::Char('q'))]);

    run_loop(&mut buffer, &mut viewer, &mut events).unwrap();

    assert_eq!(viewer.offset(), 0);
}

#[test]
fn the_loop_ignores_keys_the_viewer_does_not_use() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![press(KeyCode::Tab), press(KeyCode::Char('q'))]);

    run_loop(&mut buffer, &mut viewer, &mut events).unwrap();

    assert_eq!(viewer.offset(), 0);
}

#[test]
fn the_loop_ignores_events_that_are_not_keys_or_resizes() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mouse = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::NONE,
    });
    let mut events = scripted(vec![mouse, press(KeyCode::Char('q'))]);

    run_loop(&mut buffer, &mut viewer, &mut events).unwrap();

    assert_eq!(viewer.offset(), 0);
}

#[test]
fn the_loop_reports_an_event_source_failure() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![]);

    let result = run_loop(&mut buffer, &mut viewer, &mut events);

    assert_eq!(result.unwrap_err().to_string(), "no more events");
}

struct FailingWriter;

impl io::Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("terminal is gone"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("terminal is gone"))
    }
}

#[test]
fn drawing_reports_a_write_failure() {
    let viewer = numbered_viewer(5, 2);

    let result = draw(&mut FailingWriter, &viewer);

    assert_eq!(result.unwrap_err().to_string(), "terminal is gone");
}

#[test]
fn the_loop_reports_a_draw_failure() {
    let mut viewer = numbered_viewer(5, 2);
    let mut events = scripted(vec![press(KeyCode::Char('q'))]);

    let result = run_loop(&mut FailingWriter, &mut viewer, &mut events);

    assert_eq!(result.unwrap_err().to_string(), "terminal is gone");
}
