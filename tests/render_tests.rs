use splash::config::{Config, Options, Profiles, Settings};
use splash::filter::{Entry, FilterOptions};
use splash::output::OutputMode;
use splash::stats::{Stats, StatsOptions, StatsSettings};
use splash::theme::Theme;
use splash::zone::Zone;
use splash::{closing, opening, progress, render_contents_counted, render_settings_counted};
use splash::{
    default_jobs, render_contents, render_contents_settings, render_contents_with_jobs,
    render_lines, render_lines_with_jobs, render_settings, PARALLEL_THRESHOLD,
};
use splash::{render_file, resolve_file_settings};

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

const AUTO_LINES: &str = concat!(
    "Oct  3 12:00:01 mail postfix/smtpd[1234]: connect from unknown[10.0.0.5]\n",
    "Oct  3 12:00:01 mail postfix/smtpd[1234]: 4F2A1C0123: client=unknown[10.0.0.5]\n",
    "maintenance window moves to 02:00\n",
);

#[test]
fn rendering_in_auto_mode_keeps_lines_the_detected_mode_does_not_read() {
    let rendered = render_contents(AUTO_LINES, "auto", OutputMode::Plain, &Theme::default());

    assert_eq!(rendered, AUTO_LINES);
}

#[test]
fn rendering_in_auto_mode_across_workers_detects_the_format_once() {
    let contents = AUTO_LINES.repeat(PARALLEL_THRESHOLD);

    assert_eq!(
        render_contents_with_jobs(&contents, "auto", OutputMode::Json, &Theme::default(), 4),
        render_contents(
            &contents,
            "auto:postfix",
            OutputMode::Json,
            &Theme::default()
        )
    );
}

#[test]
fn a_log_file_in_auto_mode_is_rendered_with_the_mode_its_first_lines_detect() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("mail.log");
    std::fs::write(&path, AUTO_LINES).unwrap();

    assert_eq!(
        resolve_file_settings(&path, &settings("auto", 1))
            .unwrap()
            .mode,
        "auto:postfix"
    );
}

#[test]
fn a_log_file_with_a_named_mode_keeps_that_mode() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("mail.log");
    std::fs::write(&path, AUTO_LINES).unwrap();

    assert_eq!(
        resolve_file_settings(&path, &settings("syslog", 1)).unwrap(),
        settings("syslog", 1)
    );
}

#[test]
fn resolving_settings_for_a_missing_log_file_is_reported() {
    let directory = tempfile::TempDir::new().unwrap();

    assert!(
        resolve_file_settings(&directory.path().join("missing.log"), &settings("auto", 1)).is_err()
    );
}

const ENTRIES: &str = concat!(
    "[2023-10-03T12:00:01,123][INFO ][o.e.n.Node] [node-1] starting ...\n",
    "[2023-10-03T12:00:04,456][ERROR][o.e.b.Handler] [node-1] uncaught exception in thread [main]\n",
    "java.lang.IllegalStateException: failed to obtain node locks\n",
    "[2023-10-03T12:00:05,567][INFO ][o.e.n.Node] [node-1] stopping ...\n",
);

fn filtered(jobs: usize, filter: FilterOptions) -> Settings {
    let options = Options {
        mode: Some("auto".to_string()),
        output: Some("plain".to_string()),
        jobs: Some(jobs),
        filter,
        ..Options::default()
    };

    Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(std::path::PathBuf::from("unused")),
    )
    .unwrap()
}

fn error_level() -> FilterOptions {
    FilterOptions {
        levels: vec!["error".to_string()],
        ..FilterOptions::default()
    }
}

#[test]
fn rendering_with_a_level_filter_keeps_the_lines_belonging_to_a_kept_entry() {
    let rendered = render_settings(ENTRIES, &filtered(1, error_level()), &mut Entry::default());

    assert_eq!(
        rendered,
        vec![
            "[2023-10-03T12:00:04,456][ERROR][o.e.b.Handler] [node-1] uncaught exception in thread [main]",
            "java.lang.IllegalStateException: failed to obtain node locks",
        ]
    );
}

#[test]
fn rendering_with_filters_across_workers_matches_rendering_on_one_thread() {
    let contents = ENTRIES.repeat(PARALLEL_THRESHOLD);

    assert_eq!(
        render_settings(
            &contents,
            &filtered(4, error_level()),
            &mut Entry::default()
        ),
        render_settings(
            &contents,
            &filtered(1, error_level()),
            &mut Entry::default()
        )
    );
}

#[test]
fn the_entry_a_chunk_ends_in_carries_over_to_the_next_chunk() {
    let settings = filtered(1, error_level());
    let mut entry = Entry::default();

    render_settings(
        "[2023-10-03T12:00:04,456][ERROR][o.e.b.Handler] [node-1] uncaught exception\n",
        &settings,
        &mut entry,
    );

    assert_eq!(
        render_contents_settings("    at main.rs:42\n", &settings, &mut entry),
        "    at main.rs:42\n"
    );
}

#[test]
fn rendering_with_grep_highlights_the_match() {
    let settings = Settings {
        output_mode: OutputMode::Json,
        ..filtered(
            1,
            FilterOptions {
                grep: vec!["node locks".to_string()],
                ..FilterOptions::default()
            },
        )
    };

    let rendered = render_settings(ENTRIES, &settings, &mut Entry::default());

    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].contains(r#"{"kind":"match","text":"node"}"#));
}

