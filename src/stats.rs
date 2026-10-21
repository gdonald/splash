//! Statistics
//!
//! `--stats` writes a report on the lines a run keeps in place of the lines:
//! the counts, the time range, the levels, the status codes, and the
//! addresses, URLs, and user agents seen most. `--summary` writes the counts
//! and time range after the lines, and `--histogram` writes the number of
//! lines in each period of time in place of the lines. `--stats-format`
//! writes any of them as text, JSON, or CSV.
use crate::filter::Level;
use crate::output::{escape_json, ParsedLine, TokenKind};
use crate::time;
use crate::zone::Zone;
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

/// The number of rows a report lists for each of its top tables
pub const TOP_COUNT: usize = 10;

/// The number of `#` marks in the longest histogram bar
pub const BAR_WIDTH: u64 = 40;

/// The period a histogram bar covers when `--histogram` names none
pub const DEFAULT_BUCKET: &str = "1h";

/// A histogram period, such as `30s`, `5m`, `1h`, or `1d`
static BUCKET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)([smhd])$").unwrap());

/// An IPv4 address inside an address field
static ADDRESS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d{1,3}(?:\.\d{1,3}){3}").unwrap());

/// Error returned when a statistics option cannot be used
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsError {
    UnknownFormat(String),
    BadBucket(String),
}

impl fmt::Display for StatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatsError::UnknownFormat(name) => write!(
                f,
                "Unknown statistics format '{}' (expected text, json, or csv)",
                name
            ),
            StatsError::BadBucket(value) => write!(
                f,
                "Invalid histogram period '{}' (expected a count and a unit, such as 30s, 5m, 1h, or 1d)",
                value
            ),
        }
    }
}

impl std::error::Error for StatsError {}

/// How reports are written
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StatsFormat {
    #[default]
    Text,
    Json,
    Csv,
}

impl FromStr for StatsFormat {
    type Err = StatsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "text" => Ok(StatsFormat::Text),
            "json" => Ok(StatsFormat::Json),
            "csv" => Ok(StatsFormat::Csv),
            _ => Err(StatsError::UnknownFormat(value.to_string())),
        }
    }
}

/// The period of time one histogram bar covers
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    /// The period as written, such as `1h`
    pub name: String,
    pub seconds: i64,
}

impl FromStr for Bucket {
    type Err = StatsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bad = || StatsError::BadBucket(value.to_string());
        let found = BUCKET.captures(value).ok_or_else(bad)?;
        let count: i64 = found[1].parse().map_err(|_| bad())?;
        let unit = match &found[2] {
            "s" => 1,
            "m" => 60,
            "h" => 3_600,
            _ => time::SECONDS_PER_DAY,
        };

        match count.checked_mul(unit) {
            Some(seconds) if seconds > 0 => Ok(Bucket {
                name: value.to_string(),
                seconds,
            }),
            _ => Err(bad()),
        }
    }
}

/// What a run writes in place of its lines
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Report {
    /// The lines themselves
    #[default]
    Lines,
    /// The statistics report
    Stats,
    /// The number of lines in each period of time
    Histogram(Bucket),
}

/// The statistics options given on the command line
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatsOptions {
    pub stats: bool,
    pub summary: bool,
    pub histogram: Option<String>,
    pub format: Option<String>,
}

/// What a run writes about the lines it keeps
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatsSettings {
    pub report: Report,
    pub summary: bool,
    pub format: StatsFormat,
    /// The zone a timestamp with no zone of its own is read in
    pub source: Zone,
    /// The zone report times are written in
    pub display: Zone,
    pub now: i64,
}

impl StatsSettings {
    /// The statistics the options ask for, reading timestamps in the local
    /// zone and writing times in `display`, or the local zone when it is
    /// `None`
    pub fn build(options: &StatsOptions, display: Option<Zone>) -> Result<Self, StatsError> {
        let settings = StatsSettings::build_at(options, Zone::Utc, Zone::Utc, 0)?;

        if !settings.collects() {
            return Ok(settings);
        }

        let local = Zone::local();

        Ok(StatsSettings {
            display: display.unwrap_or_else(|| local.clone()),
            source: local,
            now: time::now(),
            ..settings
        })
    }

    /// The statistics the options ask for, reading timestamps with no zone in
    /// `source` as of `now`
    pub fn build_at(
        options: &StatsOptions,
        source: Zone,
        display: Zone,
        now: i64,
    ) -> Result<Self, StatsError> {
        let report = match (&options.histogram, options.stats) {
            (Some(bucket), _) => Report::Histogram(bucket.parse()?),
            (None, true) => Report::Stats,
            (None, false) => Report::Lines,
        };

        Ok(StatsSettings {
            report,
            summary: options.summary,
            format: options.format.as_deref().unwrap_or("text").parse()?,
            source,
            display,
            now,
        })
    }

