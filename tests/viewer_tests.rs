use splash::viewer::{Key, Line, Outcome, Viewer, KEY_HELP, SPLIT_RULE};

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
fn space_ctrl_f_and_page_down_scroll_a_full_screen() {
    let mut subject = viewer(100, 10);

    subject.handle(Key::Char(' '));
    assert_eq!(subject.offset(), 10);

    subject.handle(Key::Ctrl('f'));
    assert_eq!(subject.offset(), 20);

    subject.handle(Key::PageDown);
    assert_eq!(subject.offset(), 30);
}

#[test]
fn b_ctrl_b_and_page_up_scroll_back_a_full_screen() {
    let mut subject = viewer(100, 10);

    subject.scroll_down(50);
    subject.handle(Key::Char('b'));
    assert_eq!(subject.offset(), 40);

    subject.handle(Key::Ctrl('b'));
    assert_eq!(subject.offset(), 30);

    subject.handle(Key::PageUp);
    assert_eq!(subject.offset(), 20);
}

#[test]
fn gg_and_home_jump_to_the_top() {
    let mut subject = viewer(100, 10);

    subject.scroll_down(50);
    subject.handle(Key::Char('g'));
    assert_eq!(subject.offset(), 50);

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

/// A viewer over lines whose log text differs from their rendering
fn viewer_over(raw: &[&str], height: usize) -> Viewer {
    Viewer::with_lines(
        raw.iter()
            .map(|text| Line::new(text, &format!("<{}>", text)))
            .collect(),
        height,
    )
}

fn type_keys(subject: &mut Viewer, keys: &str) {
    for character in keys.chars() {
        subject.handle(Key::Char(character));
    }
}

fn search(subject: &mut Viewer, pattern: &str) {
    type_keys(subject, "/");
    type_keys(subject, pattern);
    subject.handle(Key::Enter);
}

fn filter(subject: &mut Viewer, pattern: &str) {
    type_keys(subject, "&");
    type_keys(subject, pattern);
    subject.handle(Key::Enter);
}

const SERVICE_LOG: [&str; 8] = [
    "GET /healthz 200",
    "GET /index.html 200",
    "ERROR disk full",
    "GET /healthz 200",
    "POST /login 302",
    "ERROR timeout",
    "GET /healthz 200",
    "GET /about.html 200",
];

#[test]
fn a_count_repeats_a_scroll() {
    let mut subject = viewer(100, 10);

    type_keys(&mut subject, "12j");
    assert_eq!(subject.offset(), 12);

    type_keys(&mut subject, "5k");
    assert_eq!(subject.offset(), 7);
}

#[test]
fn a_count_can_hold_a_zero_after_its_first_digit() {
    let mut subject = viewer(100, 10);

    type_keys(&mut subject, "10j");

    assert_eq!(subject.offset(), 10);
}

#[test]
fn a_zero_with_no_count_before_it_is_ignored() {
    let mut subject = viewer(100, 10);

    type_keys(&mut subject, "0j");

    assert_eq!(subject.offset(), 1);
}

#[test]
fn a_count_before_shift_g_goes_to_that_line() {
    let mut subject = viewer(100, 10);

    type_keys(&mut subject, "42G");

    assert_eq!(subject.offset(), 41);
}

#[test]
fn a_count_before_gg_goes_to_that_line() {
    let mut subject = viewer(100, 10);

    type_keys(&mut subject, "7gg");

    assert_eq!(subject.offset(), 6);
}

#[test]
fn going_to_a_line_past_the_end_shows_the_last_screenful() {
    let mut subject = viewer(100, 10);

    type_keys(&mut subject, "500G");

    assert_eq!(subject.offset(), 90);
}

#[test]
fn ctrl_d_and_ctrl_u_scroll_half_a_screen() {
    let mut subject = viewer(100, 10);

    subject.handle(Key::Ctrl('d'));
    subject.handle(Key::Ctrl('d'));
    assert_eq!(subject.offset(), 10);

    subject.handle(Key::Ctrl('u'));
    assert_eq!(subject.offset(), 5);
}

#[test]
fn enter_scrolls_down_one_line() {
    let mut subject = viewer(100, 10);

    subject.handle(Key::Enter);

    assert_eq!(subject.offset(), 1);
}

#[test]
fn a_search_scrolls_the_next_matching_line_to_the_top() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "ERROR");

    assert_eq!(
        subject.visible(),
        ["<ERROR disk full>", "<GET /healthz 200>"]
    );
}

#[test]
fn a_search_matches_the_log_text_and_not_the_rendering() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "<");

    assert_eq!(subject.status(), "Pattern not found: <");
}

