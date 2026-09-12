//! Color themes
//!
//! A theme assigns one style to every token kind, plus the page colors used by
//! HTML output. Presets ship with splash and any style can be overridden from
//! a config file, from a saved profile, or from the command line.
use crate::color::{Color, Style, UnknownColor};
use crate::output::TokenKind;
use std::fmt;

/// Names of the built-in presets, in the order `--theme` lists them
pub const PRESETS: [&str; 4] = ["dark", "light", "solarized", "dracula"];

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
                "Unknown theme '{}' (expected one of {})",
                name,
                PRESETS.join(", ")
            ),
            ThemeError::UnknownTokenKind(name) => {
                write!(f, "Unknown color key '{}'", name)
            }
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

    fn build(name: &str, background: Color, foreground: Color, styles: [Style; 17]) -> Theme {
        Theme {
            name: name.to_string(),
            styles,
            background,
            foreground,
        }
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
            ],
        )
    }
}
