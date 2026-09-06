use splash::output::OutputMode;
use splash::{render_contents, render_lines};

const CLF_LINES: &str = concat!(
    "127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] \"GET /apache_pb.gif HTTP/1.0\" 200 2326\n",
    "not a log line\n",
    "10.0.0.5 - - [12/Dec/2001:01:02:03 +0000] \"POST /submit HTTP/1.1\" 302 -\n",
);

#[test]
fn rendering_emits_one_line_per_parsed_line() {
    let rendered = render_contents("alpha\nbeta\n", "ad-hoc", OutputMode::Plain);

    assert_eq!(rendered, "alpha\nbeta\n");
}

#[test]
fn rendering_drops_blank_lines() {
    let rendered = render_contents("alpha\n\n\nbeta\n", "ad-hoc", OutputMode::Plain);

    assert_eq!(rendered, "alpha\nbeta\n");
}

#[test]
fn rendering_clf_drops_lines_that_do_not_match() {
    let rendered = render_contents(CLF_LINES, "clf", OutputMode::Plain);

    assert_eq!(rendered.lines().count(), 2);
    assert!(!rendered.contains("not a log line"));
}

#[test]
fn rendering_empty_input_produces_no_output() {
    assert_eq!(render_contents("", "ad-hoc", OutputMode::Plain), "");
}

#[test]
fn rendering_as_json_produces_one_object_per_line() {
    let rendered = render_contents("404 ok\n", "ad-hoc", OutputMode::Json);

    assert_eq!(
        rendered,
        "{\"text\":\"404 ok\",\"tokens\":[{\"kind\":\"number\",\"text\":\"404\"},{\"kind\":\"plain\",\"text\":\" \"},{\"kind\":\"plain\",\"text\":\"ok\"}]}\n"
    );
}

#[test]
fn rendering_as_html_escapes_the_log_text() {
    let rendered = render_contents("<script>\n", "ad-hoc", OutputMode::Html);

    assert_eq!(
        rendered,
        "<span class=\"splash-plain\">&lt;script&gt;</span>\n"
    );
}

#[test]
fn rendering_as_ansi_colorizes_the_log_text() {
    colored::control::set_override(true);

    let rendered = render_contents("192.168.1.1\n", "ad-hoc", OutputMode::Ansi);

    assert_eq!(rendered, "\u{1b}[91m192.168.1.1\u{1b}[0m\n");
}

#[test]
fn rendering_to_lines_returns_one_string_per_parsed_line() {
    let lines = render_lines("alpha\n\nbeta\n", "ad-hoc", OutputMode::Plain);

    assert_eq!(lines, vec!["alpha".to_string(), "beta".to_string()]);
}

#[test]
fn rendering_to_lines_drops_lines_the_mode_cannot_parse() {
    let lines = render_lines(CLF_LINES, "clf", OutputMode::Plain);

    assert_eq!(lines.len(), 2);
}
