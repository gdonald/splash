use splash::diff::{self, Change, Hunk, CONTEXT};

fn same(old: usize, new: usize) -> Change {
    Change::Same { old, new }
}

fn removed(old: usize) -> Change {
    Change::Removed { old }
}

fn added(new: usize) -> Change {
    Change::Added { new }
}

#[test]
fn test_timestamps_uuids_and_trace_ids_are_masked() {
    assert_eq!(
        diff::masked(
            "2024-03-05 10:15:01,120 job 0f8fad5b-d9cb-469f-a165-70867728950e trace 4bf92f3577b34da6a3ce929d0e0e4736 took 3 tries"
        ),
        "<time> job <id> trace <id> took 3 tries"
    );
}

#[test]
fn test_logs_that_differ_only_in_timestamps_compare_equal() {
    let old = ["2024-03-05T10:15:01Z start", "2024-03-05T10:15:02Z done"];
    let new = ["2024-03-06T08:00:00Z start", "2024-03-06T08:00:09Z done"];

    assert_eq!(diff::changes(&old, &new), vec![same(0, 0), same(1, 1)]);
}

#[test]
fn test_a_removed_line_and_an_added_line_are_changes() {
    let old = ["start", "connect", "done"];
    let new = ["start", "retry", "connect", "done", "exit"];

    assert_eq!(
        diff::changes(&old, &new),
        vec![same(0, 0), added(1), same(1, 2), same(2, 3), added(4)]
    );
}

#[test]
fn test_a_replaced_line_is_removed_then_added() {
    let old = ["start", "ok", "done"];
    let new = ["start", "failed", "done"];

    assert_eq!(
        diff::changes(&old, &new),
        vec![same(0, 0), removed(1), added(1), same(2, 2)]
    );
}

#[test]
fn test_empty_logs_have_no_changes() {
    assert_eq!(diff::changes(&[], &[]), vec![]);
    assert_eq!(diff::changes(&["a"], &[]), vec![removed(0)]);
    assert_eq!(diff::changes(&[], &["a"]), vec![added(0)]);
}

/// The length of the longest common subsequence, by dynamic programming
fn common_length(old: &[&str], new: &[&str]) -> usize {
    let mut table = vec![vec![0; new.len() + 1]; old.len() + 1];

    for (row, old_line) in old.iter().enumerate() {
        for (column, new_line) in new.iter().enumerate() {
            table[row + 1][column + 1] = match old_line == new_line {
                true => table[row][column] + 1,
                false => table[row][column + 1].max(table[row + 1][column]),
            };
        }
    }

    table[old.len()][new.len()]
}

#[test]
fn test_the_changes_rebuild_both_logs_with_the_fewest_edits() {
    let words = ["a", "b", "c"];
    let mut seed: u64 = 7;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };

    for _ in 0..300 {
        let old: Vec<&str> = (0..next() % 9).map(|_| words[next() % 3]).collect();
        let new: Vec<&str> = (0..next() % 9).map(|_| words[next() % 3]).collect();
        let changes = diff::changes(&old, &new);

        let rebuilt_old: Vec<&str> = changes
            .iter()
            .filter_map(|change| match *change {
                Change::Same { old: index, .. } | Change::Removed { old: index } => {
                    Some(old[index])
                }
                Change::Added { .. } => None,
            })
            .collect();
        let rebuilt_new: Vec<&str> = changes
            .iter()
            .filter_map(|change| match *change {
                Change::Same { new: index, .. } | Change::Added { new: index } => Some(new[index]),
                Change::Removed { .. } => None,
            })
            .collect();
        let kept = changes
            .iter()
            .filter(|change| matches!(change, Change::Same { .. }))
            .count();

        assert_eq!(
            (rebuilt_old, rebuilt_new, kept),
            (old.clone(), new.clone(), common_length(&old, &new)),
            "{:?} {:?}",
            old,
            new
        );
    }
}

#[test]
fn test_changes_far_apart_form_separate_hunks_with_context() {
    let old: Vec<String> = (1..=20).map(|number| format!("line {}", number)).collect();
    let mut new = old.clone();
    new[1] = "line two".to_string();
    new[16] = "line seventeen".to_string();

    let old: Vec<&str> = old.iter().map(String::as_str).collect();
    let new: Vec<&str> = new.iter().map(String::as_str).collect();
    let hunks = diff::hunks(&diff::changes(&old, &new), CONTEXT);

    assert_eq!(
        hunks.iter().map(Hunk::header).collect::<Vec<String>>(),
        ["@@ -1,5 +1,5 @@", "@@ -14,7 +14,7 @@"]
    );
}

#[test]
fn test_changes_close_together_share_a_hunk() {
    let old = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let new = ["a", "B", "c", "d", "e", "f", "G", "h"];

    assert_eq!(
        diff::hunks(&diff::changes(&old, &new), 1),
        vec![
            Hunk {
                old_start: 0,
                old_count: 3,
                new_start: 0,
                new_count: 3,
                changes: vec![same(0, 0), removed(1), added(1), same(2, 2)],
            },
            Hunk {
                old_start: 5,
                old_count: 3,
                new_start: 5,
                new_count: 3,
                changes: vec![same(5, 5), removed(6), added(6), same(7, 7)],
            },
        ]
    );
    assert_eq!(diff::hunks(&diff::changes(&old, &new), CONTEXT).len(), 1);
}

#[test]
fn test_a_hunk_holding_no_lines_of_a_log_starts_at_the_line_before_it() {
    let hunks = diff::hunks(&diff::changes(&["a", "b"], &["a", "b", "c"]), 0);

    assert_eq!(hunks[0].header(), "@@ -2,0 +3,1 @@");
}

#[test]
fn test_logs_that_do_not_differ_have_no_hunks() {
    assert!(diff::hunks(&diff::changes(&["a"], &["a"]), CONTEXT).is_empty());
}