#[test]
fn a_log_file_is_rendered_with_its_filters() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("app.log");
    std::fs::write(&path, ENTRIES).unwrap();

    assert_eq!(
        render_file(&path, &filtered(1, error_level()))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

/// Settings for the example error log with the statistics options given,
/// writing times in UTC
fn counted(output: &str, options: StatsOptions) -> Settings {
    let resolved = Settings::resolve(
        &Options {
            mode: Some("auto".to_string()),
            output: Some(output.to_string()),
            jobs: Some(1),
            ..Options::default()
        },
        &Config::default(),
        &Profiles::new(std::path::PathBuf::from("unused")),
    )
    .unwrap();

    Settings {
        stats: StatsSettings::build_at(&options, Zone::Utc, Zone::Utc, 0).unwrap(),
        ..resolved
    }
}

fn stats_report() -> StatsOptions {
    StatsOptions {
        stats: true,
        ..StatsOptions::default()
    }
}

fn summary_after_lines() -> StatsOptions {
    StatsOptions {
        summary: true,
        ..StatsOptions::default()
    }
}

#[test]
fn rendering_counts_the_kept_lines() {
    let mut stats = Stats::default();

    render_settings_counted(
        ENTRIES,
        &counted("plain", summary_after_lines()),
        &mut Entry::default(),
        &mut stats,
    );

    assert_eq!((stats.lines, stats.errors), (4, 1));
}

#[test]
fn rendering_without_statistics_counts_nothing() {
    let mut stats = Stats::default();

    render_settings_counted(
        ENTRIES,
        &counted("plain", StatsOptions::default()),
        &mut Entry::default(),
        &mut stats,
    );

    assert_eq!(stats, Stats::default());
}

#[test]
fn rendering_for_a_report_writes_no_lines() {
    let mut stats = Stats::default();

    assert_eq!(
        render_contents_counted(
            ENTRIES,
            &counted("plain", stats_report()),
            &mut Entry::default(),
            &mut stats,
        ),
        ""
    );
}

#[test]
fn rendering_with_a_summary_writes_the_lines() {
    let rendered = render_contents_counted(
        ENTRIES,
        &counted("plain", summary_after_lines()),
        &mut Entry::default(),
        &mut Stats::default(),
    );

    assert_eq!(rendered, ENTRIES);
}

#[test]
fn the_opening_is_the_html_header_when_lines_are_written() {
    assert!(opening(&counted("html", summary_after_lines())).starts_with("<!DOCTYPE html>"));
}

#[test]
fn there_is_no_opening_when_a_report_replaces_the_lines() {
    assert_eq!(opening(&counted("html", stats_report())), "");
}

#[test]
fn there_is_no_opening_for_a_format_without_a_header() {
    assert_eq!(opening(&counted("plain", StatsOptions::default())), "");
}

#[test]
fn the_progress_is_the_report() {
    assert!(
        progress(&counted("plain", stats_report()), &Stats::default()).starts_with("Statistics\n")
    );
}

#[test]
fn there_is_no_progress_when_lines_are_written() {
    assert_eq!(
        progress(&counted("plain", summary_after_lines()), &Stats::default()),
        ""
    );
}

#[test]
fn the_closing_is_the_summary() {
    assert!(
        closing(&counted("plain", summary_after_lines()), &Stats::default())
            .starts_with("Summary\n")
    );
}

#[test]
fn the_closing_in_html_is_the_escaped_summary_then_the_footer() {
    let options = StatsOptions {
        format: Some("json".to_string()),
        ..summary_after_lines()
    };
    let closing = closing(&counted("html", options), &Stats::default());

    assert!(closing.starts_with("{&quot;lines&quot;:0"));
    assert!(closing.ends_with("</html>\n"));
}

#[test]
fn the_closing_after_a_report_has_no_footer() {
    assert_eq!(
        closing(&counted("html", stats_report()), &Stats::default()),
        ""
    );
}

fn numbered(jobs: usize) -> Settings {
    let options = Options {
        mode: Some("ad-hoc".to_string()),
        output: Some("html".to_string()),
        jobs: Some(jobs),
        line_numbers: true,
        no_wordcolor: true,
        ..Options::default()
    };

    Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(std::path::PathBuf::from("unused")),
    )
    .unwrap()
}

fn line_numbers(rendered: &str) -> Vec<usize> {
    rendered
        .lines()
        .map(|line| {
            line.strip_prefix("<span class=\"splash-line-number\">")
                .and_then(|rest| rest.split_once('<'))
                .map(|(number, _)| number.parse().unwrap())
                .unwrap()
        })
        .collect()
}

#[test]
fn line_numbers_count_blank_lines_and_carry_from_one_chunk_to_the_next() {
    let settings = numbered(1);
    let mut entry = Entry::default();

    let first = render_contents_settings("alpha\n\nbeta\n", &settings, &mut entry);
    let second = render_contents_settings("gamma\n", &settings, &mut entry);

    assert_eq!(line_numbers(&first), [1, 3]);
    assert_eq!(line_numbers(&second), [4]);
}

#[test]
fn line_numbers_follow_the_input_across_worker_threads() {
    let contents: String = (1..=PARALLEL_THRESHOLD * 2)
        .map(|number| format!("line {}\n", number))
        .collect();

    let rendered = render_contents_settings(&contents, &numbered(4), &mut Entry::default());

    assert_eq!(
        line_numbers(&rendered),
        (1..=PARALLEL_THRESHOLD * 2).collect::<Vec<_>>()
    );
}
