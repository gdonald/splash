use splash::color::{Color, Style, UnknownColor};
use splash::output::TokenKind;
use splash::theme::{Theme, ThemeError, OKABE_ITO, PRESETS};
use std::error::Error;

#[test]
fn the_default_theme_is_the_dark_theme() {
    assert_eq!(Theme::default(), Theme::dark());
}

#[test]
fn every_preset_loads_by_name() {
    for name in PRESETS {
        let theme = Theme::preset(name).unwrap();

        assert_eq!(theme.name(), name);
    }
}

#[test]
fn the_default_preset_name_loads_the_dark_theme() {
    assert_eq!(Theme::preset("default"), Ok(Theme::dark()));
}

#[test]
fn preset_names_ignore_case() {
    assert_eq!(Theme::preset("DRACULA"), Ok(Theme::dracula()));
}

#[test]
fn an_unknown_preset_lists_the_presets_it_knows() {
    let error = Theme::preset("neon").unwrap_err();

    assert_eq!(
        error.to_string(),
        "Unknown theme 'neon' (expected one of dark, light, solarized, dracula, colorblind, high-contrast)"
    );
}

#[test]
fn every_preset_styles_every_token_kind() {
    for name in PRESETS {
        let theme = Theme::preset(name).unwrap();

        for kind in TokenKind::all() {
            assert!(
                theme.css(kind).starts_with('#'),
                "{} has no color for {}",
                name,
                kind.name()
            );
        }
    }
}

#[test]
fn every_preset_renders_the_user_id_in_bold() {
    for name in PRESETS {
        assert!(
            Theme::preset(name).unwrap().is_bold(TokenKind::UserId),
            "{} does not embolden the user id",
            name
        );
    }
}

#[test]
fn a_theme_reports_the_style_it_paints_a_token_kind_in() {
    assert_eq!(
        Theme::dark().style(TokenKind::Status),
        Style::new(Color::BrightYellow)
    );
}

#[test]
fn a_style_can_be_replaced() {
    let mut theme = Theme::dark();

    theme.set_style(TokenKind::Status, Style::new(Color::Green));

    assert_eq!(theme.style(TokenKind::Status), Style::new(Color::Green));
}

#[test]
fn a_theme_supplies_the_page_colors_for_html_output() {
    let theme = Theme::dracula();

    assert_eq!(theme.background().css(), "#282a36");
    assert_eq!(theme.foreground().css(), "#f8f8f2");
}

#[test]
fn the_light_theme_puts_dark_text_on_a_light_page() {
    let theme = Theme::light();

    assert_eq!(theme.background().css(), "#ffffff");
    assert_eq!(theme.foreground().css(), "#000000");
}

#[test]
fn the_solarized_theme_uses_the_solarized_background() {
    assert_eq!(Theme::solarized().background().css(), "#002b36");
}

#[test]
fn a_theme_colorizes_a_token_with_its_style() {
    colored::control::set_override(true);

    assert_eq!(
        Theme::dark().colorize(TokenKind::Status, "200"),
        Style::new(Color::BrightYellow).colorize("200")
    );
}

#[test]
fn an_override_repaints_one_token_kind() {
    let mut theme = Theme::dark();

    theme.apply_override("ip", "bright cyan").unwrap();

    assert_eq!(theme.style(TokenKind::Ip), Style::new(Color::BrightCyan));
}

#[test]
fn an_override_naming_an_unknown_token_kind_is_rejected() {
    let mut theme = Theme::dark();

    assert_eq!(
        theme.apply_override("hostname", "cyan"),
        Err(ThemeError::UnknownTokenKind("hostname".to_string()))
    );
}

#[test]
fn an_unknown_color_key_says_which_key_it_does_not_know() {
    let error = ThemeError::UnknownTokenKind("hostname".to_string());

    assert_eq!(error.to_string(), "Unknown color key 'hostname'");
}

#[test]
fn an_override_naming_an_unknown_color_is_rejected() {
    let mut theme = Theme::dark();

    assert_eq!(
        theme.apply_override("ip", "mauve"),
        Err(ThemeError::UnknownColor(UnknownColor("mauve".to_string())))
    );
}

