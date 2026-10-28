//! Suggestions for misspelled names
//!
//! An error naming a value splash does not know, such as a theme or a mode,
//! ends with the known name closest to it, when one is at most two edits
//! away and the edits change less than half of the name.

/// The most edits a suggested name may be from the name given
const MOST_EDITS: usize = 2;

/// The number of single-character insertions, deletions, substitutions,
/// and swaps of neighboring characters turning one word into the other,
/// ignoring case
pub fn edit_distance(one: &str, other: &str) -> usize {
    let one: Vec<char> = one.to_lowercase().chars().collect();
    let other: Vec<char> = other.to_lowercase().chars().collect();
    let mut table = vec![vec![0; other.len() + 1]; one.len() + 1];

    for (row, cells) in table.iter_mut().enumerate() {
        cells[0] = row;
    }

    for (column, cell) in table[0].iter_mut().enumerate() {
        *cell = column;
    }

    for row in 1..=one.len() {
        for column in 1..=other.len() {
            let differs = usize::from(one[row - 1] != other[column - 1]);
            let mut distance = (table[row - 1][column] + 1)
                .min(table[row][column - 1] + 1)
                .min(table[row - 1][column - 1] + differs);

            if row > 1
                && column > 1
                && one[row - 1] == other[column - 2]
                && one[row - 2] == other[column - 1]
            {
                distance = distance.min(table[row - 2][column - 2] + 1);
            }

            table[row][column] = distance;
        }
    }

    table[one.len()][other.len()]
}

/// The number of characters two words start with in common, ignoring case
fn shared_prefix(one: &str, other: &str) -> usize {
    one.to_lowercase()
        .chars()
        .zip(other.to_lowercase().chars())
        .take_while(|(one, other)| one == other)
        .count()
}

/// The choice closest to `word`, when one is close enough to be what was
/// meant. Of choices as close, the one sharing the longest start with
/// `word` is taken.
pub fn closest<'a, I>(word: &str, choices: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let length = word.chars().count();

    choices
        .into_iter()
        .filter(|choice| *choice != word)
        .map(|choice| (edit_distance(word, choice), choice))
        .filter(|(distance, _)| *distance <= MOST_EDITS && distance * 2 < length)
        .min_by_key(|(distance, choice)| (*distance, usize::MAX - shared_prefix(word, choice)))
        .map(|(_, choice)| choice)
}

/// The sentence an error ends with to suggest the choice closest to `word`,
/// or nothing
pub fn hint<'a, I>(word: &str, choices: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    closest(word, choices)
        .map(|choice| format!(". Did you mean '{}'?", choice))
        .unwrap_or_default()
}
