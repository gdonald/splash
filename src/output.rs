/// Output modes and renderers
///
/// A parsed line is a sequence of styled tokens. Each output mode renders that
/// same sequence differently: ANSI escapes for terminals, HTML for a browser,
/// JSON for other programs, JSON Lines and CSV keyed by field for analysis,
/// and plain text for scripting.
use crate::json;
use crate::logfmt;
use crate::suggest;
use crate::theme::Theme;
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

/// The names `--output` accepts, as `--help` lists them
pub const OUTPUT_NAMES: [&str; 7] = ["ansi", "csv", "curses", "html", "json", "jsonl", "plain"];

/// The style assigned to a token by the parser
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Plain,
    Punctuation,
    Ip,
    Number,
    DateTime,
    TimezoneOffset,
    HttpVerb,
    HttpVersion,
    Client,
    UserIdentifier,
    UserId,
    Timestamp,
    Method,
    Request,
    Protocol,
    Status,
    Size,
    Referer,
    UserAgent,
    VirtualHost,
    Duration,
    Level,
    Module,
    Pid,
    Message,
    CacheHit,
    CacheMiss,
    CacheResult,
    Hierarchy,
    ContentType,
    Transaction,
    Tag,
    Header,
    Host,
    Frontend,
    Backend,
    Server,
    Timers,
    Termination,
    Counters,
    Email,
    QueueId,
    Path,
    Success,
    Warning,
    Failure,
    Match,
    Capture,
    Keyword,
    UnknownUser,
    Service,
    Location,
    Source,
}

impl TokenKind {
    /// The machine-readable name used in JSON output and HTML class names
    pub fn name(&self) -> &'static str {
        match self {
            TokenKind::Plain => "plain",
            TokenKind::Punctuation => "punctuation",
            TokenKind::Ip => "ip",
            TokenKind::Number => "number",
            TokenKind::DateTime => "datetime",
            TokenKind::TimezoneOffset => "tz_offset",
            TokenKind::HttpVerb => "http_verb",
            TokenKind::HttpVersion => "http_version",
            TokenKind::Client => "client",
            TokenKind::UserIdentifier => "user_identifier",
            TokenKind::UserId => "userid",
            TokenKind::Timestamp => "timestamp",
            TokenKind::Method => "method",
            TokenKind::Request => "request",
            TokenKind::Protocol => "protocol",
            TokenKind::Status => "status",
            TokenKind::Size => "size",
            TokenKind::Referer => "referer",
            TokenKind::UserAgent => "user_agent",
            TokenKind::VirtualHost => "vhost",
            TokenKind::Duration => "duration",
            TokenKind::Level => "level",
            TokenKind::Module => "module",
            TokenKind::Pid => "pid",
            TokenKind::Message => "message",
            TokenKind::CacheHit => "cache_hit",
            TokenKind::CacheMiss => "cache_miss",
            TokenKind::CacheResult => "cache_result",
            TokenKind::Hierarchy => "hierarchy",
            TokenKind::ContentType => "content_type",
            TokenKind::Transaction => "transaction",
            TokenKind::Tag => "tag",
            TokenKind::Header => "header",
            TokenKind::Host => "host",
            TokenKind::Frontend => "frontend",
            TokenKind::Backend => "backend",
            TokenKind::Server => "server",
            TokenKind::Timers => "timers",
            TokenKind::Termination => "termination",
            TokenKind::Counters => "counters",
            TokenKind::Email => "email",
            TokenKind::QueueId => "queue_id",
            TokenKind::Path => "path",
            TokenKind::Success => "success",
            TokenKind::Warning => "warning",
            TokenKind::Failure => "failure",
            TokenKind::Match => "match",
            TokenKind::Capture => "capture",
            TokenKind::Keyword => "keyword",
            TokenKind::UnknownUser => "unknown_user",
            TokenKind::Service => "service",
            TokenKind::Location => "location",
            TokenKind::Source => "source",
        }
    }

    /// The number of distinct token styles
    pub const COUNT: usize = 53;

    /// Position of this style in `all()`, used to index a theme
    pub fn index(&self) -> usize {
        TokenKind::all()
            .iter()
            .position(|kind| kind == self)
            .unwrap_or(0)
    }

    /// The style with this machine-readable name
    pub fn from_name(name: &str) -> Option<TokenKind> {
        TokenKind::all()
            .into_iter()
            .find(|kind| kind.name() == name)
    }

    /// The machine-readable name of every style, in declaration order
    pub fn names() -> [&'static str; TokenKind::COUNT] {
        TokenKind::all().map(|kind| kind.name())
    }

    /// Every style, in declaration order
    pub fn all() -> [TokenKind; TokenKind::COUNT] {
        [
            TokenKind::Plain,
            TokenKind::Punctuation,
            TokenKind::Ip,
            TokenKind::Number,
            TokenKind::DateTime,
            TokenKind::TimezoneOffset,
            TokenKind::HttpVerb,
            TokenKind::HttpVersion,
            TokenKind::Client,
            TokenKind::UserIdentifier,
            TokenKind::UserId,
            TokenKind::Timestamp,
            TokenKind::Method,
            TokenKind::Request,
            TokenKind::Protocol,
            TokenKind::Status,
            TokenKind::Size,
            TokenKind::Referer,
            TokenKind::UserAgent,
            TokenKind::VirtualHost,
            TokenKind::Duration,
            TokenKind::Level,
            TokenKind::Module,
            TokenKind::Pid,
            TokenKind::Message,
            TokenKind::CacheHit,
            TokenKind::CacheMiss,
            TokenKind::CacheResult,
            TokenKind::Hierarchy,
            TokenKind::ContentType,
            TokenKind::Transaction,
            TokenKind::Tag,
            TokenKind::Header,
            TokenKind::Host,
            TokenKind::Frontend,
            TokenKind::Backend,
            TokenKind::Server,
            TokenKind::Timers,
            TokenKind::Termination,
            TokenKind::Counters,
            TokenKind::Email,
            TokenKind::QueueId,
            TokenKind::Path,
            TokenKind::Success,
            TokenKind::Warning,
            TokenKind::Failure,
            TokenKind::Match,
            TokenKind::Capture,
            TokenKind::Keyword,
            TokenKind::UnknownUser,
            TokenKind::Service,
            TokenKind::Location,
            TokenKind::Source,
        ]
    }
}

