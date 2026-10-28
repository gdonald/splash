use splash::output::{
    self, escape_html, escape_json, OutputMode, ParsedLine, Token, TokenKind, UnknownOutputMode,
};
use splash::theme::Theme;
use std::collections::HashSet;
use std::error::Error;

fn sample_line() -> ParsedLine<'static> {
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
    let theme = Theme::default();

    for kind in TokenKind::all() {
        let color = theme.css(kind);

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
        let colored_text = Theme::default().colorize(kind, "sample");

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

    assert_eq!(
        Theme::default().colorize(TokenKind::Plain, "sample"),
        "sample"
    );
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
    assert_eq!("csv".parse(), Ok(OutputMode::Csv));
    assert_eq!("color".parse(), Ok(OutputMode::Ansi));
    assert_eq!("curses".parse(), Ok(OutputMode::Curses));
    assert_eq!("tui".parse(), Ok(OutputMode::Curses));
    assert_eq!("html".parse(), Ok(OutputMode::Html));
    assert_eq!("json".parse(), Ok(OutputMode::Json));
    assert_eq!("jsonl".parse(), Ok(OutputMode::Jsonl));
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
        "Unknown output mode 'sparkles' (expected ansi, csv, curses, html, json, jsonl, or plain)"
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
        OutputMode::Csv,
        OutputMode::Curses,
        OutputMode::Html,
        OutputMode::Json,
        OutputMode::Jsonl,
        OutputMode::Plain,
    ]
    .iter()
    .map(|mode| mode.to_string())
    .collect();

    assert_eq!(
        names,
        vec!["ansi", "csv", "curses", "html", "json", "jsonl", "plain"]
    );
}

#[test]
fn ansi_output_colorizes_each_token() {
    colored::control::set_override(true);

    let rendered = OutputMode::Ansi.render(&sample_line(), &Theme::default());

    assert!(rendered.contains("\u{1b}[91m127.0.0.1\u{1b}[0m"));
}

#[test]
fn plain_output_strips_all_styling() {
    let rendered = OutputMode::Plain.render(&sample_line(), &Theme::default());

    assert_eq!(rendered, "127.0.0.1 \"GET\"");
}

#[test]
fn html_output_wraps_each_token_in_a_classed_span() {
    let rendered = OutputMode::Html.render(&sample_line(), &Theme::default());

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
        OutputMode::Json.render(&line, &Theme::default()),
        "{\"text\":\"404 \",\"tokens\":[{\"kind\":\"number\",\"text\":\"404\"},{\"kind\":\"plain\",\"text\":\" \"}]}"
    );
}

#[test]
fn only_html_output_has_a_header() {
    assert!(OutputMode::Html.header(&Theme::default()).is_some());
    assert!(OutputMode::Ansi.header(&Theme::default()).is_none());
    assert!(OutputMode::Curses.header(&Theme::default()).is_none());
    assert!(OutputMode::Json.header(&Theme::default()).is_none());
    assert!(OutputMode::Plain.header(&Theme::default()).is_none());
}

#[test]
fn only_html_output_has_a_footer() {
    assert_eq!(
        OutputMode::Html.footer(),
        Some("</pre>\n</main>\n</body>\n</html>\n".to_string())
    );
    assert!(OutputMode::Ansi.footer().is_none());
    assert!(OutputMode::Curses.footer().is_none());
    assert!(OutputMode::Json.footer().is_none());
    assert!(OutputMode::Plain.footer().is_none());
}

#[test]
fn the_html_header_opens_a_document_and_a_preformatted_block() {
    let header = OutputMode::Html.header(&Theme::default()).unwrap();

    assert!(header.starts_with("<!DOCTYPE html>\n"));
    assert!(header.ends_with("<pre class=\"splash\" role=\"log\" aria-label=\"Log lines\">\n"));
}

#[test]
fn the_html_log_is_a_labeled_log_region_in_the_main_landmark() {
    let header = OutputMode::Html.header(&Theme::default()).unwrap();
    let footer = OutputMode::Html.footer().unwrap();

    assert_eq!(
        (
            header.ends_with(
                "<body>\n<main>\n<pre class=\"splash\" role=\"log\" aria-label=\"Log lines\">\n"
            ),
            footer.starts_with("</pre>\n</main>\n"),
        ),
        (true, true)
    );
}