    /// Whether the run writes its lines
    pub fn shows_lines(&self) -> bool {
        self.report == Report::Lines
    }

    /// Whether the run counts the lines it keeps
    pub fn collects(&self) -> bool {
        !self.shows_lines() || self.summary
    }

    /// The facts the statistics need from a kept line
    pub fn facts(&self, parsed: &ParsedLine) -> Facts {
        Facts::of(parsed, &self.source, self.now)
    }

    /// The report written in place of the lines, if the run writes one
    pub fn report(&self, stats: &Stats) -> Option<String> {
        match &self.report {
            Report::Lines => None,
            Report::Stats => Some(self.write(
                "Statistics",
                &stats.figures(true, &self.display),
                &stats.tables(),
            )),
            Report::Histogram(bucket) => Some(self.histogram(stats, bucket)),
        }
    }

    /// The summary written after the lines, if the run writes one
    pub fn summary(&self, stats: &Stats) -> Option<String> {
        self.summary
            .then(|| self.write("Summary", &stats.figures(false, &self.display), &[]))
    }

    fn write(&self, title: &str, figures: &[Figure], tables: &[Table]) -> String {
        match self.format {
            StatsFormat::Text => write_text(title, figures, tables),
            StatsFormat::Json => write_json(figures, tables),
            StatsFormat::Csv => write_csv(figures, tables),
        }
    }

    fn histogram(&self, stats: &Stats, bucket: &Bucket) -> String {
        let bars = stats.buckets(bucket.seconds, &self.display);

        match self.format {
            StatsFormat::Text => {
                let title = format!("Lines per {}", bucket.name);
                let mut text = format!("{}\n{}\n", title, "=".repeat(title.len()));
                let most = bars.iter().map(|(_, count)| *count).max().unwrap_or(0);

                for (start, count) in &bars {
                    let length = (count * BAR_WIDTH).div_ceil(most.max(1)) as usize;

                    text.push_str(&format!(
                        "{}  {:<width$}  {}\n",
                        start,
                        "#".repeat(length),
                        count,
                        width = BAR_WIDTH as usize
                    ));
                }

                text
            }
            StatsFormat::Json => {
                let bars: Vec<String> = bars
                    .iter()
                    .map(|(start, count)| {
                        format!(
                            "{{\"start\":\"{}\",\"count\":{}}}",
                            escape_json(start),
                            count
                        )
                    })
                    .collect();

                format!(
                    "{{\"bucket\":\"{}\",\"buckets\":[{}]}}\n",
                    escape_json(&bucket.name),
                    bars.join(",")
                )
            }
            StatsFormat::Csv => {
                let mut csv = String::from("start,count\n");

                for (start, count) in &bars {
                    csv.push_str(&format!("{},{}\n", escape_csv(start), count));
                }

                csv
            }
        }
    }
}

/// What the statistics read from one kept line
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub level: Option<Level>,
    pub moment: Option<i64>,
    pub status: Option<String>,
    pub addresses: Vec<String>,
    /// Whether the line holds an HTTP method
    pub request: bool,
    /// The URL of a request
    pub url: Option<String>,
    pub user_agent: Option<String>,
}

impl Facts {
    /// The facts in a parsed line, reading a timestamp with no zone in
    /// `zone` as of `now`
    pub fn of(parsed: &ParsedLine, zone: &Zone, now: i64) -> Facts {
        let first = |kind: TokenKind| {
            parsed
                .tokens
                .iter()
                .find(|token| token.kind == kind)
                .map(|token| token.text.trim_matches('"').to_string())
        };

        let request = parsed
            .tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Method | TokenKind::HttpVerb));

        Facts {
            level: Level::of_line(parsed),
            moment: parsed
                .tokens
                .iter()
                .filter(|token| matches!(token.kind, TokenKind::Timestamp | TokenKind::DateTime))
                .find_map(|token| time::parse_timestamp(&token.text, zone, now)),
            status: first(TokenKind::Status),
            addresses: parsed
                .tokens
                .iter()
                .filter(|token| matches!(token.kind, TokenKind::Ip | TokenKind::Client))
                .map(|token| {
                    ADDRESS
                        .find(&token.text)
                        .map_or(token.text.as_ref(), |found| found.as_str())
                        .to_string()
                })
                .collect(),
            request,
            url: first(TokenKind::Request).filter(|_| request),
            user_agent: first(TokenKind::UserAgent),
        }
    }

    /// Whether the line reports an error: it is written at the error or
    /// critical level, or its status starts with 5, as HTTP's 5xx and SMTP's
    /// 5.x.x do
    pub fn is_error(&self) -> bool {
        self.level.is_some_and(|level| level >= Level::Error)
            || self
                .status
                .as_deref()
                .is_some_and(|status| status.starts_with('5'))
    }
}

