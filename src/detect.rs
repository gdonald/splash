//! Log format detection
//!
//! `--auto` (or `--mode auto`) reads the first lines of the input, asks every
//! mode how many of them it reads as its own format, and renders the input
//! with the best scoring mode. A line that mode does not read is detected on
//! its own, and a line no mode reads is rendered in ad-hoc mode.
//!
//! A detected mode is carried as `auto:NAME`, so a renderer handed a later
//! chunk of the same input keeps the mode the first lines chose.
use crate::output::{ParsedLine, TokenKind};
use crate::parser;
use crate::plugin::ParseResult;
use crate::registry;
use crate::syslog;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// The mode name that asks splash to detect the format
pub const AUTO: &str = "auto";

/// The mode name for the Common Log Format, which has no plugin
pub const CLF: &str = "clf";

/// The number of lines read before choosing a mode
pub const SAMPLE_SIZE: usize = 50;

/// The share of the sample a mode must read before the whole input is
/// rendered with it. Below this, every line is detected on its own.
pub const MIN_CONFIDENCE: f32 = 0.5;

/// Modes that read any line behind a syslog header. They lose a tie to a mode
/// that reads the same lines, since that mode knows the program writing them.
pub const SYSLOG_FAMILY: [&str; 3] = ["auth", "journalctl", "syslog"];

const DETECTED_PREFIX: &str = "auto:";

/// How a mode scored against a sample of lines
#[derive(Debug, Clone, PartialEq)]
pub struct Detection {
    pub mode: String,
    /// The sample lines the mode reads as its own format
    pub matched: usize,
    /// The parsed lines in which the mode found a timestamp
    pub timestamps: usize,
    /// The fields the mode picked out of the lines it parsed
    pub fields: usize,
    /// The share of the sample the mode reads, from 0.0 to 1.0
    pub confidence: f32,
}

/// The modes detection considers, in name order, leaving out disabled plugins
pub fn candidate_modes() -> Vec<String> {
    let mut modes = registry::builtins().list_enabled_plugins().unwrap();
    modes.push(CLF.to_string());
    modes.sort();
    modes
}

/// Scores one mode against the non-blank lines of a sample
pub fn score_mode(mode: &str, lines: &[&str]) -> Detection {
    let parsed: Vec<ParsedLine> = lines
        .iter()
        .filter_map(|line| parse_as(mode, line))
        .collect();

    let confidence = match mode {
        CLF if !lines.is_empty() => parsed.len() as f32 / lines.len() as f32,
        CLF => 0.0,
        _ => registry::builtins()
            .get(mode)
            .map(|plugin| plugin.detect_format(lines))
            .unwrap_or(0.0),
    };

    Detection {
        mode: mode.to_string(),
        matched: (confidence * lines.len() as f32).round() as usize,
        timestamps: parsed.iter().filter(|line| has_timestamp(line)).count(),
        fields: parsed.iter().map(field_count).sum(),
        confidence,
    }
}

/// Scores every candidate mode against a sample, best first.
///
/// Modes are ranked by the lines they read, then a syslog family mode below
/// any other, then by the lines they found a timestamp in, then by the fields
/// they picked out, then by name.
pub fn rank(sample: &[&str]) -> Vec<Detection> {
    let lines: Vec<&str> = sample
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();

    let mut detections: Vec<Detection> = candidate_modes()
        .iter()
        .map(|mode| score_mode(mode, &lines))
        .collect();

    detections.sort_by(compare);
    detections
}

fn compare(left: &Detection, right: &Detection) -> Ordering {
    right
        .matched
        .cmp(&left.matched)
        .then_with(|| is_syslog_family(&left.mode).cmp(&is_syslog_family(&right.mode)))
        .then_with(|| right.timestamps.cmp(&left.timestamps))
        .then_with(|| right.fields.cmp(&left.fields))
        .then_with(|| left.mode.cmp(&right.mode))
}

fn has_timestamp(line: &ParsedLine) -> bool {
    line.tokens
        .iter()
        .any(|token| token.kind == TokenKind::Timestamp)
}

fn is_syslog_family(mode: &str) -> bool {
    SYSLOG_FAMILY.contains(&mode)
}

