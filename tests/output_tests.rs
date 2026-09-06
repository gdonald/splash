use splash::output::{
    escape_html, escape_json, OutputMode, ParsedLine, Token, TokenKind, UnknownOutputMode,
};
use std::collections::HashSet;
use std::error::Error;

fn sample_line() -> ParsedLine {
    ParsedLine::new(vec![
        Token::new("127.0.0.1", TokenKind::Ip),
        Token::new(" ", TokenKind::Plain),
        Token::new("\"", TokenKind::Punctuation),
        Token::new("GET", TokenKind::HttpVerb),
        Token::new("\"", TokenKind::Punctuation),
    ])
}

#[test]
fn every_token_kind_has_a_unique_name() {
    let names: HashSet<&str> = TokenKind::all().iter().map(|kind| kind.name()).collect();

    assert_eq!(names.len(), TokenKind::all().len());
}

#[test]
fn every_token_kind_has_a_six_digit_css_color() {
    for kind in TokenKind::all() {
        let color = kind.css_color();

        assert!(
            color.starts_with('#') && color.len() == 7,
            "{} has css color {}",
            kind.name(),
            color
        );
    }
}

#[test]
fn every_styled_token_kind_wraps_its_text_in_ansi_escapes() {
    colored::control::set_override(true);

    for kind in TokenKind::all() {
        let colored_text = kind.colorize("sample");

        assert!(
            colored_text.contains("sample"),
            "{} lost its text",
            kind.name()
        );

        if kind != TokenKind::Plain {
            assert!(
                colored_text.contains('\u{1b}'),
                "{} produced no escape sequence",
                kind.name()
            );
        }
    }
}

#[test]
fn the_plain_token_kind_adds_no_escape_sequences() {
    colored::control::set_override(true);

    assert_eq!(TokenKind::Plain.colorize("sample"), "sample");
}

#[test]
fn a_parsed_line_reports_its_undecorated_text() {
    assert_eq!(sample_line().text(), "127.0.0.1 \"GET\"");
}

#[test]
fn ansi_is_the_default_output_mode() {
    assert_eq!(OutputMode::default(), OutputMode::Ansi);
}

#[test]
fn output_modes_parse_from_their_names() {
    assert_eq!("ansi".parse(), Ok(OutputMode::Ansi));
    assert_eq!("color".parse(), Ok(OutputMode::Ansi));
    assert_eq!("curses".parse(), Ok(OutputMode::Curses));
    assert_eq!("tui".parse(), Ok(OutputMode::Curses));
    assert_eq!("html".parse(), Ok(OutputMode::Html));
    assert_eq!("json".parse(), Ok(OutputMode::Json));
    assert_eq!("plain".parse(), Ok(OutputMode::Plain));
    assert_eq!("none".parse(), Ok(OutputMode::Plain));
}

#[test]
fn output_mode_parsing_ignores_case() {
    assert_eq!("HTML".parse(), Ok(OutputMode::Html));
}

#[test]
fn an_unknown_output_mode_is_rejected() {
    let result: Result<OutputMode, UnknownOutputMode> = "sparkles".parse();

    assert_eq!(result, Err(UnknownOutputMode("sparkles".to_string())));
}

#[test]
fn an_unknown_output_mode_names_the_supported_modes() {
    let error = UnknownOutputMode("sparkles".to_string());

    assert_eq!(
        error.to_string(),
        "Unknown output mode 'sparkles' (expected ansi, curses, html, json, or plain)"
    );
}

#[test]
fn an_unknown_output_mode_is_a_standard_error() {
    let error = UnknownOutputMode("sparkles".to_string());
    let as_error: &dyn Error = &error;

    assert!(as_error.source().is_none());
}

#[test]
fn output_modes_display_their_names() {
    let names: Vec<String> = [
        OutputMode::Ansi,
        OutputMode::Curses,
        OutputMode::Html,
        OutputMode::Json,
        OutputMode::Plain,
    ]
    .iter()
    .map(|mode| mode.to_string())
    .collect();

    assert_eq!(names, vec!["ansi", "curses", "html", "json", "plain"]);
}

