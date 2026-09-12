//! Colors and text styles used by themes
//!
//! A style is a color plus an optional bold flag. The same style renders as
//! ANSI escapes for the terminal and as a hex color for HTML output.
use colored::Colorize;
use std::fmt;

/// A color a token can be painted in
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Default,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Rgb(u8, u8, u8),
}

/// Error returned when a color or style cannot be understood
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownColor(pub String);

impl fmt::Display for UnknownColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unknown color '{}' (expected a name such as red or bright cyan, or a hex value such as #ff5555)",
            self.0
        )
    }
}

impl std::error::Error for UnknownColor {}

impl Color {
    /// Reads a color name such as `red`, `bright cyan`, or a hex value `#ff5555`
    pub fn parse(value: &str) -> Result<Color, UnknownColor> {
        let normalized = normalize(value);

        if let Some(hex) = normalized.strip_prefix('#') {
            return parse_hex(hex).ok_or_else(|| UnknownColor(value.to_string()));
        }

        match normalized.as_str() {
            "default" | "normal" => Ok(Color::Default),
            "black" => Ok(Color::Black),
            "red" => Ok(Color::Red),
            "green" => Ok(Color::Green),
            "yellow" => Ok(Color::Yellow),
            "blue" => Ok(Color::Blue),
            "magenta" | "purple" => Ok(Color::Magenta),
            "cyan" => Ok(Color::Cyan),
            "white" => Ok(Color::White),
            "gray" | "brightblack" => Ok(Color::BrightBlack),
            "brightred" => Ok(Color::BrightRed),
            "brightgreen" => Ok(Color::BrightGreen),
            "brightyellow" => Ok(Color::BrightYellow),
            "brightblue" => Ok(Color::BrightBlue),
            "brightmagenta" | "brightpurple" => Ok(Color::BrightMagenta),
            "brightcyan" => Ok(Color::BrightCyan),
            "brightwhite" => Ok(Color::BrightWhite),
            _ => Err(UnknownColor(value.to_string())),
        }
    }

    /// The hex color used for this color in HTML output
    pub fn css(&self) -> String {
        match self {
            Color::Default => "#d4d4d4".to_string(),
            Color::Black => "#000000".to_string(),
            Color::Red => "#aa0000".to_string(),
            Color::Green => "#00aa00".to_string(),
            Color::Yellow => "#aa5500".to_string(),
            Color::Blue => "#0000aa".to_string(),
            Color::Magenta => "#aa00aa".to_string(),
            Color::Cyan => "#00aaaa".to_string(),
            Color::White => "#aaaaaa".to_string(),
            Color::BrightBlack => "#555555".to_string(),
            Color::BrightRed => "#ff5555".to_string(),
            Color::BrightGreen => "#55ff55".to_string(),
            Color::BrightYellow => "#ffff55".to_string(),
            Color::BrightBlue => "#5555ff".to_string(),
            Color::BrightMagenta => "#ff55ff".to_string(),
            Color::BrightCyan => "#55ffff".to_string(),
            Color::BrightWhite => "#ffffff".to_string(),
            Color::Rgb(red, green, blue) => format!("#{:02x}{:02x}{:02x}", red, green, blue),
        }
    }

    fn ansi(&self) -> Option<colored::Color> {
        match self {
            Color::Default => None,
            Color::Black => Some(colored::Color::Black),
            Color::Red => Some(colored::Color::Red),
            Color::Green => Some(colored::Color::Green),
            Color::Yellow => Some(colored::Color::Yellow),
            Color::Blue => Some(colored::Color::Blue),
            Color::Magenta => Some(colored::Color::Magenta),
            Color::Cyan => Some(colored::Color::Cyan),
            Color::White => Some(colored::Color::White),
            Color::BrightBlack => Some(colored::Color::BrightBlack),
            Color::BrightRed => Some(colored::Color::BrightRed),
            Color::BrightGreen => Some(colored::Color::BrightGreen),
            Color::BrightYellow => Some(colored::Color::BrightYellow),
            Color::BrightBlue => Some(colored::Color::BrightBlue),
            Color::BrightMagenta => Some(colored::Color::BrightMagenta),
            Color::BrightCyan => Some(colored::Color::BrightCyan),
            Color::BrightWhite => Some(colored::Color::BrightWhite),
            Color::Rgb(red, green, blue) => Some(colored::Color::TrueColor {
                r: *red,
                g: *green,
                b: *blue,
            }),
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Color::Default => "default",
            Color::Black => "black",
            Color::Red => "red",
            Color::Green => "green",
            Color::Yellow => "yellow",
            Color::Blue => "blue",
            Color::Magenta => "magenta",
            Color::Cyan => "cyan",
            Color::White => "white",
            Color::BrightBlack => "bright black",
            Color::BrightRed => "bright red",
            Color::BrightGreen => "bright green",
            Color::BrightYellow => "bright yellow",
            Color::BrightBlue => "bright blue",
            Color::BrightMagenta => "bright magenta",
            Color::BrightCyan => "bright cyan",
            Color::BrightWhite => "bright white",
            Color::Rgb(_, _, _) => return write!(f, "{}", self.css()),
        };

        write!(f, "{}", name)
    }
}

/// A color together with the attributes applied on top of it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub color: Color,
    pub bold: bool,
}

impl Style {
    /// A style painting text in `color` with no attributes
    pub const fn new(color: Color) -> Self {
        Self { color, bold: false }
    }

    /// A style painting text in `color` and rendering it bold
    pub const fn bold(color: Color) -> Self {
        Self { color, bold: true }
    }

    /// Reads a style such as `bright cyan` or `white bold`
    pub fn parse(value: &str) -> Result<Style, UnknownColor> {
        let normalized = normalize(value);
        let mut bold = false;

        let words: Vec<&str> = normalized
            .split(' ')
            .filter(|word| {
                if *word == "bold" {
                    bold = true;
                    return false;
                }

                !word.is_empty()
            })
            .collect();

        if words.is_empty() {
            return Err(UnknownColor(value.to_string()));
        }

        Ok(Style {
            color: Color::parse(&words.join(" "))?,
            bold,
        })
    }

    /// The text wrapped in the ANSI escapes for this style
    pub fn colorize(&self, text: &str) -> String {
        let colored = match self.color.ansi() {
            Some(color) => text.color(color),
            None => text.normal(),
        };

        if self.bold {
            colored.bold().to_string()
        } else {
            colored.to_string()
        }
    }
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.bold {
            write!(f, "{} bold", self.color)
        } else {
            write!(f, "{}", self.color)
        }
    }
}

/// Lowercases, and treats underscores and hyphens as spaces, so that
/// `bright_red`, `bright-red`, and `Bright Red` all read the same
fn normalize(value: &str) -> String {
    let spaced: String = value
        .chars()
        .map(|character| match character {
            '_' | '-' => ' ',
            other => other.to_ascii_lowercase(),
        })
        .collect();

    let words: Vec<&str> = spaced.split_whitespace().collect();

    if words.len() > 1 && words[0] == "bright" {
        return format!("bright{}", words[1..].join(" "));
    }

    words.join(" ")
}

fn parse_hex(hex: &str) -> Option<Color> {
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let channel = |start: usize| u8::from_str_radix(&hex[start..start + 2], 16).ok();

    Some(Color::Rgb(channel(0)?, channel(2)?, channel(4)?))
}
