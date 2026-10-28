use splash::suggest::{closest, edit_distance, hint};

#[test]
fn edits_count_insertions_deletions_substitutions_and_swaps() {
    assert_eq!(
        [
            edit_distance("syslog", "syslog"),
            edit_distance("sylog", "syslog"),
            edit_distance("syslogs", "syslog"),
            edit_distance("jsn", "json"),
            edit_distance("josn", "json"),
            edit_distance("hots", "host"),
            edit_distance("DARK", "dark"),
            edit_distance("", "dark"),
        ],
        [0, 1, 1, 1, 1, 1, 0, 4]
    );
}

#[test]
fn the_closest_choice_is_suggested_when_it_is_near_enough() {
    let themes = ["dark", "light", "solarized", "dracula"];

    assert_eq!(
        [
            closest("drakula", themes),
            closest("DARK", themes),
            closest("dark", themes),
            closest("neon", themes),
            closest("da", themes),
        ],
        [Some("dracula"), Some("dark"), None, None, None]
    );
}

#[test]
fn of_choices_as_close_the_one_sharing_the_longest_start_is_suggested() {
    assert_eq!(closest("sylog", ["sulog", "syslog"]), Some("syslog"));
}

#[test]
fn a_hint_ends_an_error_with_a_question() {
    assert_eq!(
        (hint("jsn", ["json", "csv"]), hint("xml", ["json", "csv"])),
        (". Did you mean 'json'?".to_string(), String::new())
    );
}