#[test]
fn ansi_output_colorizes_each_token() {
    colored::control::set_override(true);

    let rendered = OutputMode::Ansi.render(&sample_line());

    assert!(rendered.contains("\u{1b}[91m127.0.0.1\u{1b}[0m"));
}

#[test]
fn plain_output_strips_all_styling() {
    let rendered = OutputMode::Plain.render(&sample_line());

    assert_eq!(rendered, "127.0.0.1 \"GET\"");
}

#[test]
fn html_output_wraps_each_token_in_a_classed_span() {
    let rendered = OutputMode::Html.render(&sample_line());

    assert_eq!(
        rendered,
        "<span class=\"splash-ip\">127.0.0.1</span>\
         <span class=\"splash-plain\"> </span>\
         <span class=\"splash-punctuation\">&quot;</span>\
         <span class=\"splash-http_verb\">GET</span>\
         <span class=\"splash-punctuation\">&quot;</span>"
    );
}

#[test]
fn json_output_lists_the_line_text_and_its_tokens() {
    let line = ParsedLine::new(vec![
        Token::new("404", TokenKind::Number),
        Token::new(" ", TokenKind::Plain),
    ]);

    assert_eq!(
        OutputMode::Json.render(&line),
        "{\"text\":\"404 \",\"tokens\":[{\"kind\":\"number\",\"text\":\"404\"},{\"kind\":\"plain\",\"text\":\" \"}]}"
    );
}

#[test]
fn only_html_output_has_a_header() {
    assert!(OutputMode::Html.header().is_some());
    assert!(OutputMode::Ansi.header().is_none());
    assert!(OutputMode::Curses.header().is_none());
    assert!(OutputMode::Json.header().is_none());
    assert!(OutputMode::Plain.header().is_none());
}

#[test]
fn only_html_output_has_a_footer() {
    assert_eq!(
        OutputMode::Html.footer(),
        Some("</pre>\n</body>\n</html>\n".to_string())
    );
    assert!(OutputMode::Ansi.footer().is_none());
    assert!(OutputMode::Curses.footer().is_none());
    assert!(OutputMode::Json.footer().is_none());
    assert!(OutputMode::Plain.footer().is_none());
}

#[test]
fn the_html_header_opens_a_document_and_a_preformatted_block() {
    let header = OutputMode::Html.header().unwrap();

    assert!(header.starts_with("<!DOCTYPE html>\n"));
    assert!(header.ends_with("<pre class=\"splash\">\n"));
}

#[test]
fn the_html_header_defines_a_css_rule_for_every_token_kind() {
    let header = OutputMode::Html.header().unwrap();

    for kind in TokenKind::all() {
        let rule = format!(".splash-{} {{ color: {}; }}", kind.name(), kind.css_color());

        assert!(header.contains(&rule), "missing rule for {}", kind.name());
    }
}

#[test]
fn html_escaping_replaces_the_markup_characters() {
    assert_eq!(
        escape_html("<a href='x' title=\"y\">&</a>"),
        "&lt;a href=&#39;x&#39; title=&quot;y&quot;&gt;&amp;&lt;/a&gt;"
    );
}

#[test]
fn json_escaping_replaces_quotes_backslashes_and_whitespace() {
    assert_eq!(
        escape_json("say \"hi\"\\\n\t\r"),
        "say \\\"hi\\\"\\\\\\n\\t\\r"
    );
}

#[test]
fn json_escaping_uses_unicode_escapes_for_other_control_characters() {
    assert_eq!(escape_json("\u{7}\u{1f}"), "\\u0007\\u001f");
}

#[test]
fn json_escaping_leaves_ordinary_text_alone() {
    assert_eq!(escape_json("plain text"), "plain text");
}

#[test]
fn only_curses_is_an_interactive_output_mode() {
    assert!(OutputMode::Curses.is_interactive());
    assert!(!OutputMode::Ansi.is_interactive());
    assert!(!OutputMode::Html.is_interactive());
    assert!(!OutputMode::Json.is_interactive());
    assert!(!OutputMode::Plain.is_interactive());
}

#[test]
fn curses_output_colorizes_lines_the_same_way_ansi_does() {
    colored::control::set_override(true);

    let line = sample_line();

    assert_eq!(
        OutputMode::Curses.render(&line),
        OutputMode::Ansi.render(&line)
    );
}
