//! Color themes
//!
//! A theme assigns one style to every token kind, plus the page colors used by
//! HTML output. Presets ship with splash and any style can be overridden from
//! a config file, from a saved profile, or from the command line.
use crate::color::{Color, Style, UnknownColor};
use crate::output::TokenKind;
use crate::suggest;
use std::fmt;

/// Names of the built-in presets, in the order `--theme` lists them
pub const PRESETS: [&str; 6] = [
    "dark",
    "light",
    "solarized",
    "dracula",
    "colorblind",
    "high-contrast",
];

/// The Okabe-Ito palette, whose colors stay apart for readers with
/// protanopia, deuteranopia, and tritanopia
pub const OKABE_ITO: [Color; 8] = [
    Color::Rgb(0x00, 0x00, 0x00),
    Color::Rgb(0xe6, 0x9f, 0x00),
    Color::Rgb(0x56, 0xb4, 0xe9),
    Color::Rgb(0x00, 0x9e, 0x73),
    Color::Rgb(0xf0, 0xe4, 0x42),
    Color::Rgb(0x00, 0x72, 0xb2),
    Color::Rgb(0xd5, 0x5e, 0x00),
    Color::Rgb(0xcc, 0x79, 0xa7),
];

/// What a token kind holds, which the role-based presets color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Text,
    Muted,
    Time,
    Address,
    Number,
    Method,
    Request,
    Status,
    Level,
    Identity,
    Name,
    Success,
    Warning,
    Failure,
    Highlight,
    Header,
}

impl Role {
    fn of(kind: TokenKind) -> Role {
        match kind {
            TokenKind::Plain | TokenKind::Message => Role::Text,
            TokenKind::Punctuation
            | TokenKind::HttpVersion
            | TokenKind::UserIdentifier
            | TokenKind::Protocol
            | TokenKind::UserAgent
            | TokenKind::ContentType => Role::Muted,
            TokenKind::DateTime | TokenKind::TimezoneOffset | TokenKind::Timestamp => Role::Time,
            TokenKind::Ip | TokenKind::Client | TokenKind::VirtualHost | TokenKind::Host => {
                Role::Address
            }
            TokenKind::Number
            | TokenKind::Size
            | TokenKind::Duration
            | TokenKind::Pid
            | TokenKind::Timers
            | TokenKind::Counters => Role::Number,
            TokenKind::HttpVerb | TokenKind::Method => Role::Method,
            TokenKind::Request | TokenKind::Referer | TokenKind::Path => Role::Request,
            TokenKind::Status | TokenKind::CacheResult | TokenKind::Termination => Role::Status,
            TokenKind::Level => Role::Level,
            TokenKind::UserId | TokenKind::Transaction | TokenKind::Email | TokenKind::QueueId => {
                Role::Identity
            }
            TokenKind::Module
            | TokenKind::Hierarchy
            | TokenKind::Tag
            | TokenKind::Frontend
            | TokenKind::Backend
            | TokenKind::Server
            | TokenKind::Keyword
            | TokenKind::Service
            | TokenKind::Location => Role::Name,
            TokenKind::Success | TokenKind::CacheHit => Role::Success,
            TokenKind::Warning | TokenKind::CacheMiss => Role::Warning,
            TokenKind::Failure | TokenKind::UnknownUser => Role::Failure,
            TokenKind::Match | TokenKind::Capture => Role::Highlight,
            TokenKind::Header | TokenKind::Source => Role::Header,
        }
    }
}