/// A run of text with one style, borrowed from the line it was parsed from
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    pub text: Cow<'a, str>,
    pub kind: TokenKind,
}

impl<'a> Token<'a> {
    pub fn new(text: &'a str, kind: TokenKind) -> Self {
        Self {
            text: Cow::Borrowed(text),
            kind,
        }
    }

    /// A token holding text that is not in the line, such as a rewritten
    /// timestamp
    pub fn owned(text: String, kind: TokenKind) -> Self {
        Self {
            text: Cow::Owned(text),
            kind,
        }
    }

    /// The token's text between two byte offsets, in the same style
    pub fn slice(&self, start: usize, end: usize) -> Token<'a> {
        let text = match &self.text {
            Cow::Borrowed(text) => Cow::Borrowed(&text[start..end]),
            Cow::Owned(text) => Cow::Owned(text[start..end].to_string()),
        };

        Token {
            text,
            kind: self.kind,
        }
    }
}

/// One log line, parsed into styled tokens that borrow the line's text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedLine<'a> {
    pub tokens: Vec<Token<'a>>,
}

impl<'a> ParsedLine<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens }
    }

    /// The line text with all styling removed
    pub fn text(&self) -> String {
        self.tokens
            .iter()
            .map(|token| token.text.as_ref())
            .collect()
    }
}

/// Error returned when an unknown output mode is requested
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownOutputMode(pub String);

impl fmt::Display for UnknownOutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unknown output mode '{}' (expected ansi, csv, curses, html, json, jsonl, or plain){}",
            self.0,
            suggest::hint(&self.0, OUTPUT_NAMES)
        )
    }
}

impl std::error::Error for UnknownOutputMode {}

/// How rendered lines are written out
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    #[default]
    Ansi,
    Csv,
    Curses,
    Html,
    Json,
    Jsonl,
    Plain,
}

