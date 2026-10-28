//! Line differences between two logs
//!
//! Lines are compared with their timestamps, UUIDs, and 32-digit hexadecimal
//! IDs masked, so two runs of one program that differ only in when they ran
//! and in the IDs they generated compare equal. The differences are grouped
//! into hunks with lines of context around them, as `diff -u` groups them.
use crate::correlate::{TRACE_ID, UUID};
use crate::learn::TIMESTAMP;

/// The lines of context a hunk keeps around its changes
pub const CONTEXT: usize = 3;

/// One line of either log: in both, only in the old one, or only in the new
/// one, with its position in each log it is in
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Same { old: usize, new: usize },
    Removed { old: usize },
    Added { new: usize },
}

/// A run of changes with the context around them, and where it starts and
/// how many lines it spans in each log, as a `@@` header gives them
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub changes: Vec<Change>,
}

impl Hunk {
    /// The `@@ -START,COUNT +START,COUNT @@` header. A start is the number
    /// of the hunk's first line, or of the line before it when the hunk
    /// holds no lines of that log.
    pub fn header(&self) -> String {
        let start = |start: usize, count: usize| match count {
            0 => start,
            _ => start + 1,
        };

        format!(
            "@@ -{},{} +{},{} @@",
            start(self.old_start, self.old_count),
            self.old_count,
            start(self.new_start, self.new_count),
            self.new_count
        )
    }
}

/// A line with its timestamps, UUIDs, and trace IDs masked
pub fn masked(line: &str) -> String {
    let line = TIMESTAMP.replace_all(line, "<time>");
    let line = UUID.replace_all(&line, "<id>");

    TRACE_ID.replace_all(&line, "<id>").into_owned()
}

/// The shortest list of changes turning `old` into `new`, comparing masked
/// lines, found with Myers' algorithm
pub fn changes(old: &[&str], new: &[&str]) -> Vec<Change> {
    let old: Vec<String> = old.iter().map(|line| masked(line)).collect();
    let new: Vec<String> = new.iter().map(|line| masked(line)).collect();
    let (n, m) = (old.len() as isize, new.len() as isize);

    // trace[d] holds, for each diagonal k from -d to d, the furthest x
    // reached with d - 1 edits
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut furthest = vec![0isize; 3];
    let mut d: isize = 0;

    loop {
        trace.push(furthest.clone());

        let mut next = vec![0isize; (2 * d + 3) as usize];
        let reached = |k: isize| furthest[(k + d) as usize];

        for k in (-d..=d).step_by(2) {
            let mut x = match k == -d || (k != d && reached(k - 1) < reached(k + 1)) {
                true => reached(k + 1),
                false => reached(k - 1) + 1,
            };
            let mut y = x - k;

            while x < n && y < m && old[x as usize] == new[y as usize] {
                x += 1;
                y += 1;
            }

            next[(k + d + 1) as usize] = x;

            if x >= n && y >= m {
                return backtrack(&trace, n, m);
            }
        }

        furthest = next;
        d += 1;
    }
}

fn backtrack(trace: &[Vec<isize>], n: isize, m: isize) -> Vec<Change> {
    let (mut x, mut y) = (n, m);
    let mut found = Vec::new();

    for (d, furthest) in trace.iter().enumerate().rev() {
        let d = d as isize;
        let k = x - y;
        let reached = |k: isize| furthest[(k + d) as usize];

        let previous_k = match k == -d || (k != d && reached(k - 1) < reached(k + 1)) {
            true => k + 1,
            false => k - 1,
        };
        let previous_x = reached(previous_k);
        let previous_y = previous_x - previous_k;

        while x > previous_x && y > previous_y {
            x -= 1;
            y -= 1;
            found.push(Change::Same {
                old: x as usize,
                new: y as usize,
            });
        }

        if d > 0 {
            match x == previous_x {
                true => found.push(Change::Added {
                    new: previous_y as usize,
                }),
                false => found.push(Change::Removed {
                    old: previous_x as usize,
                }),
            }
        }

        x = previous_x;
        y = previous_y;
    }

    found.reverse();
    found
}

/// The changes grouped into hunks with `context` lines around them. Logs
/// that do not differ have no hunks.
pub fn hunks(changes: &[Change], context: usize) -> Vec<Hunk> {
    let changed: Vec<usize> = (0..changes.len())
        .filter(|&index| !matches!(changes[index], Change::Same { .. }))
        .collect();

    let mut spans: Vec<(usize, usize)> = Vec::new();

    for index in changed {
        let start = index.saturating_sub(context);
        let end = (index + context + 1).min(changes.len());

        match spans.last_mut() {
            Some(last) if start <= last.1 => last.1 = end,
            _ => spans.push((start, end)),
        }
    }

    spans
        .into_iter()
        .map(|(start, end)| {
            let before = &changes[..start];
            let within = &changes[start..end];
            let olds = |changes: &[Change]| {
                changes
                    .iter()
                    .filter(|change| !matches!(change, Change::Added { .. }))
                    .count()
            };
            let news = |changes: &[Change]| {
                changes
                    .iter()
                    .filter(|change| !matches!(change, Change::Removed { .. }))
                    .count()
            };

            Hunk {
                old_start: olds(before),
                old_count: olds(within),
                new_start: news(before),
                new_count: news(within),
                changes: within.to_vec(),
            }
        })
        .collect()
}