/// Error returned when a theme cannot be built
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    UnknownTheme(String),
    UnknownTokenKind(String),
    UnknownColor(UnknownColor),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::UnknownTheme(name) => write!(
                f,
                "Unknown theme '{}' (expected one of {}){}",
                name,
                PRESETS.join(", "),
                suggest::hint(name, PRESETS)
            ),
            ThemeError::UnknownTokenKind(name) => write!(
                f,
                "Unknown color key '{}'{}",
                name,
                suggest::hint(name, TokenKind::names())
            ),
            ThemeError::UnknownColor(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for ThemeError {}

impl From<UnknownColor> for ThemeError {
    fn from(error: UnknownColor) -> Self {
        ThemeError::UnknownColor(error)
    }
}

/// A full set of token styles under a name
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    name: String,
    styles: [Style; TokenKind::COUNT],
    background: Color,
    foreground: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}

impl Theme {
    /// The preset with this name
    pub fn preset(name: &str) -> Result<Theme, ThemeError> {
        match name.to_ascii_lowercase().as_str() {
            "dark" | "default" => Ok(Theme::dark()),
            "light" => Ok(Theme::light()),
            "solarized" => Ok(Theme::solarized()),
            "dracula" => Ok(Theme::dracula()),
            "colorblind" => Ok(Theme::colorblind()),
            "high-contrast" => Ok(Theme::high_contrast()),
            _ => Err(ThemeError::UnknownTheme(name.to_string())),
        }
    }

    /// The theme's name, as written back out when a profile is saved
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The style this theme paints `kind` in
    pub fn style(&self, kind: TokenKind) -> Style {
        self.styles[kind.index()]
    }

    /// Replaces the style for one token kind
    pub fn set_style(&mut self, kind: TokenKind, style: Style) {
        self.styles[kind.index()] = style;
    }

    /// The page background used by HTML output
    pub fn background(&self) -> Color {
        self.background
    }

    /// The page text color used by HTML output
    pub fn foreground(&self) -> Color {
        self.foreground
    }

    /// The token text wrapped in this theme's ANSI escapes
    pub fn colorize(&self, kind: TokenKind, text: &str) -> String {
        self.style(kind).colorize(text)
    }

    /// The hex color this theme uses for `kind` in HTML output
    pub fn css(&self, kind: TokenKind) -> String {
        self.style(kind).color.css()
    }

    /// Whether this theme renders `kind` in bold
    pub fn is_bold(&self, kind: TokenKind) -> bool {
        self.style(kind).bold
    }

    /// Applies one `key=value` style override, such as `ip=bright cyan`
    pub fn apply_override(&mut self, key: &str, value: &str) -> Result<(), ThemeError> {
        let kind = TokenKind::from_name(key)
            .ok_or_else(|| ThemeError::UnknownTokenKind(key.to_string()))?;

        self.set_style(kind, Style::parse(value)?);

        Ok(())
    }

    /// Applies a set of `key=value` style overrides in the order given
    pub fn apply_overrides<'a, I>(&mut self, overrides: I) -> Result<(), ThemeError>
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        for (key, value) in overrides {
            self.apply_override(key, value)?;
        }

