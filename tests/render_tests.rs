use splash::config::{Config, Options, Profiles, Settings};
use splash::output::OutputMode;
use splash::render_file;
use splash::theme::Theme;
use splash::{
    default_jobs, render_contents, render_contents_with_jobs, render_lines, render_lines_with_jobs,
    PARALLEL_THRESHOLD,
};

const CLF_LINES: &str = concat!(
    "127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] \"GET /apache_pb.gif HTTP/1.0\" 200 2326\n",
    "not a log line\n",
    "10.0.0.5 - - [12/Dec/2001:01:02:03 +0000] \"POST /submit HTTP/1.1\" 302 -\n",
);

#[test]
fn rendering_emits_one_line_per_parsed_line() {
    let rendered = render_contents(
        "alpha\nbeta\n",
        "ad-hoc",
        OutputMode::Plain,
        &Theme::default(),
    );

    assert_eq!(rendered, "alpha\nbeta\n");
}

#[test]
fn rendering_drops_blank_lines() {
    let rendered = render_contents(
        "alpha\n\n\nbeta\n",
        "ad-hoc",
        OutputMode::Plain,
        &Theme::default(),
    );

    assert_eq!(rendered, "alpha\nbeta\n");
}

#[test]
fn rendering_clf_drops_lines_that_do_not_match() {
    let rendered = render_contents(CLF_LINES, "clf", OutputMode::Plain, &Theme::default());

    assert_eq!(rendered.lines().count(), 2);
    assert!(!rendered.contains("not a log line"));
}

#[test]
fn rendering_empty_input_produces_no_output() {
    assert_eq!(
        render_contents("", "ad-hoc", OutputMode::Plain, &Theme::default()),
        ""
    );
}

#[test]
fn rendering_as_json_produces_one_object_per_line() {
    let rendered = render_contents("404 ok\n", "ad-hoc", OutputMode::Json, &Theme::default());

    assert_eq!(
        rendered,
        "{\"text\":\"404 ok\",\"tokens\":[{\"kind\":\"number\",\"text\":\"404\"},{\"kind\":\"plain\",\"text\":\" \"},{\"kind\":\"plain\",\"text\":\"ok\"}]}\n"
    );
}

#[test]
fn rendering_as_html_escapes_the_log_text() {
    let rendered = render_contents("<script>\n", "ad-hoc", OutputMode::Html, &Theme::default());

    assert_eq!(
        rendered,
        "<span class=\"splash-plain\">&lt;script&gt;</span>\n"
    );
}

#[test]
fn rendering_as_ansi_colorizes_the_log_text() {
    colored::control::set_override(true);

    let rendered = render_contents(
        "192.168.1.1\n",
        "ad-hoc",
        OutputMode::Ansi,
        &Theme::default(),
    );

    assert_eq!(rendered, "\u{1b}[91m192.168.1.1\u{1b}[0m\n");
}

#[test]
fn rendering_to_lines_returns_one_string_per_parsed_line() {
    let lines = render_lines(
        "alpha\n\nbeta\n",
        "ad-hoc",
        OutputMode::Plain,
        &Theme::default(),
    );

    assert_eq!(lines, vec!["alpha".to_string(), "beta".to_string()]);
}

#[test]
fn rendering_to_lines_drops_lines_the_mode_cannot_parse() {
    let lines = render_lines(CLF_LINES, "clf", OutputMode::Plain, &Theme::default());

    assert_eq!(lines.len(), 2);
}

fn many_lines(count: usize) -> String {
    (0..count)
        .map(|line| format!("line {} from 10.0.0.{}\n", line, line % 256))
        .collect()
}

#[test]
fn the_default_worker_count_is_at_least_one() {
    assert!(default_jobs() >= 1);
}

#[test]
fn rendering_across_workers_gives_the_same_lines_as_one_worker() {
    let contents = many_lines(PARALLEL_THRESHOLD * 3 + 7);

    assert_eq!(
        render_lines_with_jobs(&contents, "ad-hoc", OutputMode::Plain, &Theme::default(), 4),
        render_lines(&contents, "ad-hoc", OutputMode::Plain, &Theme::default())
    );
}

#[test]
fn rendering_across_workers_keeps_the_lines_in_input_order() {
    let contents = many_lines(PARALLEL_THRESHOLD * 2);

    let lines =
        render_lines_with_jobs(&contents, "ad-hoc", OutputMode::Plain, &Theme::default(), 3);

    assert_eq!(lines.first().unwrap(), "line 0 from 10.0.0.0");
    assert_eq!(
        lines.last().unwrap(),
        &format!(
            "line {} from 10.0.0.{}",
            contents.lines().count() - 1,
            (contents.lines().count() - 1) % 256
        )
    );
}

#[test]
fn an_input_shorter_than_the_threshold_is_rendered_on_the_calling_thread() {
    let contents = many_lines(PARALLEL_THRESHOLD - 1);

    assert_eq!(
        render_lines_with_jobs(&contents, "ad-hoc", OutputMode::Plain, &Theme::default(), 8),
        render_lines(&contents, "ad-hoc", OutputMode::Plain, &Theme::default())
    );
}

#[test]
fn a_single_worker_renders_a_long_input_on_the_calling_thread() {
    let contents = many_lines(PARALLEL_THRESHOLD + 1);

    assert_eq!(
        render_lines_with_jobs(&contents, "ad-hoc", OutputMode::Plain, &Theme::default(), 1),
        render_lines(&contents, "ad-hoc", OutputMode::Plain, &Theme::default())
    );
}

#[test]
fn rendering_contents_across_workers_ends_every_line() {
    let contents = many_lines(PARALLEL_THRESHOLD + 4);

    assert_eq!(
        render_contents_with_jobs(&contents, "ad-hoc", OutputMode::Plain, &Theme::default(), 4),
        render_contents(&contents, "ad-hoc", OutputMode::Plain, &Theme::default())
    );
}

#[test]
fn unparsed_lines_are_dropped_by_every_worker() {
    let contents = "not clf\n".repeat(PARALLEL_THRESHOLD + 1);

    assert!(
        render_lines_with_jobs(&contents, "clf", OutputMode::Plain, &Theme::default(), 4)
            .is_empty()
    );
}

fn settings(mode: &str, jobs: usize) -> Settings {
    let options = Options {
        mode: Some(mode.to_string()),
        output: Some("plain".to_string()),
        jobs: Some(jobs),
        ..Options::default()
    };

    Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(std::path::PathBuf::from("unused")),
    )
    .unwrap()
}

#[test]
fn a_log_file_is_rendered_from_disk() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("small.log");
    std::fs::write(&path, "alpha\nbeta\n").unwrap();

    assert_eq!(
        render_file(&path, &settings("ad-hoc", 1)).unwrap(),
        "alpha\nbeta\n"
    );
}

#[test]
fn a_large_log_file_is_rendered_across_workers() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("large.log");
    let sample = splash::bench::sample_log(2_000);
    std::fs::write(&path, &sample).unwrap();

    let rendered = render_file(&path, &settings("clf", 4)).unwrap();

    assert_eq!(rendered.lines().count(), 2_000);
    assert_eq!(rendered, sample);
}

#[test]
fn a_missing_log_file_is_reported() {
    let directory = tempfile::TempDir::new().unwrap();

    assert!(render_file(
        &directory.path().join("missing.log"),
        &settings("ad-hoc", 1)
    )
    .is_err());
}