#[test]
fn an_unknown_color_reaches_the_theme_error_unchanged() {
    let error = ThemeError::from(UnknownColor("mauve".to_string()));

    assert_eq!(
        error.to_string(),
        UnknownColor("mauve".to_string()).to_string()
    );
}

#[test]
fn a_theme_error_is_an_error() {
    let error: Box<dyn Error> = Box::new(ThemeError::UnknownTheme("neon".to_string()));

    assert!(error.source().is_none());
}

#[test]
fn overrides_are_applied_in_the_order_given() {
    let mut theme = Theme::dark();

    theme
        .apply_overrides([("ip", "green"), ("ip", "red"), ("status", "blue")])
        .unwrap();

    assert_eq!(theme.style(TokenKind::Ip), Style::new(Color::Red));
    assert_eq!(theme.style(TokenKind::Status), Style::new(Color::Blue));
}

#[test]
fn a_failing_override_stops_the_rest() {
    let mut theme = Theme::dark();

    let result = theme.apply_overrides([("ip", "green"), ("status", "mauve")]);

    assert!(result.is_err());
    assert_eq!(
        theme.style(TokenKind::Status),
        Theme::dark().style(TokenKind::Status)
    );
}

#[test]
fn a_theme_writes_itself_as_a_config_file() {
    let toml = Theme::dark().to_toml();

    assert!(toml.starts_with("theme = \"dark\"\n\n[colors]\n"));
    assert!(toml.contains("status = \"bright yellow\"\n"));
    assert!(toml.contains("userid = \"white bold\"\n"));
}

#[test]
fn a_saved_theme_lists_every_token_kind() {
    let toml = Theme::solarized().to_toml();

    for kind in TokenKind::all() {
        assert!(
            toml.contains(&format!("{} = ", kind.name())),
            "missing {}",
            kind.name()
        );
    }
}

/// The WCAG relative luminance of a color
fn luminance(color: Color) -> f64 {
    let Color::Rgb(red, green, blue) = color else {
        panic!("{:?} has no fixed value", color);
    };
    let linear = |channel: u8| {
        let value = channel as f64 / 255.0;

        match value <= 0.04045 {
            true => value / 12.92,
            false => ((value + 0.055) / 1.055).powf(2.4),
        }
    };

    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

/// The WCAG contrast ratio between two colors
fn contrast(one: Color, other: Color) -> f64 {
    let (lighter, darker) = match luminance(one) >= luminance(other) {
        true => (luminance(one), luminance(other)),
        false => (luminance(other), luminance(one)),
    };

    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn the_high_contrast_theme_paints_every_token_at_least_seven_to_one_against_its_background() {
    let theme = Theme::high_contrast();

    for kind in TokenKind::all() {
        let ratio = contrast(theme.style(kind).color, theme.background());

        assert!(ratio >= 7.0, "{} is {:.2}:1", kind.name(), ratio);
    }

    assert!(contrast(theme.foreground(), theme.background()) >= 7.0);
}

#[test]
fn the_colorblind_theme_paints_with_the_okabe_ito_palette_and_neutrals() {
    let theme = Theme::colorblind();
    let neutrals = [
        Color::Default,
        Color::BrightWhite,
        Color::Rgb(0x99, 0x99, 0x99),
    ];

    for kind in TokenKind::all() {
        let color = theme.style(kind).color;

        assert!(
            OKABE_ITO.contains(&color) || neutrals.contains(&color),
            "{} is {:?}",
            kind.name(),
            color
        );
    }
}

#[test]
fn the_colorblind_theme_tells_success_warning_and_failure_apart_by_color() {
    let theme = Theme::colorblind();
    let colors = [
        theme.style(TokenKind::Success).color,
        theme.style(TokenKind::Warning).color,
        theme.style(TokenKind::Failure).color,
    ];

    assert_eq!(
        colors,
        [
            Color::Rgb(0x56, 0xb4, 0xe9),
            Color::Rgb(0xf0, 0xe4, 0x42),
            Color::Rgb(0xd5, 0x5e, 0x00)
        ]
    );
}