        Ok(())
    }

    /// The theme written as a config file holding a `[colors]` table
    pub fn to_toml(&self) -> String {
        let mut toml = format!("theme = \"{}\"\n\n[colors]\n", self.name);

        for kind in TokenKind::all() {
            toml.push_str(&format!("{} = \"{}\"\n", kind.name(), self.style(kind)));
        }

        toml
    }

    fn build(
        name: &str,
        background: Color,
        foreground: Color,
        styles: [Style; TokenKind::COUNT],
    ) -> Theme {
        Theme {
            name: name.to_string(),
            styles,
            background,
            foreground,
        }
    }

    /// A theme painting each token kind in the style of its role
    fn from_roles(
        name: &str,
        background: Color,
        foreground: Color,
        style: impl Fn(Role) -> Style,
    ) -> Theme {
        Theme::build(
            name,
            background,
            foreground,
            TokenKind::all().map(|kind| style(Role::of(kind))),
        )
    }

    /// Colors from the Okabe-Ito palette on a dark terminal, with success
    /// sky blue and failure vermillion where other themes use green and red
    pub fn colorblind() -> Theme {
        let [_, orange, sky_blue, bluish_green, yellow, _, vermillion, reddish_purple] = OKABE_ITO;
        let gray = Color::Rgb(0x99, 0x99, 0x99);

        Theme::from_roles(
            "colorblind",
            Color::Rgb(0x1e, 0x1e, 0x1e),
            Color::Default,
            |role| match role {
                Role::Text => Style::new(Color::Default),
                Role::Muted => Style::new(gray),
                Role::Time => Style::new(sky_blue),
                Role::Address => Style::new(reddish_purple),
                Role::Number => Style::new(yellow),
                Role::Method => Style::bold(bluish_green),
                Role::Request => Style::new(bluish_green),
                Role::Status => Style::new(orange),
                Role::Level => Style::bold(orange),
                Role::Identity => Style::bold(Color::Default),
                Role::Name => Style::bold(reddish_purple),
                Role::Success => Style::bold(sky_blue),
                Role::Warning => Style::bold(yellow),
                Role::Failure => Style::bold(vermillion),
                Role::Highlight => Style::bold(Color::BrightWhite),
                Role::Header => Style::bold(Color::Default),
            },
        )
    }

    /// Light colors on black, each at least seven times as bright as the
    /// background by the WCAG contrast ratio, with the fields that report
    /// an outcome in bold
    pub fn high_contrast() -> Theme {
        let white = Color::Rgb(0xff, 0xff, 0xff);
        let yellow = Color::Rgb(0xff, 0xff, 0x00);
        let cyan = Color::Rgb(0x00, 0xff, 0xff);
        let green = Color::Rgb(0x00, 0xff, 0x00);
        let red = Color::Rgb(0xff, 0x80, 0x80);
        let orange = Color::Rgb(0xff, 0xa5, 0x00);
        let magenta = Color::Rgb(0xff, 0x80, 0xff);
        let blue = Color::Rgb(0x80, 0xc0, 0xff);

        Theme::from_roles(
            "high-contrast",
            Color::Rgb(0x00, 0x00, 0x00),
            white,
            |role| match role {
                Role::Text | Role::Muted => Style::new(white),
                Role::Time => Style::new(cyan),
                Role::Address => Style::new(magenta),
                Role::Number => Style::new(yellow),
                Role::Method => Style::bold(blue),
                Role::Request => Style::new(blue),
                Role::Status => Style::new(orange),
                Role::Level => Style::bold(orange),
                Role::Identity | Role::Header | Role::Highlight => Style::bold(white),
                Role::Name => Style::bold(magenta),
                Role::Success => Style::bold(green),
                Role::Warning => Style::bold(yellow),
                Role::Failure => Style::bold(red),
            },
        )
    }

    /// The default theme: bright ANSI colors on a dark terminal
    pub fn dark() -> Theme {
        Theme::build(
            "dark",
            Color::Rgb(0x1e, 0x1e, 0x1e),
            Color::Default,
            [
                Style::new(Color::Default),
                Style::new(Color::BrightWhite),
                Style::new(Color::BrightRed),
                Style::new(Color::BrightBlue),
                Style::new(Color::Cyan),
                Style::new(Color::Cyan),
                Style::new(Color::BrightGreen),
                Style::new(Color::Cyan),
                Style::new(Color::BrightRed),
                Style::new(Color::White),
                Style::bold(Color::White),
                Style::new(Color::BrightMagenta),
                Style::new(Color::BrightCyan),
                Style::new(Color::Cyan),
                Style::new(Color::Cyan),
                Style::new(Color::BrightYellow),
                Style::new(Color::BrightGreen),
                Style::new(Color::Blue),
                Style::new(Color::BrightBlack),
                Style::new(Color::BrightMagenta),
                Style::new(Color::Magenta),
                Style::bold(Color::BrightYellow),
                Style::new(Color::BrightBlue),
                Style::new(Color::Magenta),
                Style::new(Color::White),
                Style::bold(Color::BrightGreen),
                Style::new(Color::BrightYellow),
                Style::new(Color::White),
                Style::new(Color::Cyan),
                Style::new(Color::BrightBlue),
                Style::bold(Color::BrightMagenta),
                Style::new(Color::BrightCyan),
                Style::new(Color::BrightBlue),
                Style::new(Color::Cyan),
                Style::new(Color::BrightBlue),
                Style::new(Color::BrightMagenta),
                Style::new(Color::Magenta),
                Style::new(Color::Cyan),
                Style::bold(Color::BrightRed),
                Style::new(Color::Blue),
                Style::new(Color::BrightCyan),
                Style::new(Color::BrightMagenta),
                Style::new(Color::Green),
                Style::bold(Color::BrightGreen),
                Style::bold(Color::BrightYellow),
                Style::bold(Color::BrightRed),
                Style::bold(Color::BrightRed),
                Style::bold(Color::BrightYellow),
                Style::new(Color::BrightCyan),
                Style::bold(Color::BrightRed),
                Style::new(Color::BrightGreen),
                Style::new(Color::BrightYellow),
                Style::bold(Color::Blue),
            ],
        )
    }

    /// Darker colors for a light terminal or a printed page
    pub fn light() -> Theme {
        Theme::build(
            "light",
            Color::BrightWhite,
            Color::Black,
            [
                Style::new(Color::Black),
                Style::new(Color::BrightBlack),
                Style::new(Color::Red),
                Style::new(Color::Blue),
                Style::new(Color::Cyan),
                Style::new(Color::Cyan),
                Style::new(Color::Green),
                Style::new(Color::Cyan),
                Style::new(Color::Red),
                Style::new(Color::BrightBlack),
                Style::bold(Color::Black),
                Style::new(Color::Magenta),
                Style::new(Color::Blue),
                Style::new(Color::Cyan),
                Style::new(Color::Cyan),
                Style::new(Color::Yellow),
                Style::new(Color::Green),
                Style::new(Color::Blue),
                Style::new(Color::BrightBlack),
                Style::new(Color::Magenta),
                Style::new(Color::Magenta),
                Style::bold(Color::Yellow),
                Style::new(Color::Blue),
                Style::new(Color::Magenta),
                Style::new(Color::Black),
                Style::bold(Color::Green),
                Style::new(Color::Yellow),
                Style::new(Color::BrightBlack),
                Style::new(Color::Cyan),
                Style::new(Color::Blue),
                Style::bold(Color::Magenta),
                Style::new(Color::Cyan),
                Style::new(Color::Blue),
                Style::new(Color::Cyan),
                Style::new(Color::Blue),
                Style::new(Color::Magenta),
                Style::new(Color::Magenta),
                Style::new(Color::Cyan),
                Style::bold(Color::Red),
                Style::new(Color::Blue),
                Style::new(Color::Cyan),
                Style::new(Color::Magenta),
                Style::new(Color::Green),
                Style::bold(Color::Green),
                Style::bold(Color::Yellow),
                Style::bold(Color::Red),
                Style::bold(Color::Red),
                Style::bold(Color::Magenta),
                Style::new(Color::Blue),
                Style::bold(Color::Red),
                Style::new(Color::Green),
                Style::new(Color::Yellow),
                Style::bold(Color::Blue),
            ],
        )
    }

    /// The Solarized Dark palette
    pub fn solarized() -> Theme {
        let base01 = Color::Rgb(0x58, 0x6e, 0x75);
        let base0 = Color::Rgb(0x83, 0x94, 0x96);
        let base1 = Color::Rgb(0x93, 0xa1, 0xa1);
        let yellow = Color::Rgb(0xb5, 0x89, 0x00);
        let red = Color::Rgb(0xdc, 0x32, 0x2f);
        let violet = Color::Rgb(0x6c, 0x71, 0xc4);
        let blue = Color::Rgb(0x26, 0x8b, 0xd2);
        let cyan = Color::Rgb(0x2a, 0xa1, 0x98);
        let green = Color::Rgb(0x85, 0x99, 0x00);

        Theme::build(
            "solarized",
            Color::Rgb(0x00, 0x2b, 0x36),
            base0,
            [
                Style::new(base0),
                Style::new(base1),
                Style::new(red),
                Style::new(blue),
                Style::new(cyan),
                Style::new(cyan),
                Style::new(green),
                Style::new(cyan),
                Style::new(red),
                Style::new(base01),
                Style::bold(base1),
                Style::new(violet),
                Style::new(blue),
                Style::new(cyan),
                Style::new(cyan),
                Style::new(yellow),
                Style::new(green),
                Style::new(blue),
                Style::new(base01),
                Style::new(violet),
                Style::new(violet),
                Style::bold(yellow),
                Style::new(blue),
                Style::new(violet),
                Style::new(base0),
                Style::bold(green),
                Style::new(yellow),
                Style::new(base01),
                Style::new(cyan),
                Style::new(blue),
                Style::bold(violet),
                Style::new(cyan),
                Style::new(blue),
                Style::new(cyan),
                Style::new(blue),
                Style::new(violet),
                Style::new(violet),
                Style::new(cyan),
                Style::bold(red),
                Style::new(blue),
                Style::new(cyan),
                Style::new(violet),
                Style::new(green),
                Style::bold(green),
                Style::bold(yellow),
                Style::bold(red),
                Style::bold(red),
                Style::bold(yellow),
                Style::new(cyan),
                Style::bold(red),
                Style::new(green),
                Style::new(yellow),
                Style::bold(violet),
            ],
        )
    }

    /// The Dracula palette
    pub fn dracula() -> Theme {
        let foreground = Color::Rgb(0xf8, 0xf8, 0xf2);
        let comment = Color::Rgb(0x62, 0x72, 0xa4);
        let pink = Color::Rgb(0xff, 0x79, 0xc6);
        let purple = Color::Rgb(0xbd, 0x93, 0xf9);
        let green = Color::Rgb(0x50, 0xfa, 0x7b);
        let cyan = Color::Rgb(0x8b, 0xe9, 0xfd);
        let red = Color::Rgb(0xff, 0x55, 0x55);
        let yellow = Color::Rgb(0xf1, 0xfa, 0x8c);

        Theme::build(
            "dracula",
            Color::Rgb(0x28, 0x2a, 0x36),
            foreground,
            [
                Style::new(foreground),
                Style::new(comment),
                Style::new(red),
                Style::new(purple),
                Style::new(cyan),
                Style::new(cyan),
                Style::new(green),
                Style::new(cyan),
                Style::new(red),
                Style::new(comment),
                Style::bold(foreground),
                Style::new(pink),
                Style::new(cyan),
                Style::new(cyan),
                Style::new(cyan),
                Style::new(yellow),
                Style::new(green),
                Style::new(purple),
                Style::new(comment),
                Style::new(pink),
                Style::new(pink),
                Style::bold(yellow),
                Style::new(purple),
                Style::new(pink),
                Style::new(foreground),
                Style::bold(green),
                Style::new(yellow),
                Style::new(comment),
                Style::new(cyan),
                Style::new(purple),
                Style::bold(pink),
                Style::new(cyan),
                Style::new(purple),
                Style::new(cyan),
                Style::new(purple),
                Style::new(pink),
                Style::new(pink),
                Style::new(cyan),
                Style::bold(red),
                Style::new(purple),
                Style::new(cyan),
                Style::new(purple),
                Style::new(green),
                Style::bold(green),
                Style::bold(yellow),
                Style::bold(red),
                Style::bold(red),
                Style::bold(yellow),
                Style::new(cyan),
                Style::bold(red),
                Style::new(green),
                Style::new(yellow),
                Style::bold(comment),
            ],
        )
    }
}