impl FromStr for OutputMode {
    type Err = UnknownOutputMode;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "ansi" | "color" => Ok(OutputMode::Ansi),
            "csv" => Ok(OutputMode::Csv),
            "curses" | "tui" => Ok(OutputMode::Curses),
            "html" => Ok(OutputMode::Html),
            "json" => Ok(OutputMode::Json),
            "jsonl" => Ok(OutputMode::Jsonl),
            "plain" | "none" => Ok(OutputMode::Plain),
            _ => Err(UnknownOutputMode(value.to_string())),
        }
    }
}

impl fmt::Display for OutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            OutputMode::Ansi => "ansi",
            OutputMode::Csv => "csv",
            OutputMode::Curses => "curses",
            OutputMode::Html => "html",
            OutputMode::Json => "json",
            OutputMode::Jsonl => "jsonl",
            OutputMode::Plain => "plain",
        };

        write!(f, "{}", name)
    }
}

impl OutputMode {
    /// Text written before the first line, if the format needs a preamble
    pub fn header(&self, theme: &Theme) -> Option<String> {
        self.header_with_fields(theme, &[])
    }

    /// Text written before the first line, with the fields `--fields` named,
    /// which give CSV its header row
    pub fn header_with_fields(&self, theme: &Theme, fields: &[String]) -> Option<String> {
        self.header_with_css(theme, fields, None)
    }

    /// The header, with a style sheet added to HTML output after the
    /// theme's rules, so its rules win
    pub fn header_with_css(
        &self,
        theme: &Theme,
        fields: &[String],
        css: Option<&str>,
    ) -> Option<String> {
        match self {
            OutputMode::Html => Some(html_header(theme, css)),
            OutputMode::Csv => Some(format!("{}\n", csv_row(csv_columns(fields)))),
            _ => None,
        }
    }

    /// Text written after the last line, if the format needs a closing
    pub fn footer(&self) -> Option<String> {
        match self {
            OutputMode::Html => Some(html_footer()),
            _ => None,
        }
    }

    /// Whether this mode takes over the terminal instead of writing a stream
    pub fn is_interactive(&self) -> bool {
        matches!(self, OutputMode::Curses)
    }

    /// Renders one parsed line in this mode, painted with `theme`
    pub fn render(&self, line: &ParsedLine<'_>, theme: &Theme) -> String {
        self.render_with_fields(line, theme, &[])
    }

    /// Renders one parsed line, writing only the fields `--fields` named in
    /// the formats keyed by field. No fields leaves JSON Lines every field and
    /// CSV the line text.
    pub fn render_with_fields(
        &self,
        line: &ParsedLine<'_>,
        theme: &Theme,
        fields: &[String],
    ) -> String {
        match self {
            OutputMode::Ansi | OutputMode::Curses => render_ansi(line, theme),
            OutputMode::Csv => render_csv(line, fields),
            OutputMode::Html => render_html(line),
            OutputMode::Json => render_json(line),
            OutputMode::Jsonl => render_jsonl(line, fields),
            OutputMode::Plain => line.text(),
        }
    }
}

fn render_ansi(line: &ParsedLine<'_>, theme: &Theme) -> String {
    line.tokens
        .iter()
        .map(|token| theme.colorize(token.kind, &visible_controls(&token.text)))
        .collect()
}

/// Text with its control characters written as text a terminal shows rather
/// than obeys, so a log line cannot move the cursor, clear the screen, or
/// set the window title: `^[` for escape and the other C0 controls in caret
/// notation, `^?` for delete, and `\x9b` for a C1 control. A tab is kept.
pub fn visible_controls(text: &str) -> Cow<'_, str> {
    if !text.chars().any(is_hidden_control) {
        return Cow::Borrowed(text);
    }

    let mut visible = String::with_capacity(text.len() + 8);

    for character in text.chars() {
        match character as u32 {
            _ if !is_hidden_control(character) => visible.push(character),
            code @ 0..=0x1f => {
                visible.push('^');
                visible.push(char::from(code as u8 + 0x40));
            }
            0x7f => visible.push_str("^?"),
            code => visible.push_str(&format!("\\x{:02x}", code)),
        }
    }

    Cow::Owned(visible)
}