#[test]
fn html_line_numbers_are_hidden_from_screen_readers() {
    assert_eq!(
        splash::output::number_html_line(7, "text"),
        "<span class=\"splash-line-number\" aria-hidden=\"true\">7</span>text"
    );
}

#[test]
fn the_html_header_defines_a_css_rule_for_every_token_kind() {
    let header = OutputMode::Html.header(&Theme::default()).unwrap();

    for kind in TokenKind::all() {
        let theme = Theme::default();
        let weight = if theme.is_bold(kind) {
            " font-weight: bold;"
        } else {
            ""
        };
        let rule = format!(
            ".splash-{} {{ color: {};{} }}",
            kind.name(),
            theme.css(kind),
            weight
        );

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
        OutputMode::Curses.render(&line, &Theme::default()),
        OutputMode::Ansi.render(&line, &Theme::default())
    );
}

#[test]
fn a_slice_of_a_borrowed_token_borrows_the_same_text() {
    let line = String::from("connect from 10.0.0.5");
    let token = Token::new(&line, TokenKind::Message);

    assert!(matches!(
        token.slice(8, 12).text,
        std::borrow::Cow::Borrowed("from")
    ));
}

#[test]
fn a_slice_of_an_owned_token_keeps_its_style() {
    let token = Token::owned("2023-10-03T12:00:01Z".to_string(), TokenKind::Timestamp);

    assert_eq!(
        token.slice(0, 10),
        Token::new("2023-10-03", TokenKind::Timestamp)
    );
}

fn render_jsonl(tokens: Vec<Token<'static>>) -> String {
    OutputMode::Jsonl.render(&ParsedLine::new(tokens), &Theme::default())
}

#[test]
fn jsonl_output_keys_each_field_by_its_kind() {
    assert_eq!(
        render_jsonl(sample_line().tokens),
        r#"{"text":"127.0.0.1 \"GET\"","ip":"127.0.0.1","http_verb":"GET"}"#
    );
}

#[test]
fn jsonl_output_gathers_a_repeated_kind_into_an_array() {
    let output = render_jsonl(vec![
        Token::new("2023-10-03T12:00:01Z", TokenKind::Timestamp),
        Token::new(" ", TokenKind::Plain),
        Token::new("[03/Oct/2023:12:00:01 +0000]", TokenKind::Timestamp),
    ]);

    assert_eq!(
        output,
        r#"{"text":"2023-10-03T12:00:01Z [03/Oct/2023:12:00:01 +0000]","timestamp":["2023-10-03T12:00:01Z","[03/Oct/2023:12:00:01 +0000]"]}"#
    );
}

#[test]
fn jsonl_output_leaves_out_pair_keys_and_whitespace() {
    let output = render_jsonl(vec![
        Token::new("level", TokenKind::Header),
        Token::new("=", TokenKind::Punctuation),
        Token::new("info", TokenKind::Level),
        Token::new(" ", TokenKind::Message),
    ]);

    assert_eq!(
        output,
        r#"{"text":"level=info ","level":"info","record":{"level":"info"}}"#
    );
}

#[test]
fn jsonl_output_escapes_field_values() {
    let output = render_jsonl(vec![Token::new("say \"hi\"\t", TokenKind::Message)]);

    assert_eq!(
        output,
        r#"{"text":"say \"hi\"\t","message":"say \"hi\"\t"}"#
    );
}

#[test]
fn jsonl_output_has_no_header_or_footer() {
    assert_eq!(OutputMode::Jsonl.header(&Theme::default()), None);
    assert_eq!(OutputMode::Jsonl.footer(), None);
}

fn access_line() -> ParsedLine<'static> {
    ParsedLine::new(vec![
        Token::new("10.0.0.5", TokenKind::Client),
        Token::new(" ", TokenKind::Plain),
        Token::new("GET", TokenKind::Method),
        Token::new(" ", TokenKind::Plain),
        Token::new("/a,b", TokenKind::Request),
        Token::new(" ", TokenKind::Plain),
        Token::new("404", TokenKind::Status),
    ])
}

fn names(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|field| field.to_string()).collect()
}