/// The mode a sample is written in, or `None` when no mode reads at least
/// `MIN_CONFIDENCE` of it.
///
/// A syslog family mode is chosen only when no other mode reads that much of
/// the sample, so a log mostly from one program is read with that program's
/// mode and the other programs' lines are detected one by one.
pub fn detect(sample: &[&str]) -> Option<Detection> {
    let (family, specific): (Vec<Detection>, Vec<Detection>) = rank(sample)
        .into_iter()
        .filter(|detection| detection.confidence >= MIN_CONFIDENCE)
        .partition(|detection| is_syslog_family(&detection.mode));

    specific.into_iter().chain(family).next()
}

/// Whether a mode asks for detection, before or after a mode was detected
pub fn is_auto(mode: &str) -> bool {
    mode == AUTO || mode.starts_with(DETECTED_PREFIX)
}

/// The mode detection chose, if `mode` carries one
pub fn detected_mode(mode: &str) -> Option<&str> {
    mode.strip_prefix(DETECTED_PREFIX)
}

/// Turns `auto` into `auto:NAME` for the mode the first `SAMPLE_SIZE` lines
/// of `contents` are written in. Any other mode, and `auto` when the format
/// is unclear, is returned unchanged.
pub fn resolve_mode(mode: &str, contents: &str) -> String {
    if mode != AUTO {
        return mode.to_string();
    }

    let sample: Vec<&str> = contents.lines().take(SAMPLE_SIZE).collect();

    match detect(&sample) {
        Some(detection) => format!("{}{}", DETECTED_PREFIX, detection.mode),
        None => mode.to_string(),
    }
}

/// Parses a line in auto mode: with the detected mode when it reads the
/// line, otherwise with the best mode for the line alone, and otherwise in
/// ad-hoc mode.
pub fn parse_auto_line<'a>(line: &'a str, mode: &str) -> Option<ParsedLine<'a>> {
    if line.trim().is_empty() {
        return None;
    }

    let detected = detected_mode(mode);

    if let Some(parsed) = detected
        .filter(|detected| is_syslog_family(detected))
        .and_then(|_| program_mode(line))
        .and_then(|program_mode| parse_as(&program_mode, line))
    {
        return Some(parsed);
    }

    if let Some(parsed) = detected.and_then(|detected| parse_as(detected, line)) {
        return Some(parsed);
    }

    match detect(&[line]) {
        Some(detection) => parse_as(&detection.mode, line),
        None => Some(parser::parse_adhoc_line(line)),
    }
}

/// Parses a line with one mode, or `None` when the mode does not read it
/// The mode chosen for each program a syslog line names, so each program is
/// detected once
static PROGRAM_MODES: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The mode of the program that wrote a syslog line, other than a syslog
/// family mode, as detection chose it from the first line of the program
/// it saw, or `None` when only a syslog family mode reads that line
pub fn program_mode(line: &str) -> Option<String> {
    let program = syslog::split_header(line)?.program;
    let mut modes = PROGRAM_MODES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    modes
        .entry(program.to_string())
        .or_insert_with(|| {
            detect(&[line])
                .map(|detection| detection.mode)
                .filter(|mode| !is_syslog_family(mode))
        })
        .clone()
}

/// Forgets the mode chosen for each program, for when the plugins change
pub fn forget_program_modes() {
    PROGRAM_MODES.lock().unwrap().clear();
}

pub fn parse_as<'a>(mode: &str, line: &'a str) -> Option<ParsedLine<'a>> {
    match mode {
        CLF => parser::parse_clf_line(line),
        _ => match registry::builtins().get(mode).ok()?.parse_line(line) {
            ParseResult::Parsed(parsed) => Some(parsed),
            ParseResult::NoMatch | ParseResult::Error(_) => None,
        },
    }
}

/// The tokens a parse styled as a field of the format, leaving out plain
/// text, punctuation, JSON keys, and bare numbers, which any parser produces
fn field_count(parsed: &ParsedLine) -> usize {
    parsed
        .tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Plain | TokenKind::Punctuation | TokenKind::Header | TokenKind::Number
            )
        })
        .count()
}
