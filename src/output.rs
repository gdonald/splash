/// Output modes and renderers
///
/// A parsed line is a sequence of styled tokens. Each output mode renders that
/// same sequence differently: ANSI escapes for terminals, HTML for a browser,
/// JSON for other programs, and plain text for scripting.
use crate::theme::Theme;
use std::fmt;
use std::str::FromStr;

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
        }
    }

    /// The number of distinct token styles
    pub const COUNT: usize = 17;

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
        ]
    }
}

/// A run of text with one style, borrowed from the line it was parsed from
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    pub text: &'a str,
    pub kind: TokenKind,
}

impl<'a> Token<'a> {
    pub fn new(text: &'a str, kind: TokenKind) -> Self {
        Self { text, kind }
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
        self.tokens.iter().map(|token| token.text).collect()
    }
}

/// Error returned when an unknown output mode is requested
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownOutputMode(pub String);

impl fmt::Display for UnknownOutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unknown output mode '{}' (expected ansi, curses, html, json, or plain)",
            self.0
        )
    }
}

impl std::error::Error for UnknownOutputMode {}

/// How rendered lines are written out
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    #[default]
    Ansi,
    Curses,
    Html,
    Json,
    Plain,
}

impl FromStr for OutputMode {
    type Err = UnknownOutputMode;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "ansi" | "color" => Ok(OutputMode::Ansi),
            "curses" | "tui" => Ok(OutputMode::Curses),
            "html" => Ok(OutputMode::Html),
            "json" => Ok(OutputMode::Json),
            "plain" | "none" => Ok(OutputMode::Plain),
            _ => Err(UnknownOutputMode(value.to_string())),
        }
    }
}

impl fmt::Display for OutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            OutputMode::Ansi => "ansi",
            OutputMode::Curses => "curses",
            OutputMode::Html => "html",
            OutputMode::Json => "json",
            OutputMode::Plain => "plain",
        };

        write!(f, "{}", name)
    }
}

impl OutputMode {
    /// Text written before the first line, if the format needs a preamble
    pub fn header(&self, theme: &Theme) -> Option<String> {
        match self {
            OutputMode::Html => Some(html_header(theme)),
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
        match self {
            OutputMode::Ansi | OutputMode::Curses => render_ansi(line, theme),
            OutputMode::Html => render_html(line),
            OutputMode::Json => render_json(line),
            OutputMode::Plain => line.text(),
        }
    }
}

fn render_ansi(line: &ParsedLine<'_>, theme: &Theme) -> String {
    line.tokens
        .iter()
        .map(|token| theme.colorize(token.kind, token.text))
        .collect()
}

fn render_html(line: &ParsedLine<'_>) -> String {
    line.tokens
        .iter()
        .map(|token| {
            format!(
                "<span class=\"splash-{}\">{}</span>",
                token.kind.name(),
                escape_html(token.text)
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
                escape_json(token.text)
            )
        })
        .collect();

    format!(
        "{{\"text\":\"{}\",\"tokens\":[{}]}}",
        escape_json(&line.text()),
        tokens.join(",")
    )
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

fn html_header(theme: &Theme) -> String {
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
         {}\
         </style>\n\
         </head>\n\
         <body>\n\
         <pre class=\"splash\">\n",
        theme.background().css(),
        theme.foreground().css(),
        rules
    )
}

fn html_footer() -> String {
    "</pre>\n</body>\n</html>\n".to_string()
}
