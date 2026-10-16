//! Timestamp rewriting
//!
//! `--convert-date` rewrites timestamps written as seconds since the epoch,
//! and `--normalize-dates`, `--tz`, and `--relative` rewrite every timestamp
//! splash reads. A rewritten timestamp is written as RFC 3339 in the local
//! zone or the `--tz` zone, or with `--relative` as its distance from now,
//! such as `2 hours ago`.
use crate::output::{ParsedLine, Token, TokenKind};
use crate::time;
use crate::zone::Zone;
use std::fmt;

/// Error returned when a zone given on the command line cannot be found
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownZone(pub String);

impl fmt::Display for UnknownZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unknown time zone '{}' (expected UTC, a name such as America/New_York, or a POSIX TZ rule)",
            self.0
        )
    }
}

impl std::error::Error for UnknownZone {}

/// The timestamp options given on the command line
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DateOptions {
    pub convert_epoch: bool,
    pub normalize: bool,
    pub tz: Option<String>,
    pub relative: bool,
}

/// The timestamps a rewrite touches
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Selection {
    #[default]
    None,
    Epoch,
    All,
}

/// How timestamps are rewritten
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DateRewrite {
    pub selection: Selection,
    /// The zone a timestamp with no zone of its own is read in
    pub source: Zone,
    /// The zone rewritten timestamps are written in
    pub display: Zone,
    pub relative: bool,
    pub now: i64,
}

impl DateRewrite {
    /// The rewrite the options ask for, reading and writing in the local zone
    /// unless `--tz` names another one to write in
    pub fn build(options: &DateOptions) -> Result<DateRewrite, UnknownZone> {
        if options == &DateOptions::default() {
            return Ok(DateRewrite::default());
        }

        DateRewrite::build_at(options, Zone::local(), time::now())
    }

    /// The rewrite the options ask for, reading timestamps with no zone in
    /// `local` as of `now`
    pub fn build_at(
        options: &DateOptions,
        local: Zone,
        now: i64,
    ) -> Result<DateRewrite, UnknownZone> {
        let display = match &options.tz {
            Some(name) => Zone::named(name).ok_or_else(|| UnknownZone(name.clone()))?,
            None => local.clone(),
        };

        let selection = if options.normalize || options.tz.is_some() || options.relative {
            Selection::All
        } else if options.convert_epoch {
            Selection::Epoch
        } else {
            Selection::None
        };

        Ok(DateRewrite {
            selection,
            source: local,
            display,
            relative: options.relative,
            now,
        })
    }

    /// Rewrites the timestamps of a parsed line that this rewrite selects
    pub fn apply<'a>(&self, parsed: ParsedLine<'a>) -> ParsedLine<'a> {
        if self.selection == Selection::None {
            return parsed;
        }

        let tokens = parsed
            .tokens
            .into_iter()
            .map(|token| self.rewrite(&token).unwrap_or(token))
            .collect();

        ParsedLine::new(tokens)
    }

    fn rewrite<'a>(&self, token: &Token<'a>) -> Option<Token<'a>> {
        if !matches!(token.kind, TokenKind::Timestamp | TokenKind::DateTime) {
            return None;
        }

        if self.selection == Selection::Epoch && !time::is_epoch(&token.text) {
            return None;
        }

        let moment = time::parse_timestamp(&token.text, &self.source, self.now)?;
        let (start, end) = time::timestamp_span(&token.text);
        let written = &token.text[start..end];

        let rewritten = if self.relative {
            time::format_relative(moment, self.now)
        } else {
            time::format_rfc3339(moment, time::fraction(written), &self.display)
        };

        Some(Token::owned(
            format!(
                "{}{}{}",
                &token.text[..start],
                rewritten,
                &token.text[end..]
            ),
            token.kind,
        ))
    }
}
