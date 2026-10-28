//! Plugin files written from sample lines
//!
//! Each sample line is split into whitespace, punctuation, words, and fields
//! such as timestamps, levels, numbers, and addresses. Lines that agree up to
//! their first word share a pattern. A pattern reads the pieces its lines
//! agree on, and the text from the first piece they disagree on is the
//! message.
use crate::filter::Level;
use crate::registry;
use regex::Regex;
use std::collections::HashSet;
use std::fmt;
use std::sync::LazyLock;

/// Why a plugin file could not be written from the samples
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LearnError {
    NoSamples,
    BuiltinName(String),
}

impl fmt::Display for LearnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LearnError::NoSamples => write!(f, "there are no sample lines to learn from"),
            LearnError::BuiltinName(name) => {
                write!(f, "'{}' is the name of a built-in plugin", name)
            }
        }
    }
}

impl std::error::Error for LearnError {}

/// Timestamps as ISO 8601, syslog, and the Common Log Format write them
pub(crate) static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[-+]\d{2}:?\d{2})?",
        r"|[A-Z][a-z]{2} [ \d]\d \d{2}:\d{2}:\d{2}",
        r"|\d{2}/[A-Z][a-z]{2}/\d{4}:\d{2}:\d{2}:\d{2}(?: [-+]\d{4})?",
    ))
    .unwrap()
});

/// A run of whitespace, one punctuation character, or a word
static RUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\s+|[\[\](){}<>",;=|:]|[^\s\[\](){}<>",;=|:]+"#).unwrap());

/// The text a word holds
const WORD: &str = r#"[^\s\[\](){}<>",;=|:]+"#;

const NUMBER: &str = r"\d+";

const IP: &str = r"\d{1,3}(?:\.\d{1,3}){3}";

const PROTOCOL: &str = r"HTTP/\d(?:\.\d)?";

const DURATION: &str = r"\d+(?:\.\d+)?(?:ns|us|µs|ms|s|m|h)";

const METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "CONNECT", "OPTIONS", "TRACE",
];

static WHOLE_WORD: LazyLock<Regex> = LazyLock::new(|| whole(WORD));
static WHOLE_NUMBER: LazyLock<Regex> = LazyLock::new(|| whole(NUMBER));
static WHOLE_IP: LazyLock<Regex> = LazyLock::new(|| whole(IP));
static WHOLE_PROTOCOL: LazyLock<Regex> = LazyLock::new(|| whole(PROTOCOL));
static WHOLE_DURATION: LazyLock<Regex> = LazyLock::new(|| whole(DURATION));
static WHOLE_EMAIL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^@]+@[^@]+\.[A-Za-z]+$").unwrap());

fn whole(pattern: &str) -> Regex {
    Regex::new(&format!("^(?:{})$", pattern)).unwrap()
}

/// One piece of a sample line
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Space,
    Punctuation(String),
    Word(String),
    Field { key: &'static str, pattern: String },
}

/// Writes a plugin file named `name` whose patterns read every line of
/// `text` that is not blank
pub fn learn(name: &str, text: &str) -> Result<String, LearnError> {
    if registry::is_builtin(name) {
        return Err(LearnError::BuiltinName(name.to_string()));
    }

    let samples: Vec<Vec<Piece>> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(pieces)
        .collect();

    if samples.is_empty() {
        return Err(LearnError::NoSamples);
    }

    let mut groups: Vec<(Vec<Piece>, Vec<Vec<Piece>>)> = Vec::new();

    for sample in samples {
        let signature = signature(&sample);

        match groups.iter_mut().find(|(known, _)| *known == signature) {
            Some((_, members)) => members.push(sample),
            None => groups.push((signature, vec![sample])),
        }
    }

    let count: usize = groups.iter().map(|(_, members)| members.len()).sum();
    let description = match count {
        1 => "Learned from 1 sample line".to_string(),
        _ => format!("Learned from {} sample lines", count),
    };

    let mut file = format!(
        "name = {}\ndescription = {}\n\n[patterns]\n",
        quote(name),
        quote(&description)
    );

    for (index, (_, members)) in groups.iter().enumerate() {
        let key = match groups.len() {
            1 => "line".to_string(),
            _ => format!("line{}", index + 1),
        };

        file.push_str(&format!("{} = {}\n", key, quote(&pattern(members))));
    }

    Ok(file)
}

/// The pieces of a line, in order
fn pieces(line: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut cursor = 0;

    for timestamp in TIMESTAMP.find_iter(line) {
        push_runs(&mut pieces, &line[cursor..timestamp.start()]);
        pieces.push(Piece::Field {
            key: "timestamp",
            pattern: shape(timestamp.as_str()),
        });
        cursor = timestamp.end();
    }

    push_runs(&mut pieces, &line[cursor..]);

    pieces
}

