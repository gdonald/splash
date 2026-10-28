//! Log entries linked by the identifiers they share
//!
//! An identifier is a UUID, a 32-digit hexadecimal trace ID, or the value of
//! a request, trace, correlation, transaction, or session ID field, such as
//! `request_id=4f2a` or `"traceId": "9c1e"`. Entries holding the same
//! identifier form a group, and an identifier held by one entry alone forms
//! none.
use regex::Regex;
use std::sync::LazyLock;

pub(crate) static UUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")
        .unwrap()
});

pub(crate) static TRACE_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[0-9a-fA-F]{32}\b").unwrap());

static ID_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b(?:x[-_])?(?:request|req|trace|correlation|transaction|txn|session)[-_]?id"?\s*[=:]\s*"?([A-Za-z0-9][A-Za-z0-9._-]*)"#,
    )
    .unwrap()
});

/// Entries that share an identifier: the identifier, and the position of
/// each entry in the order given
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub members: Vec<usize>,
}

/// The identifiers a text holds, in the order they first appear
pub fn ids(text: &str) -> Vec<String> {
    let mut found: Vec<(usize, String)> = UUID
        .find_iter(text)
        .chain(TRACE_ID.find_iter(text))
        .map(|id| (id.start(), id.as_str().to_string()))
        .chain(ID_FIELD.captures_iter(text).map(|caps| {
            let value = caps.get(1).unwrap();

            (
                value.start(),
                value.as_str().trim_end_matches('.').to_string(),
            )
        }))
        .collect();

    found.sort();

    let mut ids: Vec<String> = Vec::new();

    for (_, id) in found {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    ids
}

/// The groups of entries sharing an identifier, ordered by the first entry
/// of each, and an entry holding several identifiers is in each of their
/// groups
pub fn groups<T: AsRef<str>>(entries: &[T]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        for id in ids(entry.as_ref()) {
            match groups.iter_mut().find(|group| group.id == id) {
                Some(group) => group.members.push(index),
                None => groups.push(Group {
                    id,
                    members: vec![index],
                }),
            }
        }
    }

    groups.retain(|group| group.members.len() > 1);
    groups
}