/// Counts of the lines a run has kept
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub lines: u64,
    pub errors: u64,
    pub requests: u64,
    pub first: Option<i64>,
    pub last: Option<i64>,
    pub levels: BTreeMap<Level, u64>,
    pub statuses: BTreeMap<String, u64>,
    pub addresses: HashMap<String, u64>,
    pub urls: HashMap<String, u64>,
    pub user_agents: HashMap<String, u64>,
    /// The number of lines written at each second
    pub moments: BTreeMap<i64, u64>,
}

impl Stats {
    /// Counts one kept line
    pub fn record(&mut self, facts: Facts) {
        self.lines += 1;
        self.errors += u64::from(facts.is_error());
        self.requests += u64::from(facts.request);

        if let Some(moment) = facts.moment {
            self.first = Some(self.first.map_or(moment, |first| first.min(moment)));
            self.last = Some(self.last.map_or(moment, |last| last.max(moment)));
            *self.moments.entry(moment).or_default() += 1;
        }

        if let Some(level) = facts.level {
            *self.levels.entry(level).or_default() += 1;
        }

        if let Some(status) = facts.status {
            *self.statuses.entry(status).or_default() += 1;
        }

        for address in facts.addresses {
            *self.addresses.entry(address).or_default() += 1;
        }

        if let Some(url) = facts.url {
            *self.urls.entry(url).or_default() += 1;
        }

        if let Some(user_agent) = facts.user_agent {
            *self.user_agents.entry(user_agent).or_default() += 1;
        }
    }

    /// The share of lines that report an error, as a percentage
    pub fn error_percent(&self) -> Option<f64> {
        (self.lines > 0).then(|| self.errors as f64 * 100.0 / self.lines as f64)
    }

    /// Requests per second over the time from the first timestamp to the last
    pub fn request_rate(&self) -> Option<f64> {
        let span = self.last? - self.first?;

        (span > 0).then(|| self.requests as f64 / span as f64)
    }

    /// The number of lines written in each period of `seconds`, with periods
    /// starting on the wall clock of `zone` and labeled with their start,
    /// from the first period with a timestamp to the last
    pub fn buckets(&self, seconds: i64, zone: &Zone) -> Vec<(String, u64)> {
        let mut counts: BTreeMap<i64, u64> = BTreeMap::new();

        for (moment, count) in &self.moments {
            let local = moment + zone.offset_at(*moment);
            *counts.entry(local - local.rem_euclid(seconds)).or_default() += count;
        }

        let (Some(first), Some(last)) = (counts.keys().next(), counts.keys().next_back()) else {
            return Vec::new();
        };

        (*first..=*last)
            .step_by(seconds as usize)
            .map(|start| {
                (
                    time::format_rfc3339(zone.to_utc(start), None, zone),
                    counts.get(&start).copied().unwrap_or(0),
                )
            })
            .collect()
    }

    /// The counts, and with `everything` the request counts, with the time
    /// range written in `zone`
    fn figures(&self, everything: bool, zone: &Zone) -> Vec<Figure> {
        let moment =
            |moment: Option<i64>| moment.map(|moment| time::format_rfc3339(moment, None, zone));

        let mut figures = vec![
            Figure::number("lines", "Lines", "", Some(self.lines.to_string())),
            Figure::number("errors", "Errors", "", Some(self.errors.to_string())),
            Figure::number(
                "error_percent",
                "Error rate",
                "%",
                self.error_percent()
                    .map(|percent| format!("{:.1}", percent)),
            ),
        ];

        if everything {
            figures.push(Figure::number(
                "requests",
                "Requests",
                "",
                Some(self.requests.to_string()),
            ));
            figures.push(Figure::number(
                "request_rate",
                "Request rate",
                "/s",
                self.request_rate().map(|rate| format!("{:.4}", rate)),
            ));
        }

        figures.push(Figure::text("from", "From", moment(self.first)));
        figures.push(Figure::text("to", "To", moment(self.last)));

        figures
    }