fn push_runs(pieces: &mut Vec<Piece>, text: &str) {
    for run in RUN.find_iter(text) {
        let run = run.as_str();

        pieces.push(match run {
            _ if run.starts_with(char::is_whitespace) => Piece::Space,
            _ if !WHOLE_WORD.is_match(run) => Piece::Punctuation(run.to_string()),
            _ => classify(run),
        });
    }
}

/// The field a word is, or the word itself
fn classify(word: &str) -> Piece {
    let field = |key, pattern: &str| Piece::Field {
        key,
        pattern: pattern.to_string(),
    };

    match word {
        _ if WHOLE_NUMBER.is_match(word) => field("number", NUMBER),
        _ if WHOLE_IP.is_match(word) => field("ip", IP),
        _ if word
            .chars()
            .all(|character| character.is_ascii_alphabetic())
            && Level::from_name(word).is_some() =>
        {
            field("level", "[A-Za-z]+")
        }
        _ if METHODS.contains(&word) => field("method", "[A-Z]+"),
        _ if WHOLE_PROTOCOL.is_match(word) => field("protocol", PROTOCOL),
        _ if WHOLE_DURATION.is_match(word) => field("duration", DURATION),
        _ if WHOLE_EMAIL.is_match(word) => field("email", WORD),
        _ if word.starts_with('/') => field("path", WORD),
        _ => Piece::Word(word.to_string()),
    }
}

/// A pattern matching text of the same form: each run of digits, of
/// letters, and of whitespace stands for any run of its kind
fn shape(text: &str) -> String {
    let mut pattern = String::new();
    let mut last = "";

    for character in text.chars() {
        let class = match character {
            _ if character.is_ascii_digit() => r"\d+",
            _ if character.is_ascii_alphabetic() => "[A-Za-z]+",
            _ if character.is_whitespace() => r"\s+",
            _ => "",
        };

        match class {
            "" => pattern.push_str(&regex::escape(&character.to_string())),
            _ if class == last => continue,
            _ => pattern.push_str(class),
        }

        last = class;
    }

    pattern
}

/// The pieces of a line up to and including its first word, with the
/// word's text left out, which the lines a pattern reads agree on
fn signature(sample: &[Piece]) -> Vec<Piece> {
    let end = sample
        .iter()
        .position(|piece| matches!(piece, Piece::Word(_)))
        .map_or(sample.len(), |index| index + 1);

    sample[..end]
        .iter()
        .map(|piece| match piece {
            Piece::Word(_) => Piece::Word(String::new()),
            other => other.clone(),
        })
        .collect()
}

/// The pattern reading the pieces every sample agrees on, then the rest of
/// the line as the message
fn pattern(samples: &[Vec<Piece>]) -> String {
    let mut regex = String::from("^");
    let mut named = HashSet::new();

    for position in 0.. {
        let at: Vec<Option<&Piece>> = samples.iter().map(|sample| sample.get(position)).collect();

        if at.iter().all(Option::is_none) {
            regex.push('$');
            break;
        }

        let agreed = at.iter().all(|piece| *piece == at[0]);

        if let Some(shape) = (!agreed).then(|| identifier_shape(&at)).flatten() {
            regex.push_str(&shape);
            continue;
        }

        match at[0].filter(|_| agreed) {
            Some(Piece::Space) => regex.push_str(r"\s+"),
            Some(Piece::Punctuation(text)) | Some(Piece::Word(text)) => {
                regex.push_str(&regex::escape(text))
            }
            Some(Piece::Field { key, pattern }) if named.insert(*key) => {
                regex.push_str(&format!("(?P<{}>{})", key, pattern))
            }
            Some(Piece::Field { pattern, .. }) => regex.push_str(&format!("(?:{})", pattern)),
            None => {
                regex.push_str("(?P<message>.*)$");
                break;
            }
        }
    }

    regex
}

/// The shape of words that differ but share one form holding digits, such
/// as `worker-2` and `worker-3`
fn identifier_shape(at: &[Option<&Piece>]) -> Option<String> {
    let shapes: Vec<String> = at
        .iter()
        .map(|piece| match piece {
            Some(Piece::Word(word)) => Some(shape(word)),
            _ => None,
        })
        .collect::<Option<_>>()?;

    Some(shapes[0].clone())
        .filter(|first| first.contains(r"\d") && shapes.iter().all(|shape| shape == first))
}

/// A TOML string holding `text`: a literal string, which takes no escapes,
/// unless the text holds a single quote
fn quote(text: &str) -> String {
    match text.contains('\'') {
        false => format!("'{}'", text),
        true => format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"")),
    }
}