fn is_hidden_control(character: char) -> bool {
    character != '\t' && character.is_control()
}

fn render_html(line: &ParsedLine<'_>) -> String {
    line.tokens
        .iter()
        .map(|token| {
            format!(
                "<span class=\"splash-{}\">{}</span>",
                token.kind.name(),
                escape_html(&token.text)
            )
        })
        .collect()
}

fn render_json(line: &ParsedLine<'_>) -> String {
    let tokens: Vec<String> = line
        .tokens
        .iter()
        .map(|token| {
            format!(
                "{{\"kind\":\"{}\",\"text\":\"{}\"}}",
                token.kind.name(),
                escape_json(&token.text)
            )
        })
        .collect();

    format!(
        "{{\"text\":\"{}\",\"tokens\":[{}]}}",
        escape_json(&logged_text(line)),
        tokens.join(",")
    )
}

/// The field that holds a line's text in the formats keyed by field
pub const TEXT_FIELD: &str = "text";

/// The field that holds the structure a line was written in
pub const RECORD_FIELD: &str = "record";

/// Whether `name` is a field `--fields` can name: the line text, its record,
/// or a token kind
pub fn is_field_name(name: &str) -> bool {
    name == TEXT_FIELD || name == RECORD_FIELD || TokenKind::from_name(name).is_some()
}

/// The structure a line was written in, as JSON: a line that is a JSON
/// object is kept as it is, nesting and value types included, and the
/// `key=value` pairs of a line become an object of strings. A line in
/// neither form has no record.
pub fn record(text: &str) -> Option<String> {
    let trimmed = text.trim();

    if json::parse_object(trimmed, |_, _| None).is_some() {
        return Some(trimmed.to_string());
    }

    let pairs = logfmt::pairs(text);

    if pairs.is_empty() {
        return None;
    }

    let members: Vec<String> = pairs
        .iter()
        .map(|(key, value)| format!("\"{}\":\"{}\"", escape_json(key), escape_json(value)))
        .collect();

    Some(format!("{{{}}}", members.join(",")))
}

/// The values of each field a line holds, keyed by kind, in the order the
/// kinds first appear. Plain text, punctuation, the keys of key-value pairs,
/// and tokens holding only whitespace are not fields.
fn line_fields<'l>(line: &'l ParsedLine<'_>) -> Vec<(&'static str, Vec<&'l str>)> {
    let mut fields: Vec<(&str, Vec<&str>)> = Vec::new();

    for token in &line.tokens {
        if !is_field_kind(token.kind) || token.text.trim().is_empty() {
            continue;
        }

        match fields
            .iter_mut()
            .find(|(name, _)| *name == token.kind.name())
        {
            Some((_, values)) => values.push(&token.text),
            None => fields.push((token.kind.name(), vec![&token.text])),
        }
    }

    fields
}

/// The line as the log wrote it, without the label naming the file a merged
/// line came from
fn logged_text(line: &ParsedLine<'_>) -> String {
    let skipped = match line.tokens.first() {
        Some(token) if token.kind == TokenKind::Source => 2,
        _ => 0,
    };

    line.tokens
        .iter()
        .skip(skipped)
        .map(|token| token.text.as_ref())
        .collect()
}

fn is_field_kind(kind: TokenKind) -> bool {
    !matches!(
        kind,
        TokenKind::Plain | TokenKind::Punctuation | TokenKind::Header
    )
}

/// One object holding the line text, each field the line holds, keyed by
/// its kind, and the line's record when it has one, or only the named
/// members the line holds, in the order named. A kind the line holds more
/// than once is an array of its values, in order.
fn render_jsonl(line: &ParsedLine<'_>, names: &[String]) -> String {
    let text = logged_text(line);

    let mut members: Vec<(&str, String)> = vec![(TEXT_FIELD, json_string(&text))];

    for (name, values) in line_fields(line) {
        let quoted: Vec<String> = values.iter().map(|value| json_string(value)).collect();

        match quoted.as_slice() {
            [single] => members.push((name, single.clone())),
            _ => members.push((name, format!("[{}]", quoted.join(",")))),
        }
    }

    if let Some(record) = record(&text) {
        members.push((RECORD_FIELD, record));
    }

    if !names.is_empty() {
        members = names
            .iter()
            .filter_map(|name| members.iter().find(|(member, _)| member == name).cloned())
            .collect();
    }

    let members: Vec<String> = members
        .iter()
        .map(|(name, value)| format!("\"{}\":{}", name, value))
        .collect();

    format!("{{{}}}", members.join(","))
}