#[test]
fn jsonl_output_writes_only_the_named_fields_in_the_order_named() {
    let output = OutputMode::Jsonl.render_with_fields(
        &access_line(),
        &Theme::default(),
        &names(&["status", "text", "userid", "client"]),
    );

    assert_eq!(
        output,
        r#"{"status":"404","text":"10.0.0.5 GET /a,b 404","client":"10.0.0.5"}"#
    );
}

#[test]
fn csv_output_writes_the_line_text_when_no_fields_are_named() {
    assert_eq!(
        OutputMode::Csv.render(&access_line(), &Theme::default()),
        r#""10.0.0.5 GET /a,b 404""#
    );
    assert_eq!(
        OutputMode::Csv.header(&Theme::default()),
        Some("text\n".to_string())
    );
}

#[test]
fn csv_output_writes_a_column_for_each_named_field() {
    let fields = names(&["client", "status", "request", "userid"]);

    assert_eq!(
        OutputMode::Csv.header_with_fields(&Theme::default(), &fields),
        Some("client,status,request,userid\n".to_string())
    );
    assert_eq!(
        OutputMode::Csv.render_with_fields(&access_line(), &Theme::default(), &fields),
        r#"10.0.0.5,404,"/a,b","#
    );
}

#[test]
fn csv_output_joins_a_repeated_field_with_spaces() {
    let line = ParsedLine::new(vec![
        Token::new("listening on", TokenKind::Message),
        Token::new(" ", TokenKind::Plain),
        Token::new("10.0.0.5", TokenKind::Ip),
        Token::new(" ", TokenKind::Plain),
        Token::new("port 80", TokenKind::Message),
    ]);

    assert_eq!(
        OutputMode::Csv.render_with_fields(&line, &Theme::default(), &names(&["message"])),
        "listening on port 80"
    );
}

#[test]
fn csv_output_doubles_quotes_and_quotes_line_breaks() {
    let line = ParsedLine::new(vec![Token::new("say \"hi\"\nnow", TokenKind::Message)]);

    assert_eq!(
        OutputMode::Csv.render(&line, &Theme::default()),
        "\"say \"\"hi\"\"\nnow\""
    );
}

#[test]
fn field_names_are_text_and_the_token_kinds() {
    assert!(output::is_field_name("text"));
    assert!(output::is_field_name("status"));
    assert!(!output::is_field_name("colour"));
}