    fn tables(&self) -> Vec<Table> {
        let levels = self
            .levels
            .iter()
            .map(|(level, count)| (level.name().to_string(), *count))
            .collect();
        let statuses = self
            .statuses
            .iter()
            .map(|(status, count)| (status.clone(), *count))
            .collect();

        vec![
            Table::new("levels", "level", "Levels", levels),
            Table::new("statuses", "status", "Status codes", statuses),
            Table::new(
                "top_addresses",
                "address",
                "Top addresses",
                top(&self.addresses),
            ),
            Table::new("top_urls", "url", "Top URLs", top(&self.urls)),
            Table::new(
                "top_user_agents",
                "user_agent",
                "Top user agents",
                top(&self.user_agents),
            ),
        ]
    }
}

/// The most frequent values, most frequent first and ties in value order
fn top(counts: &HashMap<String, u64>) -> Vec<(String, u64)> {
    let mut rows: Vec<(String, u64)> = counts
        .iter()
        .map(|(value, count)| (value.clone(), *count))
        .collect();

    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows.truncate(TOP_COUNT);

    rows
}

/// One figure of a report
struct Figure {
    /// The name in JSON and CSV output
    key: &'static str,
    /// The name in text output
    label: &'static str,
    /// Written after the value in text output
    unit: &'static str,
    value: Value,
}

enum Value {
    Number(Option<String>),
    Text(Option<String>),
}

impl Figure {
    fn number(
        key: &'static str,
        label: &'static str,
        unit: &'static str,
        value: Option<String>,
    ) -> Figure {
        Figure {
            key,
            label,
            unit,
            value: Value::Number(value),
        }
    }

    fn text(key: &'static str, label: &'static str, text: Option<String>) -> Figure {
        Figure {
            key,
            label,
            unit: "",
            value: Value::Text(text),
        }
    }

    /// The value as text output writes it, if there is one
    fn written(&self) -> Option<&str> {
        match &self.value {
            Value::Number(text) | Value::Text(text) => text.as_deref(),
        }
    }

    fn json(&self) -> String {
        match &self.value {
            Value::Number(Some(number)) => number.clone(),
            Value::Text(Some(text)) => format!("\"{}\"", escape_json(text)),
            _ => "null".to_string(),
        }
    }
}

/// A table of counts in a report
struct Table {
    /// The name in JSON output
    key: &'static str,
    /// The name of each row in CSV output
    row: &'static str,
    /// The heading in text output
    title: &'static str,
    rows: Vec<(String, u64)>,
}

impl Table {
    fn new(
        key: &'static str,
        row: &'static str,
        title: &'static str,
        rows: Vec<(String, u64)>,
    ) -> Table {
        Table {
            key,
            row,
            title,
            rows,
        }
    }
}

fn write_text(title: &str, figures: &[Figure], tables: &[Table]) -> String {
    let mut text = format!("{}\n{}\n", title, "=".repeat(title.len()));
    let width = figures
        .iter()
        .map(|figure| figure.label.len() + 1)
        .max()
        .unwrap_or(0);

    for figure in figures {
        let value = match figure.written() {
            Some(value) => format!("{}{}", value, figure.unit),
            None => "-".to_string(),
        };

        text.push_str(&format!(
            "{:<width$}  {}\n",
            format!("{}:", figure.label),
            value,
            width = width
        ));
    }

    for table in tables.iter().filter(|table| !table.rows.is_empty()) {
        let width = table
            .rows
            .iter()
            .map(|(value, _)| value.chars().count())
            .max()
            .unwrap_or(0);

        text.push_str(&format!("\n{}:\n", table.title));

        for (value, count) in &table.rows {
            text.push_str(&format!("  {:<width$}  {}\n", value, count, width = width));
        }
    }

    text
}

fn write_json(figures: &[Figure], tables: &[Table]) -> String {
    let mut fields: Vec<String> = figures
        .iter()
        .map(|figure| format!("\"{}\":{}", figure.key, figure.json()))
        .collect();

    for table in tables {
        let rows: Vec<String> = table
            .rows
            .iter()
            .map(|(value, count)| {
                format!(
                    "{{\"value\":\"{}\",\"count\":{}}}",
                    escape_json(value),
                    count
                )
            })
            .collect();

        fields.push(format!("\"{}\":[{}]", table.key, rows.join(",")));
    }

    format!("{{{}}}\n", fields.join(","))
}

fn write_csv(figures: &[Figure], tables: &[Table]) -> String {
    let mut csv = String::from("metric,key,value\n");

    for figure in figures {
        csv.push_str(&format!(
            "{},,{}\n",
            figure.key,
            escape_csv(figure.written().unwrap_or(""))
        ));
    }

    for table in tables {
        for (value, count) in &table.rows {
            csv.push_str(&format!("{},{},{}\n", table.row, escape_csv(value), count));
        }
    }

    csv
}

/// Quotes a CSV field holding a comma, a quote, or a line break
pub fn escape_csv(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}