#[test]
fn n_finds_the_next_match_and_wraps_around() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "ERROR");
    subject.handle(Key::Char('n'));
    assert_eq!(subject.offset(), 5);

    subject.handle(Key::Char('n'));
    assert_eq!(subject.offset(), 2);
}

#[test]
fn shift_n_finds_the_previous_match() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "healthz");
    assert_eq!(subject.offset(), 3);

    subject.handle(Key::Char('N'));
    assert_eq!(subject.offset(), 0);
}

#[test]
fn a_count_before_n_skips_matches() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "healthz");
    type_keys(&mut subject, "2n");

    assert_eq!(subject.offset(), 0);
}

#[test]
fn n_moves_past_a_match_in_the_last_screenful() {
    let mut subject = viewer_over(&SERVICE_LOG, 4);

    search(&mut subject, "html");
    subject.handle(Key::Char('n'));
    assert_eq!(
        subject.status().split("  ").next().unwrap(),
        "lines 5-8 of 8, match at line 8"
    );

    subject.handle(Key::Char('n'));
    assert_eq!(subject.offset(), 1);
}

#[test]
fn the_status_line_names_the_line_a_search_found() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "timeout");

    assert_eq!(
        subject.status(),
        format!("lines 6-7 of 8, match at line 6  {}", KEY_HELP)
    );
}

#[test]
fn an_empty_search_repeats_the_last_one() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "ERROR");
    search(&mut subject, "");

    assert_eq!(subject.offset(), 5);
}

#[test]
fn n_without_a_search_says_so() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    subject.handle(Key::Char('n'));

    assert_eq!(subject.status(), "No search pattern");
}

#[test]
fn a_notice_lasts_until_the_next_key() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    subject.handle(Key::Char('n'));
    subject.handle(Key::Char('j'));

    assert!(subject.status().starts_with("lines 2-3 of 8"));
}

#[test]
fn an_invalid_search_pattern_is_reported() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    search(&mut subject, "(");

    assert_eq!(subject.status(), "Invalid pattern: (");
}

#[test]
fn searching_an_empty_viewer_finds_nothing() {
    let mut subject = Viewer::new(vec![], 2);

    search(&mut subject, "x");

    assert_eq!(subject.status(), "Pattern not found: x");
}

#[test]
fn the_status_line_shows_the_prompt_being_typed() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    type_keys(&mut subject, "/err");
    assert_eq!(subject.status(), "/err");

    subject.handle(Key::Backspace);
    assert_eq!(subject.status(), "/er");

    type_keys(&mut subject, "&");
    assert_eq!(subject.status(), "/er&");
}

#[test]
fn the_prompt_ignores_keys_that_do_not_type() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    type_keys(&mut subject, "&x");
    subject.handle(Key::Down);
    subject.handle(Key::Ctrl('d'));

    assert_eq!((subject.status(), subject.offset()), ("&x".to_string(), 0));
}

#[test]
fn escape_cancels_the_prompt_without_quitting() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    type_keys(&mut subject, "/ERROR");

    assert_eq!(subject.handle(Key::Escape), Outcome::Continue);
    assert_eq!(subject.offset(), 0);
    assert!(subject.status().starts_with("lines 1-2 of 8"));
}

#[test]
fn a_filter_keeps_only_matching_lines() {
    let mut subject = viewer_over(&SERVICE_LOG, 10);

    filter(&mut subject, "ERROR");

    assert_eq!(subject.visible(), ["<ERROR disk full>", "<ERROR timeout>"]);
}

#[test]
fn a_filter_after_an_exclamation_mark_drops_matching_lines() {
    let mut subject = viewer_over(&SERVICE_LOG, 10);

    filter(&mut subject, "!healthz");

    assert_eq!(subject.shown(), 5);
}

#[test]
fn filters_combine() {
    let mut subject = viewer_over(&SERVICE_LOG, 10);

    filter(&mut subject, "GET");
    filter(&mut subject, "!healthz");

    assert_eq!(
        subject.visible(),
        ["<GET /index.html 200>", "<GET /about.html 200>"]
    );
}

#[test]
fn the_status_line_lists_the_filters() {
    let mut subject = viewer_over(&SERVICE_LOG, 10);

    filter(&mut subject, "GET");
    filter(&mut subject, "!healthz");

    assert_eq!(
        subject.status(),
        format!(
            "lines 1-2 of 2 (filtered from 8 by &GET &!healthz)  {}",
            KEY_HELP
        )
    );
}

#[test]
fn an_empty_filter_removes_every_filter() {
    let mut subject = viewer_over(&SERVICE_LOG, 10);

    filter(&mut subject, "ERROR");
    filter(&mut subject, "");

    assert_eq!(subject.shown(), 8);
}