#[test]
fn a_json_line_keeps_its_structure_as_the_record() {
    assert_eq!(
        output::record(r#"  {"level":"info","user":{"id":7,"admin":false},"tags":["a"]} "#),
        Some(r#"{"level":"info","user":{"id":7,"admin":false},"tags":["a"]}"#.to_string())
    );
}

#[test]
fn key_value_pairs_become_a_record_of_strings() {
    assert_eq!(
        output::record(r#"time="2023-10-03T12:00:01Z" level=info msg="say \"hi\"""#),
        Some(
            r#"{"time":"2023-10-03T12:00:01Z","level":"info","msg":"say \\\"hi\\\""}"#.to_string()
        )
    );
}

#[test]
fn a_line_in_neither_form_has_no_record() {
    assert_eq!(output::record("GET /search?q=1 HTTP/1.1"), None);
    assert_eq!(output::record("{not json"), None);
}

#[test]
fn jsonl_output_holds_the_record_of_a_structured_line() {
    let line = ParsedLine::new(vec![Token::new(r#"{"level":"info"}"#, TokenKind::Message)]);

    assert_eq!(
        OutputMode::Jsonl.render(&line, &Theme::default()),
        r#"{"text":"{\"level\":\"info\"}","message":"{\"level\":\"info\"}","record":{"level":"info"}}"#
    );
}

#[test]
fn the_record_is_a_field_csv_can_name() {
    let line = ParsedLine::new(vec![Token::new("level=info", TokenKind::Message)]);
    let fields = names(&["record", "text"]);

    assert!(output::is_field_name("record"));
    assert_eq!(
        OutputMode::Csv.render_with_fields(&line, &Theme::default(), &fields),
        r#""{""level"":""info""}",level=info"#
    );
}

#[test]
fn a_line_without_a_record_has_an_empty_record_cell() {
    assert_eq!(
        OutputMode::Csv.render_with_fields(&access_line(), &Theme::default(), &names(&["record"])),
        ""
    );
}

fn labeled_line() -> ParsedLine<'static> {
    ParsedLine::new(vec![
        Token::new("api.log", TokenKind::Source),
        Token::new(" ", TokenKind::Plain),
        Token::new("ready", TokenKind::Message),
    ])
}

#[test]
fn a_merged_lines_label_is_not_part_of_its_text() {
    assert_eq!(
        OutputMode::Jsonl.render(&labeled_line(), &Theme::default()),
        r#"{"text":"ready","source":"api.log","message":"ready"}"#
    );
    assert!(OutputMode::Json
        .render(&labeled_line(), &Theme::default())
        .starts_with(r#"{"text":"ready","tokens":[{"kind":"source","text":"api.log"}"#));
    assert_eq!(
        OutputMode::Csv.render_with_fields(
            &labeled_line(),
            &Theme::default(),
            &names(&["source", "text"])
        ),
        "api.log,ready"
    );
}

#[test]
fn plain_output_writes_a_merged_lines_label() {
    assert_eq!(
        OutputMode::Plain.render(&labeled_line(), &Theme::default()),
        "api.log ready"
    );
}

#[test]
fn a_style_sheet_is_added_to_the_html_header_after_the_theme_rules() {
    let header = OutputMode::Html
        .header_with_css(
            &Theme::default(),
            &[],
            Some("pre.splash { font-size: 14px; }"),
        )
        .unwrap();

    assert!(header.ends_with(
        "    .splash-source { color: #0000aa; font-weight: bold; }\npre.splash { font-size: 14px; }\n</style>\n</head>\n<body>\n<main>\n<pre class=\"splash\" role=\"log\" aria-label=\"Log lines\">\n"
    ), "{}", header);
}

#[test]
fn a_style_sheet_that_ends_with_a_line_break_is_added_as_written() {
    let header = OutputMode::Html
        .header_with_css(&Theme::default(), &[], Some("a { color: red; }\n"))
        .unwrap();

    assert!(
        header.contains("\na { color: red; }\n</style>\n"),
        "{}",
        header
    );
}

#[test]
fn a_style_sheet_is_not_added_to_output_without_a_header() {
    assert_eq!(
        OutputMode::Plain.header_with_css(&Theme::default(), &[], Some("a { color: red; }")),
        None
    );
}

#[test]
fn the_html_header_styles_line_numbers_so_copying_leaves_them_out() {
    let header = OutputMode::Html.header(&Theme::default()).unwrap();

    assert!(header.contains(
        "    .splash-line-number { display: inline-block; min-width: 6ch; margin-right: 1ch; text-align: right; opacity: 0.6; user-select: none; }\n"
    ));
}

#[test]
fn a_numbered_html_line_starts_with_its_number() {
    assert_eq!(
        output::number_html_line(42, "<span class=\"splash-plain\">a</span>"),
        "<span class=\"splash-line-number\" aria-hidden=\"true\">42</span><span class=\"splash-plain\">a</span>"
    );
}

#[test]
fn control_characters_are_written_in_caret_notation() {
    assert_eq!(
        output::visible_controls("a\u{1b}[2J\u{7}\r\u{0}\u{7f}b"),
        "a^[[2J^G^M^@^?b"
    );
}

#[test]
fn a_c1_control_is_written_as_its_hex_code() {
    assert_eq!(output::visible_controls("a\u{9b}31m"), "a\\x9b31m");
}

#[test]
fn text_without_control_characters_is_borrowed_as_it_is() {
    assert!(matches!(
        output::visible_controls("tab\tand é"),
        std::borrow::Cow::Borrowed("tab\tand é")
    ));
}

#[test]
fn ansi_output_shows_an_escape_sequence_in_a_line_instead_of_sending_it() {
    let parsed = ParsedLine::new(vec![Token::new(
        "title \u{1b}]0;owned\u{7}",
        TokenKind::Message,
    )]);

    let rendered = OutputMode::Ansi.render(&parsed, &Theme::default());

    assert!(rendered.contains("title ^[]0;owned^G"), "{:?}", rendered);
    assert!(!rendered.contains("\u{7}"));
}