fn json_string(text: &str) -> String {
    format!("\"{}\"", escape_json(text))
}

/// The columns CSV writes: the named fields, or the line text alone
fn csv_columns(names: &[String]) -> Vec<&str> {
    match names {
        [] => vec![TEXT_FIELD],
        _ => names.iter().map(String::as_str).collect(),
    }
}

/// One CSV row holding each named field of the line. A field the line holds
/// more than once has its values joined by spaces, and one it lacks is empty.
fn render_csv(line: &ParsedLine<'_>, names: &[String]) -> String {
    let text = logged_text(line);
    let fields = line_fields(line);

    let cells: Vec<String> = csv_columns(names)
        .into_iter()
        .map(|name| match name {
            TEXT_FIELD => text.clone(),
            RECORD_FIELD => record(&text).unwrap_or_default(),
            _ => fields
                .iter()
                .find(|(kind, _)| *kind == name)
                .map(|(_, values)| values.join(" "))
                .unwrap_or_default(),
        })
        .collect();

    csv_row(cells)
}

/// Joins cells into a CSV row, quoting a cell that holds a comma, a quote,
/// or a line break (RFC 4180)
fn csv_row<S: AsRef<str>>(cells: Vec<S>) -> String {
    let quoted: Vec<String> = cells
        .iter()
        .map(|cell| {
            let cell = cell.as_ref();

            if cell.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", cell.replace('"', "\"\""))
            } else {
                cell.to_string()
            }
        })
        .collect();

    quoted.join(",")
}

/// Escapes text for embedding in an HTML element
pub fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }

    escaped
}

/// Escapes text for embedding in a JSON string
pub fn escape_json(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());

    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                escaped.push_str(&format!("\\u{:04x}", control as u32))
            }
            _ => escaped.push(character),
        }
    }

    escaped
}

/// The class of the line number HTML output can start each line with
pub const LINE_NUMBER_CLASS: &str = "splash-line-number";

/// A line of HTML output after its line number
pub fn number_html_line(number: usize, html: &str) -> String {
    format!(
        "<span class=\"{}\" aria-hidden=\"true\">{}</span>{}",
        LINE_NUMBER_CLASS, number, html
    )
}

fn html_header(theme: &Theme, css: Option<&str>) -> String {
    let rules: String = TokenKind::all()
        .iter()
        .map(|kind| {
            let weight = if theme.is_bold(*kind) {
                " font-weight: bold;"
            } else {
                ""
            };

            format!(
                "    .splash-{} {{ color: {};{} }}\n",
                kind.name(),
                theme.css(*kind),
                weight
            )
        })
        .collect();

    format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"utf-8\">\n\
         <title>splash</title>\n\
         <style>\n\
         \x20   body {{ background: {}; color: {}; }}\n\
         \x20   pre.splash {{ font-family: monospace; white-space: pre-wrap; }}\n\
         \x20   .{} {{ display: inline-block; min-width: 6ch; margin-right: 1ch; text-align: right; opacity: 0.6; user-select: none; }}\n\
         {}\
         {}\
         </style>\n\
         </head>\n\
         <body>\n\
         <main>\n\
         <pre class=\"splash\" role=\"log\" aria-label=\"Log lines\">\n",
        theme.background().css(),
        theme.foreground().css(),
        LINE_NUMBER_CLASS,
        rules,
        css.map(custom_css).unwrap_or_default()
    )
}

/// A style sheet as it goes inside the header's style element, ending with
/// a line break
fn custom_css(css: &str) -> String {
    match css.ends_with('\n') {
        true => css.to_string(),
        false => format!("{}\n", css),
    }
}

fn html_footer() -> String {
    "</pre>\n</main>\n</body>\n</html>\n".to_string()
}