#[test]
fn an_invalid_filter_pattern_is_reported_and_not_applied() {
    let mut subject = viewer_over(&SERVICE_LOG, 10);

    filter(&mut subject, "!(");

    assert_eq!(subject.status(), "Invalid pattern: (");
    assert_eq!(subject.shown(), 8);
}

#[test]
fn filtering_pulls_the_scroll_position_back_into_range() {
    let mut subject = viewer_over(&SERVICE_LOG, 2);

    subject.to_bottom();
    filter(&mut subject, "ERROR");

    assert_eq!(subject.offset(), 0);
}

#[test]
fn a_search_finds_only_lines_the_filters_keep() {
    let mut subject = viewer_over(&SERVICE_LOG, 1);

    filter(&mut subject, "!disk");
    search(&mut subject, "ERROR");

    assert_eq!(subject.visible(), ["<ERROR timeout>"]);
}

#[test]
fn s_splits_the_view_into_rendered_and_log_text_panes() {
    let mut subject = viewer_over(&["alpha", "beta", "gamma"], 5);

    subject.handle(Key::Char('s'));

    assert!(subject.is_split());
    assert_eq!(
        subject.frame(),
        vec![
            "<alpha>".to_string(),
            "<beta>".to_string(),
            SPLIT_RULE.to_string(),
            "alpha".to_string(),
            "beta".to_string(),
            format!("lines 1-2 of 3 [split]  {}", KEY_HELP),
        ]
    );
}

#[test]
fn a_split_pane_with_fewer_lines_than_rows_is_padded() {
    let mut subject = viewer_over(&["alpha"], 5);

    subject.handle(Key::Char('s'));

    assert_eq!(subject.frame()[..4], ["<alpha>", "", SPLIT_RULE, "alpha"]);
}

#[test]
fn a_split_pane_is_never_shorter_than_one_line() {
    let mut subject = viewer_over(&SERVICE_LOG, 1);

    subject.handle(Key::Char('s'));

    assert_eq!(subject.page(), 1);
}

#[test]
fn s_again_joins_the_panes() {
    let mut subject = viewer_over(&SERVICE_LOG, 5);

    subject.handle(Key::Char('s'));
    subject.scroll_down(100);
    subject.handle(Key::Char('s'));

    assert!(!subject.is_split());
    assert_eq!(subject.offset(), 3);
}

#[test]
fn splitting_the_view_keeps_the_first_line_on_screen() {
    let mut subject = viewer_over(&SERVICE_LOG, 5);

    subject.scroll_down(100);
    subject.handle(Key::Char('s'));

    assert_eq!(subject.offset(), 3);
}

#[test]
fn f_follows_the_end_of_the_log() {
    let mut subject = viewer_over(&SERVICE_LOG, 3);

    subject.handle(Key::Char('f'));

    assert!(subject.is_following());
    assert_eq!(subject.offset(), 5);
    assert!(subject
        .status()
        .ends_with(&format!("[follow]  {}", KEY_HELP)));
}

#[test]
fn new_lines_keep_a_following_viewer_on_the_last_lines() {
    let mut subject = viewer_over(&SERVICE_LOG, 3);

    subject.handle(Key::Char('f'));
    subject.push(vec![Line::new("GET /new 200", "<GET /new 200>")]);

    assert_eq!(subject.visible().last(), Some(&"<GET /new 200>"));
}

#[test]
fn new_lines_leave_a_viewer_that_is_not_following_in_place() {
    let mut subject = viewer_over(&SERVICE_LOG, 3);

    subject.push(vec![Line::new("GET /new 200", "<GET /new 200>")]);

    assert_eq!((subject.offset(), subject.shown()), (0, 9));
}

#[test]
fn new_lines_pass_through_the_filters() {
    let mut subject = viewer_over(&SERVICE_LOG, 3);

    filter(&mut subject, "ERROR");
    subject.push(vec![
        Line::new("GET /new 200", "<GET /new 200>"),
        Line::new("ERROR again", "<ERROR again>"),
    ]);

    assert_eq!(subject.shown(), 3);
}

#[test]
fn scrolling_up_stops_following() {
    let mut subject = viewer_over(&SERVICE_LOG, 3);

    subject.handle(Key::Char('f'));
    subject.handle(Key::Char('k'));

    assert!(!subject.is_following());
}

#[test]
fn f_again_stops_following() {
    let mut subject = viewer_over(&SERVICE_LOG, 3);

    subject.handle(Key::Char('f'));
    subject.handle(Key::Char('f'));

    assert!(!subject.is_following());
}
