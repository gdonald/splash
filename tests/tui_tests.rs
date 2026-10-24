use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use splash::config::Settings;
use splash::filter::Entry;
use splash::filter::Filter;
use splash::output::OutputMode;
use splash::render_viewer_lines;
use splash::theme::Theme;
use splash::tui::{draw, key_for, run_loop, viewer_for, NEEDS_INPUT, NEEDS_TERMINAL};
use splash::viewer::{Key, Line, Viewer};
use std::io;

fn press(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn release(code: KeyCode) -> Event {
    let mut key = KeyEvent::new(code, KeyModifiers::NONE);
    key.kind = KeyEventKind::Release;

    Event::Key(key)
}

/// Gives each event in turn, `None` standing for a wait with no event
fn scripted(events: Vec<Option<Event>>) -> impl FnMut() -> io::Result<Option<Event>> {
    let mut remaining = events.into_iter();

    move || {
        remaining
            .next()
            .ok_or_else(|| io::Error::other("no more events"))
    }
}

fn pressed(codes: &[KeyCode]) -> Vec<Option<Event>> {
    codes.iter().map(|code| Some(press(*code))).collect()
}

/// A log that gains nothing
fn no_new_lines() -> impl FnMut() -> io::Result<Vec<Line>> {
    || Ok(Vec::new())
}

fn numbered_viewer(count: usize, height: usize) -> Viewer {
    Viewer::new(
        (1..=count)
            .map(|number| format!("line {}", number))
            .collect(),
        height,
    )
}

fn plain_settings() -> Settings {
    Settings {
        mode: "ad-hoc".to_string(),
        output_mode: OutputMode::Plain,
        theme: Theme::default(),
        jobs: 1,
        filter: Filter::default(),
        dates: Default::default(),
        keywords: Default::default(),
        lookups: Default::default(),
        stats: Default::default(),
        fields: Vec::new(),
        anonymizer: None,
        source: None,
        plugin_themes: Default::default(),
        css: None,
        line_numbers: false,
    }
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
        "curses output needs a terminal; use --output ansi, csv, html, json, jsonl, or plain when redirecting"
    );
}

#[test]
fn a_viewer_reserves_the_bottom_row_for_the_status_line() {
    let lines = render_viewer_lines("a\nb\nc\nd\n", &plain_settings(), &mut Entry::default());
    let viewer = viewer_for(lines, 3);

    assert_eq!(viewer.height(), 2);
    assert_eq!(viewer.visible(), ["a", "b"]);
}

#[test]
fn a_viewer_on_a_terminal_with_no_rows_still_shows_one_line() {
    let viewer = viewer_for(Vec::new(), 0);

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
        (KeyCode::Enter, Key::Enter),
        (KeyCode::Backspace, Key::Backspace),
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
    let mut events = scripted(pressed(&[KeyCode::Char('j'), KeyCode::Char('q')]));

    assert!(run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).is_ok());
    assert_eq!(viewer.offset(), 1);
}

#[test]
fn the_loop_draws_a_frame_for_every_key() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(pressed(&[KeyCode::Char('j'), KeyCode::Char('q')]));

    run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).unwrap();
    let output = String::from_utf8(buffer).unwrap();

    assert_eq!(output.matches("lines ").count(), 2);
}

#[test]
fn the_loop_resizes_the_viewport_on_a_resize_event() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![
        Some(Event::Resize(80, 6)),
        Some(press(KeyCode::Char('q'))),
    ]);

    run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).unwrap();

    assert_eq!(viewer.height(), 5);
}

#[test]
fn the_loop_ignores_key_releases() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![
        Some(release(KeyCode::Char('j'))),
        Some(press(KeyCode::Char('q'))),
    ]);

    run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).unwrap();

    assert_eq!(viewer.offset(), 0);
}

#[test]
fn the_loop_ignores_keys_the_viewer_does_not_use() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(pressed(&[KeyCode::Tab, KeyCode::Char('q')]));

    run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).unwrap();

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
    let mut events = scripted(vec![Some(mouse), Some(press(KeyCode::Char('q')))]);

    run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).unwrap();

    assert_eq!(viewer.offset(), 0);
}

#[test]
fn the_loop_reports_an_event_source_failure() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![]);

    let result = run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines());

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
    let mut events = scripted(pressed(&[KeyCode::Char('q')]));

    let result = run_loop(
        &mut FailingWriter,
        &mut viewer,
        &mut events,
        &mut no_new_lines(),
    );

    assert_eq!(result.unwrap_err().to_string(), "terminal is gone");
}

#[test]
fn the_input_requirement_names_the_ways_to_give_a_log() {
    assert_eq!(
        NEEDS_INPUT,
        "curses output needs a log; give --path or pipe one in"
    );
}

#[test]
fn ctrl_letters_map_to_ctrl_keys() {
    assert_eq!(
        key_for(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        Some(Key::Ctrl('d'))
    );
}

#[test]
fn the_loop_adds_the_lines_the_log_gains_while_waiting() {
    let mut viewer = numbered_viewer(2, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![None, Some(press(KeyCode::Char('q')))]);
    let mut refresh = || Ok(vec![Line::new("line 3", "line 3")]);

    run_loop(&mut buffer, &mut viewer, &mut events, &mut refresh).unwrap();

    assert_eq!(viewer.visible(), ["line 1", "line 2", "line 3"]);
    assert!(String::from_utf8(buffer)
        .unwrap()
        .contains("lines 1-3 of 3"));
}

#[test]
fn the_loop_draws_nothing_new_when_the_log_gains_nothing() {
    let mut viewer = numbered_viewer(2, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(vec![None, None, Some(press(KeyCode::Char('q')))]);

    run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).unwrap();

    assert_eq!(
        String::from_utf8(buffer).unwrap().matches("lines ").count(),
        1
    );
}

#[test]
fn the_loop_draws_nothing_new_for_an_ignored_event() {
    let mut viewer = numbered_viewer(10, 3);
    let mut buffer: Vec<u8> = Vec::new();
    let mut events = scripted(pressed(&[KeyCode::Tab, KeyCode::Char('q')]));

    run_loop(&mut buffer, &mut viewer, &mut events, &mut no_new_lines()).unwrap();

    assert_eq!(
        String::from_utf8(buffer).unwrap().matches("lines ").count(),
        1
    );
}

#[test]
fn the_loop_reports_a_failure_to_read_the_log() {
    let mut viewer = numbered_viewer(2, 3);
    let mut events = scripted(vec![None]);
    let mut refresh = || Err(io::Error::other("log is gone"));

    let result = run_loop(&mut Vec::new(), &mut viewer, &mut events, &mut refresh);

    assert_eq!(result.unwrap_err().to_string(), "log is gone");
}
