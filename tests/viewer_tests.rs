use splash::viewer::{Key, Outcome, Viewer, KEY_HELP};

fn numbered_lines(count: usize) -> Vec<String> {
    (1..=count)
        .map(|number| format!("line {}", number))
        .collect()
}

fn viewer(count: usize, height: usize) -> Viewer {
    Viewer::new(numbered_lines(count), height)
}

#[test]
fn a_new_viewer_starts_at_the_first_line() {
    let subject = viewer(10, 4);

    assert_eq!(subject.offset(), 0);
    assert_eq!(subject.visible(), ["line 1", "line 2", "line 3", "line 4"]);
}

#[test]
fn a_viewport_is_never_shorter_than_one_line() {
    let subject = viewer(10, 0);

    assert_eq!(subject.height(), 1);
}

#[test]
fn scrolling_down_moves_the_window_by_one_line() {
    let mut subject = viewer(10, 3);

    subject.scroll_down(1);

    assert_eq!(subject.visible(), ["line 2", "line 3", "line 4"]);
}

#[test]
fn scrolling_down_stops_at_the_last_screenful() {
    let mut subject = viewer(10, 3);

    subject.scroll_down(100);

    assert_eq!(subject.offset(), 7);
    assert_eq!(subject.visible(), ["line 8", "line 9", "line 10"]);
}

#[test]
fn scrolling_up_stops_at_the_first_line() {
    let mut subject = viewer(10, 3);

    subject.scroll_down(2);
    subject.scroll_up(100);

    assert_eq!(subject.offset(), 0);
}

#[test]
fn a_viewer_showing_every_line_cannot_scroll() {
    let mut subject = viewer(3, 10);

    subject.scroll_down(5);

    assert_eq!(subject.max_offset(), 0);
    assert_eq!(subject.offset(), 0);
}

#[test]
fn jumping_to_the_bottom_shows_the_last_screenful() {
    let mut subject = viewer(10, 4);

    subject.to_bottom();

    assert_eq!(subject.visible(), ["line 7", "line 8", "line 9", "line 10"]);
}

#[test]
fn jumping_to_the_top_shows_the_first_screenful() {
    let mut subject = viewer(10, 4);

    subject.to_bottom();
    subject.to_top();

    assert_eq!(subject.offset(), 0);
}

#[test]
fn a_taller_viewport_pulls_the_scroll_position_back() {
    let mut subject = viewer(10, 2);

    subject.to_bottom();
    subject.set_height(10);

    assert_eq!(subject.offset(), 0);
    assert_eq!(subject.visible().len(), 10);
}

#[test]
fn a_shorter_viewport_keeps_the_scroll_position() {
    let mut subject = viewer(10, 5);

    subject.scroll_down(2);
    subject.set_height(3);

    assert_eq!(subject.offset(), 2);
    assert_eq!(subject.visible(), ["line 3", "line 4", "line 5"]);
}

#[test]
fn an_empty_viewer_shows_nothing() {
    let subject = Viewer::new(vec![], 5);

    assert!(subject.visible().is_empty());
}

#[test]
fn the_status_line_reports_the_visible_range() {
    let mut subject = viewer(10, 4);

    subject.scroll_down(1);

    assert_eq!(subject.status(), format!("lines 2-5 of 10  {}", KEY_HELP));
}

#[test]
fn the_status_line_reports_a_partial_last_screenful() {
    let subject = viewer(3, 10);

    assert_eq!(subject.status(), format!("lines 1-3 of 3  {}", KEY_HELP));
}

#[test]
fn the_status_line_reports_an_empty_viewer() {
    let subject = Viewer::new(vec![], 5);

    assert_eq!(subject.status(), format!("no log lines  {}", KEY_HELP));
}

#[test]
fn a_frame_is_the_visible_lines_followed_by_the_status_line() {
    let subject = viewer(2, 5);

    assert_eq!(
        subject.frame(),
        vec![
            "line 1".to_string(),
            "line 2".to_string(),
            format!("lines 1-2 of 2  {}", KEY_HELP),
        ]
    );
}

#[test]
fn q_and_escape_quit_the_viewer() {
    let mut subject = viewer(10, 4);

    assert_eq!(subject.handle(Key::Char('q')), Outcome::Quit);
    assert_eq!(subject.handle(Key::Escape), Outcome::Quit);
}

#[test]
fn j_and_the_down_arrow_scroll_down_one_line() {
    let mut subject = viewer(10, 4);

    assert_eq!(subject.handle(Key::Char('j')), Outcome::Continue);
    subject.handle(Key::Down);

    assert_eq!(subject.offset(), 2);
}

#[test]
fn k_and_the_up_arrow_scroll_up_one_line() {
    let mut subject = viewer(10, 4);

    subject.scroll_down(4);
    subject.handle(Key::Char('k'));
    subject.handle(Key::Up);

    assert_eq!(subject.offset(), 2);
}

#[test]
fn space_f_and_page_down_scroll_a_full_screen() {
    let mut subject = viewer(100, 10);

    subject.handle(Key::Char(' '));
    assert_eq!(subject.offset(), 10);

    subject.handle(Key::Char('f'));
    assert_eq!(subject.offset(), 20);

    subject.handle(Key::PageDown);
    assert_eq!(subject.offset(), 30);
}

#[test]
fn b_and_page_up_scroll_back_a_full_screen() {
    let mut subject = viewer(100, 10);

    subject.scroll_down(50);
    subject.handle(Key::Char('b'));
    assert_eq!(subject.offset(), 40);

    subject.handle(Key::PageUp);
    assert_eq!(subject.offset(), 30);
}

#[test]
fn g_and_home_jump_to_the_top() {
    let mut subject = viewer(100, 10);

    subject.scroll_down(50);
    subject.handle(Key::Char('g'));
    assert_eq!(subject.offset(), 0);

    subject.scroll_down(50);
    subject.handle(Key::Home);
    assert_eq!(subject.offset(), 0);
}

#[test]
fn shift_g_and_end_jump_to_the_bottom() {
    let mut subject = viewer(100, 10);

    subject.handle(Key::Char('G'));
    assert_eq!(subject.offset(), 90);

    subject.to_top();
    subject.handle(Key::End);
    assert_eq!(subject.offset(), 90);
}

#[test]
fn an_unhandled_key_leaves_the_viewer_alone() {
    let mut subject = viewer(10, 4);

    subject.scroll_down(1);

    assert_eq!(subject.handle(Key::Char('z')), Outcome::Continue);
    assert_eq!(subject.offset(), 1);
}
