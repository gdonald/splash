//! APM parsing
//!
//! apmd, the Advanced Power Management daemon, logs through syslog under
//! `apmd`. Its battery lines read `Battery: 87%, discharging (-0.50%/min over
//! 0:10:00), 1:23:45 (2:54:00) to empty`: the charge, whether the battery is
//! charging, the rate it measured, and the time left. A low battery line is
//! prefixed with `Warning: `. It also logs the power source it switched to,
//! as `Using line power; charging battery`, and the suspends, standbys, and
//! resumes it handles.
use crate::mail::{self, Words};
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::syslog;
use regex::Regex;
use std::sync::LazyLock;

/// The name `--mode` and `--list-plugins` use for this plugin
pub const NAME: &str = "apm";

/// The parts of an apmd message that carry a value: a percentage, a rate, a
/// time, and the battery state
static SPOTS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ([+-]?\d+(?:\.\d+)?%/(?:min|day))                   # rate
        |
        ([+-]?\d+%|\?%)                                     # percentage
        |
        (\d+d\+\d+:\d{2}:\d{2}|\d+:\d{2}:\d{2})             # time
        |
        \b(not\ charging|discharging|charging|absent)\b     # battery state
        ",
    )
    .unwrap()
});

static WORDS: LazyLock<Words> = LazyLock::new(|| {
    Words::new(&[
        ("Warning", TokenKind::Warning),
        ("BATTERY IS LOW", TokenKind::Failure),
        ("Suspension was rejected by the kernel", TokenKind::Failure),
        ("Suspend rejected by proxy", TokenKind::Warning),
        ("Standby rejected by proxy", TokenKind::Warning),
        ("Received unknown event", TokenKind::Failure),
        ("Critical Resume", TokenKind::Warning),
    ])
});

/// The APM plugin
pub struct ApmPlugin {
    metadata: PluginMetadata,
}

impl ApmPlugin {
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                NAME,
                PluginVersion::new(1, 0, 0),
                "apmd Advanced Power Management logs",
                "splash",
            ),
        }
    }
}

impl Default for ApmPlugin {
    fn default() -> Self {
        ApmPlugin::new()
    }
}

impl Plugin for ApmPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a> {
        match parse_line(line) {
            Some(parsed) => ParseResult::Parsed(parsed),
            None => ParseResult::NoMatch,
        }
    }
}

/// The kind a battery state is colored in: charging is a success,
/// discharging and not charging are warnings, and an absent battery is a
/// failure
fn state_kind(state: &str) -> TokenKind {
    match state {
        "charging" => TokenKind::Success,
        "absent" => TokenKind::Failure,
        _ => TokenKind::Warning,
    }
}

/// Parses one apmd line, or returns `None` when the line was not logged by
/// apmd.
pub fn parse_line(line: &str) -> Option<ParsedLine<'_>> {
    let header = syslog::split_header(line)?;

    if header.program != "apmd" {
        return None;
    }

    let mut tokens = header.tokens;

    push_message(&mut tokens, header.body);

    Some(ParsedLine::new(tokens))
}

/// Colors an apmd message.
fn push_message<'a>(tokens: &mut Vec<Token<'a>>, message: &'a str) {
    let mut cursor = 0;

    for caps in SPOTS.captures_iter(message) {
        let whole = caps.get(0).unwrap();
        let field = |index: usize| caps.get(index).map(|found| found.as_str());

        mail::push_words(tokens, &message[cursor..whole.start()], &WORDS);

        if let Some(rate) = field(1) {
            tokens.push(Token::new(rate, TokenKind::Number));
        } else if let Some(percentage) = field(2) {
            tokens.push(Token::new(percentage, TokenKind::Size));
        } else if let Some(time) = field(3) {
            tokens.push(Token::new(time, TokenKind::Duration));
        } else {
            let state = field(4).unwrap();
            tokens.push(Token::new(state, state_kind(state)));
        }

        cursor = whole.end();
    }

    mail::push_words(tokens, &message[cursor..], &WORDS);
}
