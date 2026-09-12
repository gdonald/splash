use splash::color::{Color, Style, UnknownColor};
use std::error::Error;

const NAMED_COLORS: [(&str, Color); 19] = [
    ("default", Color::Default),
    ("normal", Color::Default),
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
    ("yellow", Color::Yellow),
    ("blue", Color::Blue),
    ("magenta", Color::Magenta),
    ("purple", Color::Magenta),
    ("cyan", Color::Cyan),
    ("white", Color::White),
    ("gray", Color::BrightBlack),
    ("bright black", Color::BrightBlack),
    ("bright red", Color::BrightRed),
    ("bright green", Color::BrightGreen),
    ("bright yellow", Color::BrightYellow),
    ("bright blue", Color::BrightBlue),
    ("bright magenta", Color::BrightMagenta),
    ("bright white", Color::BrightWhite),
];

#[test]
fn every_named_color_parses_to_its_color() {
    for (name, expected) in NAMED_COLORS {
        assert_eq!(Color::parse(name), Ok(expected), "parsing {}", name);
    }
}

#[test]
fn bright_purple_is_bright_magenta() {
    assert_eq!(Color::parse("bright purple"), Ok(Color::BrightMagenta));
}

#[test]
fn bright_cyan_parses_to_bright_cyan() {
    assert_eq!(Color::parse("bright cyan"), Ok(Color::BrightCyan));
}

#[test]
fn color_names_ignore_case_underscores_and_hyphens() {
    assert_eq!(Color::parse("Bright_Red"), Ok(Color::BrightRed));
    assert_eq!(Color::parse("BRIGHT-RED"), Ok(Color::BrightRed));
    assert_eq!(Color::parse("brightred"), Ok(Color::BrightRed));
}

#[test]
fn a_hex_value_parses_to_its_channels() {
    assert_eq!(Color::parse("#ff8000"), Ok(Color::Rgb(255, 128, 0)));
}

#[test]
fn a_hex_value_of_the_wrong_length_is_rejected() {
    assert_eq!(
        Color::parse("#ff80"),
        Err(UnknownColor("#ff80".to_string()))
    );
}

#[test]
fn a_hex_value_with_a_non_hex_digit_is_rejected() {
    assert_eq!(
        Color::parse("#gg0000"),
        Err(UnknownColor("#gg0000".to_string()))
    );
}

#[test]
fn an_unknown_color_name_is_rejected() {
    assert_eq!(
        Color::parse("mauve"),
        Err(UnknownColor("mauve".to_string()))
    );
}

#[test]
fn an_unknown_color_names_the_value_it_could_not_read() {
    let error = UnknownColor("mauve".to_string());

    assert_eq!(
        error.to_string(),
        "Unknown color 'mauve' (expected a name such as red or bright cyan, or a hex value such as #ff5555)"
    );
}

#[test]
fn an_unknown_color_is_an_error() {
    let error: Box<dyn Error> = Box::new(UnknownColor("mauve".to_string()));

    assert!(error.source().is_none());
}

#[test]
fn every_named_color_has_a_six_digit_css_value() {
    for (name, color) in NAMED_COLORS {
        let css = color.css();

        assert!(
            css.starts_with('#') && css.len() == 7,
            "{} has css value {}",
            name,
            css
        );
    }
}

#[test]
fn the_remaining_colors_have_six_digit_css_values() {
    for color in [Color::BrightCyan, Color::Rgb(0, 17, 34)] {
        let css = color.css();

        assert!(css.starts_with('#') && css.len() == 7, "css value {}", css);
    }
}

#[test]
fn an_rgb_color_writes_its_channels_as_hex() {
    assert_eq!(Color::Rgb(0, 17, 255).css(), "#0011ff");
}

#[test]
fn every_color_prints_the_name_it_parses_from() {
    for (name, color) in NAMED_COLORS {
        if name == "normal" || name == "purple" || name == "gray" {
            continue;
        }

        assert_eq!(color.to_string(), name);
    }

    assert_eq!(Color::BrightCyan.to_string(), "bright cyan");
}

#[test]
fn an_rgb_color_prints_its_hex_value() {
    assert_eq!(Color::Rgb(255, 128, 0).to_string(), "#ff8000");
}

#[test]
fn a_style_parses_a_bare_color_name() {
    assert_eq!(Style::parse("cyan"), Ok(Style::new(Color::Cyan)));
}

#[test]
fn a_style_parses_a_bold_color_name() {
    assert_eq!(Style::parse("white bold"), Ok(Style::bold(Color::White)));
}

#[test]
fn a_style_accepts_bold_before_the_color() {
    assert_eq!(
        Style::parse("bold bright red"),
        Ok(Style::bold(Color::BrightRed))
    );
}

#[test]
fn a_style_of_only_bold_is_rejected() {
    assert_eq!(Style::parse("bold"), Err(UnknownColor("bold".to_string())));
}

#[test]
fn a_style_with_an_unknown_color_is_rejected() {
    assert_eq!(
        Style::parse("mauve bold"),
        Err(UnknownColor("mauve".to_string()))
    );
}

#[test]
fn a_styles_text_survives_colorizing() {
    colored::control::set_override(true);

    assert!(Style::new(Color::Red).colorize("sample").contains("sample"));
}

#[test]
fn a_colored_style_wraps_its_text_in_escape_sequences() {
    colored::control::set_override(true);

    assert!(Style::new(Color::Red).colorize("sample").contains('\u{1b}'));
}

#[test]
fn the_default_color_adds_no_escape_sequences() {
    colored::control::set_override(true);

    assert_eq!(Style::new(Color::Default).colorize("sample"), "sample");
}

#[test]
fn a_bold_style_adds_the_bold_escape_sequence() {
    colored::control::set_override(true);

    assert!(Style::bold(Color::Red)
        .colorize("sample")
        .contains("\u{1b}[1;31m"));
}

#[test]
fn an_rgb_style_uses_a_true_color_escape_sequence() {
    colored::control::set_override(true);

    let colorized = Style::new(Color::Rgb(255, 128, 0)).colorize("sample");

    assert!(colorized.contains('\u{1b}'), "{:?}", colorized);
}

#[test]
fn a_style_prints_its_color_and_attributes() {
    assert_eq!(Style::new(Color::Cyan).to_string(), "cyan");
    assert_eq!(Style::bold(Color::White).to_string(), "white bold");
}

#[test]
fn every_color_keeps_the_text_it_colorizes() {
    colored::control::set_override(true);

    let colors: Vec<Color> = NAMED_COLORS
        .iter()
        .map(|(_, color)| *color)
        .chain([Color::BrightCyan, Color::Rgb(1, 2, 3)])
        .collect();

    for color in colors {
        assert!(
            Style::new(color).colorize("sample").contains("sample"),
            "{} lost its text",
            color
        );
    }
}
