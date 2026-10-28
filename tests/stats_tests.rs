use splash::filter::Level;
use splash::output::{ParsedLine, Token, TokenKind};
use splash::parser::parse_line;
use splash::stats::{
    escape_csv, number, sparkline, sparkline_with, Bucket, Facts, Period, Report, Stats,
    StatsError, StatsFormat, StatsOptions, StatsSettings, BAR_WIDTH, DEFAULT_BUCKET, TOP_COUNT,
};
use splash::zone::Zone;
use std::path::PathBuf;

fn example_contents(filename: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("examples")
            .join(filename),
    )
    .unwrap()
}

/// Settings writing in UTC, built from the options
fn settings(options: StatsOptions) -> StatsSettings {
    StatsSettings::build_at(&options, Zone::Utc, Zone::Utc, 0).unwrap()
}

fn report(stats: bool, histogram: Option<&str>, format: &str) -> StatsSettings {
    settings(StatsOptions {
        stats,
        histogram: histogram.map(str::to_string),
        format: Some(format.to_string()),
        ..StatsOptions::default()
    })
}

fn summary(format: &str) -> StatsSettings {
    settings(StatsOptions {
        summary: true,
        format: Some(format.to_string()),
        ..StatsOptions::default()
    })
}

/// The statistics of the example access log
fn access_stats() -> Stats {
    let mut stats = Stats::default();

    for line in example_contents("stats_access.log").lines() {
        let parsed = parse_line(line, "httpd").unwrap();
        stats.record(Facts::of(&parsed, &Zone::Utc, 0));
    }

    stats
}

/// The statistics of lines with no timestamps
fn untimed_stats() -> Stats {
    let mut stats = Stats::default();
    stats.record(Facts::default());

    stats
}

fn facts(line: &str, mode: &str) -> Facts {
    Facts::of(&parse_line(line, mode).unwrap(), &Zone::Utc, 0)
}

const ACCESS_LINE: &str = r#"10.0.0.5 - alice [03/Oct/2023:12:00:01 +0000] "GET /index.html HTTP/1.1" 503 5120 "-" "curl/8.4.0""#;

#[test]
fn statistics_formats_are_read_in_any_case() {
    assert_eq!(
        ["text", "JSON", "csv"].map(|name| name.parse::<StatsFormat>().unwrap()),
        [StatsFormat::Text, StatsFormat::Json, StatsFormat::Csv]
    );
}

#[test]
fn an_unknown_statistics_format_is_rejected() {
    assert_eq!(
        "xml".parse::<StatsFormat>(),
        Err(StatsError::UnknownFormat("xml".to_string()))
    );
}

#[test]
fn histogram_periods_are_read_in_seconds_minutes_hours_and_days() {
    assert_eq!(
        ["30s", "5m", "2h", "1d"].map(|period| period.parse::<Bucket>().unwrap().seconds),
        [30, 300, 7_200, 86_400]
    );
}

#[test]
fn a_histogram_period_keeps_its_name() {
    assert_eq!("5m".parse::<Bucket>().unwrap().name, "5m");
}

#[test]
fn a_histogram_period_that_cannot_be_used_is_rejected() {
    for period in [
        "0h",
        "h",
        "1w",
        "99999999999999999999d",
        "999999999999999999d",
    ] {
        assert_eq!(
            period.parse::<Bucket>(),
            Err(StatsError::BadBucket(period.to_string())),
            "{}",
            period
        );
    }
}

#[test]
fn statistics_errors_describe_the_problem() {
    assert_eq!(
        StatsError::UnknownFormat("xml".to_string()).to_string(),
        "Unknown statistics format 'xml' (expected text, json, or csv)"
    );
    assert_eq!(
        StatsError::BadBucket("1w".to_string()).to_string(),
        "Invalid histogram period '1w' (expected a count and a unit, such as 30s, 5m, 1h, or 1d)"
    );
}

#[test]
fn the_default_histogram_period_is_an_hour() {
    assert_eq!(DEFAULT_BUCKET, "1h");
}

#[test]
fn without_options_a_run_writes_its_lines() {
    let settings = settings(StatsOptions::default());

    assert_eq!(
        (
            settings.report.clone(),
            settings.format,
            settings.shows_lines(),
            settings.collects()
        ),
        (Report::Lines, StatsFormat::Text, true, false)
    );
}

#[test]
fn stats_replace_the_lines_with_the_report() {
    let settings = report(true, None, "text");

    assert_eq!(
        (
            settings.report.clone(),
            settings.shows_lines(),
            settings.collects()
        ),
        (Report::Stats, false, true)
    );
}

#[test]
fn a_histogram_replaces_the_lines() {
    assert_eq!(
        report(false, Some("5m"), "text").report,
        Report::Histogram("5m".parse().unwrap())
    );
}

#[test]
fn a_summary_keeps_the_lines_and_counts_them() {
    let settings = summary("text");

    assert_eq!((settings.shows_lines(), settings.collects()), (true, true));
}

#[test]
fn an_unknown_format_option_is_rejected() {
    let options = StatsOptions {
        stats: true,
        format: Some("xml".to_string()),
        ..StatsOptions::default()
    };

    assert_eq!(
        StatsSettings::build_at(&options, Zone::Utc, Zone::Utc, 0),
        Err(StatsError::UnknownFormat("xml".to_string()))
    );
}

#[test]
fn building_without_statistics_reads_no_zone_or_clock() {
    assert_eq!(
        StatsSettings::build(&StatsOptions::default(), None),
        Ok(StatsSettings::default())
    );
}

#[test]
fn building_with_statistics_reads_the_local_zone_and_clock() {
    let options = StatsOptions {
        summary: true,
        ..StatsOptions::default()
    };
    let settings = StatsSettings::build(&options, None).unwrap();

    assert_eq!(
        (settings.source.clone(), settings.display, settings.now > 0),
        (Zone::local(), Zone::local(), true)
    );
}

#[test]
fn building_with_a_display_zone_writes_times_in_it() {
    let options = StatsOptions {
        summary: true,
        ..StatsOptions::default()
    };
    let zone = Zone::named("EST5").unwrap();

    assert_eq!(
        StatsSettings::build(&options, Some(zone.clone()))
            .unwrap()
            .display,
        zone
    );
}

#[test]
fn an_access_log_line_gives_its_request_facts() {
    assert_eq!(
        facts(ACCESS_LINE, "httpd"),
        Facts {
            level: None,
            moment: Some(1_696_334_401),
            status: Some("503".to_string()),
            addresses: vec!["10.0.0.5".to_string()],
            request: true,
            url: Some("/index.html".to_string()),
            user_agent: Some("curl/8.4.0".to_string()),
            value: None,
        }
    );
}

#[test]
fn a_line_gives_its_level() {
    let line = "Oct  3 12:00:01 web01 app[42]: [error] disk full";

    assert_eq!(facts(line, "syslog").level, Some(Level::Error));
}

#[test]
fn the_address_inside_an_address_field_is_counted() {
    let parsed = ParsedLine::new(vec![Token::new("from=10.0.0.5,", TokenKind::Ip)]);

    assert_eq!(
        Facts::of(&parsed, &Zone::Utc, 0).addresses,
        vec!["10.0.0.5"]
    );
}

#[test]
fn a_client_written_as_a_host_name_is_counted_as_written() {
    let parsed = ParsedLine::new(vec![Token::new("gateway.lan", TokenKind::Client)]);

    assert_eq!(
        Facts::of(&parsed, &Zone::Utc, 0).addresses,
        vec!["gateway.lan"]
    );
}

#[test]
fn a_request_field_on_a_line_with_no_method_is_not_a_url() {
    let parsed = ParsedLine::new(vec![Token::new("/usr/bin/backup", TokenKind::Request)]);

    assert_eq!(Facts::of(&parsed, &Zone::Utc, 0).url, None);
}

#[test]
fn an_ad_hoc_http_verb_makes_a_line_a_request() {
    assert!(facts("10.0.0.5 GET /index.html", "ad-hoc").request);
}

#[test]
fn error_and_critical_levels_are_errors() {
    for (level, error) in [
        (Level::Warning, false),
        (Level::Error, true),
        (Level::Critical, true),
    ] {
        let facts = Facts {
            level: Some(level),
            ..Facts::default()
        };

        assert_eq!(facts.is_error(), error, "{:?}", level);
    }
}

#[test]
fn a_status_starting_with_five_is_an_error() {
    for (status, error) in [("503", true), ("5.1.1", true), ("404", false)] {
        let facts = Facts {
            status: Some(status.to_string()),
            ..Facts::default()
        };

        assert_eq!(facts.is_error(), error, "{}", status);
    }
}

#[test]
fn recording_counts_lines_errors_and_requests() {
    let stats = access_stats();

    assert_eq!((stats.lines, stats.errors, stats.requests), (6, 2, 6));
}

#[test]
fn recording_keeps_the_earliest_and_latest_timestamps_in_any_order() {
    let mut stats = Stats::default();

    for moment in [200, 100, 300] {
        stats.record(Facts {
            moment: Some(moment),
            ..Facts::default()
        });
    }

    assert_eq!((stats.first, stats.last), (Some(100), Some(300)));
}

#[test]
fn recording_counts_levels_and_every_address_on_a_line() {
    let mut stats = Stats::default();
    stats.record(Facts {
        level: Some(Level::Info),
        addresses: vec!["10.0.0.5".to_string(), "10.0.0.6".to_string()],
        ..Facts::default()
    });

    assert_eq!(
        (stats.levels.get(&Level::Info), stats.addresses.len()),
        (Some(&1), 2)
    );
}

#[test]
fn there_is_no_error_rate_without_lines() {
    assert_eq!(Stats::default().error_percent(), None);
}

#[test]
fn there_is_no_request_rate_without_timestamps() {
    assert_eq!(untimed_stats().request_rate(), None);
}

#[test]
fn there_is_no_request_rate_when_every_line_has_the_same_time() {
    let mut stats = Stats::default();
    stats.record(Facts {
        moment: Some(100),
        request: true,
        ..Facts::default()
    });

    assert_eq!(stats.request_rate(), None);
}

#[test]
fn the_request_rate_is_requests_per_second_between_the_first_and_last_timestamps() {
    let mut stats = Stats::default();

    for moment in [100, 102, 104] {
        stats.record(Facts {
            moment: Some(moment),
            request: true,
            ..Facts::default()
        });
    }

    assert_eq!(stats.request_rate(), Some(0.75));
}

#[test]
fn buckets_cover_every_period_from_the_first_timestamp_to_the_last() {
    assert_eq!(
        access_stats().buckets(3_600, &Zone::Utc),
        vec![
            ("2023-10-03T12:00:00Z".to_string(), 4),
            ("2023-10-03T13:00:00Z".to_string(), 0),
            ("2023-10-03T14:00:00Z".to_string(), 2),
        ]
    );
}

#[test]
fn buckets_start_on_the_wall_clock_of_the_zone() {
    assert_eq!(
        access_stats().buckets(86_400, &Zone::named("EST5").unwrap()),
        vec![("2023-10-03T00:00:00-05:00".to_string(), 6)]
    );
}

#[test]
fn there_are_no_buckets_without_timestamps() {
    assert_eq!(untimed_stats().buckets(3_600, &Zone::Utc), vec![]);
}

#[test]
fn a_run_that_writes_its_lines_writes_no_report() {
    assert_eq!(
        settings(StatsOptions::default()).report(&access_stats()),
        None
    );
}

#[test]
fn a_run_that_asks_for_no_summary_writes_none() {
    assert_eq!(report(true, None, "text").summary(&access_stats()), None);
}

#[test]
fn the_statistics_report_is_written_as_text() {
    assert_eq!(
        report(true, None, "text").report(&access_stats()).unwrap(),
        "\
Statistics
==========
Lines:         6
Errors:        2
Error rate:    33.3%
Requests:      6
Request rate:  0.0008/s
From:          2023-10-03T12:00:01Z
To:            2023-10-03T14:10:01Z

Status codes:
  200  2
  302  1
  404  1
  500  1
  503  1

Top addresses:
  10.0.0.5  3
  10.0.0.7  2
  10.0.0.9  1

Top URLs:
  /index.html    3
  /about.html    1
  /api/queue/91  1
  /login         1

Top user agents:
  curl/8.4.0                       3
  Mozilla/5.0 (X11; Linux x86_64)  1
  Wget/1.21.4                      1
  python-requests/2.31.0           1
"
    );
}

#[test]
fn the_statistics_report_lists_levels() {
    let mut stats = Stats::default();
    stats.record(Facts {
        level: Some(Level::Warning),
        ..Facts::default()
    });

    assert!(report(true, None, "text")
        .report(&stats)
        .unwrap()
        .contains("\nLevels:\n  warning  1\n"));
}

#[test]
fn the_statistics_report_is_written_as_json() {
    let mut stats = Stats::default();
    stats.record(facts(ACCESS_LINE, "httpd"));

    assert_eq!(
        report(true, None, "json").report(&stats).unwrap(),
        concat!(
            r#"{"lines":1,"errors":1,"error_percent":100.0,"requests":1,"request_rate":null,"#,
            r#""from":"2023-10-03T12:00:01Z","to":"2023-10-03T12:00:01Z","levels":[],"#,
            r#""statuses":[{"value":"503","count":1}],"#,
            r#""top_addresses":[{"value":"10.0.0.5","count":1}],"#,
            r#""top_urls":[{"value":"/index.html","count":1}],"#,
            r#""top_user_agents":[{"value":"curl/8.4.0","count":1}]}"#,
            "\n"
        )
    );
}

#[test]
fn the_statistics_report_is_written_as_csv() {
    let mut stats = Stats::default();
    stats.record(facts(
        r#"10.0.0.5 - - [03/Oct/2023:12:00:01 +0000] "GET /a,b HTTP/1.1" 200 1 "-" "Mozilla/5.0 (X11, Linux)""#,
        "httpd",
    ));

    assert_eq!(
        report(true, None, "csv").report(&stats).unwrap(),
        "\
metric,key,value
lines,,1
errors,,0
error_percent,,0.0
requests,,1
request_rate,,
from,,2023-10-03T12:00:01Z
to,,2023-10-03T12:00:01Z
status,200,1
address,10.0.0.5,1
url,\"/a,b\",1
user_agent,\"Mozilla/5.0 (X11, Linux)\",1
"
    );
}

#[test]
fn the_top_tables_list_the_most_frequent_values_first() {
    let mut stats = Stats::default();

    for index in 0..TOP_COUNT + 2 {
        stats.record(Facts {
            addresses: vec![format!("10.0.0.{}", index); index % 2 + 1],
            ..Facts::default()
        });
    }

    let text = report(true, None, "text").report(&stats).unwrap();
    let addresses: Vec<&str> = text
        .lines()
        .skip_while(|line| *line != "Top addresses:")
        .skip(1)
        .collect();

    assert_eq!(addresses.len(), TOP_COUNT);
    assert_eq!(addresses[0], "  10.0.0.1   2");
    assert_eq!(addresses[TOP_COUNT - 1], "  10.0.0.4   1");
}

#[test]
fn the_summary_is_written_as_text() {
    assert_eq!(
        summary("text").summary(&access_stats()).unwrap(),
        "\
Summary
=======
Lines:       6
Errors:      2
Error rate:  33.3%
From:        2023-10-03T12:00:01Z
To:          2023-10-03T14:10:01Z
"
    );
}

#[test]
fn a_summary_without_timestamps_has_no_time_range() {
    assert_eq!(
        summary("text").summary(&untimed_stats()).unwrap(),
        "\
Summary
=======
Lines:       1
Errors:      0
Error rate:  0.0%
From:        -
To:          -
"
    );
}

#[test]
fn the_summary_is_written_as_json() {
    assert_eq!(
        summary("json").summary(&Stats::default()).unwrap(),
        "{\"lines\":0,\"errors\":0,\"error_percent\":null,\"from\":null,\"to\":null}\n"
    );
}

#[test]
fn the_summary_is_written_as_csv() {
    assert_eq!(
        summary("csv").summary(&untimed_stats()).unwrap(),
        "metric,key,value\nlines,,1\nerrors,,0\nerror_percent,,0.0\nfrom,,\nto,,\n"
    );
}

#[test]
fn the_histogram_is_written_as_text_with_bars_scaled_to_the_busiest_period() {
    assert_eq!(
        report(false, Some("1h"), "text")
            .report(&access_stats())
            .unwrap(),
        format!(
            "Lines per 1h\n============\n\
             2023-10-03T12:00:00Z  {}  4\n\
             2023-10-03T13:00:00Z  {}  0\n\
             2023-10-03T14:00:00Z  {}{}  2\n",
            "#".repeat(BAR_WIDTH as usize),
            " ".repeat(BAR_WIDTH as usize),
            "#".repeat(BAR_WIDTH as usize / 2),
            " ".repeat(BAR_WIDTH as usize / 2),
        )
    );
}

#[test]
fn a_histogram_without_timestamps_has_only_its_title() {
    assert_eq!(
        report(false, Some("5m"), "text")
            .report(&untimed_stats())
            .unwrap(),
        "Lines per 5m\n============\n"
    );
}

#[test]
fn the_histogram_is_written_as_json() {
    assert_eq!(
        report(false, Some("2h"), "json")
            .report(&access_stats())
            .unwrap(),
        "{\"bucket\":\"2h\",\"buckets\":[{\"start\":\"2023-10-03T12:00:00Z\",\"count\":4},{\"start\":\"2023-10-03T14:00:00Z\",\"count\":2}]}\n"
    );
}

#[test]
fn the_histogram_is_written_as_csv() {
    assert_eq!(
        report(false, Some("1d"), "csv")
            .report(&access_stats())
            .unwrap(),
        "start,count\n2023-10-03T00:00:00Z,6\n"
    );
}

#[test]
fn csv_fields_holding_commas_quotes_or_line_breaks_are_quoted() {
    assert_eq!(
        ["plain", "a,b", "say \"hi\"", "two\nlines", "cr\rhere"].map(escape_csv),
        [
            "plain",
            "\"a,b\"",
            "\"say \"\"hi\"\"\"",
            "\"two\nlines\"",
            "\"cr\rhere\""
        ]
    );
}

/// Settings drawing a graph per minute in `format`, with the average of
/// `field` when one is given
fn graph(field: Option<&str>, format: &str) -> StatsSettings {
    settings(StatsOptions {
        graph: Some("1m".to_string()),
        graph_field: field.map(str::to_string),
        format: Some(format.to_string()),
        ..StatsOptions::default()
    })
}

/// The statistics of the example squid log, with its durations recorded
fn graph_stats(settings: &StatsSettings) -> Stats {
    let mut stats = Stats::default();

    for line in example_contents("graph_squid.log").lines() {
        stats.record(settings.facts(&parse_line(line, "squid").unwrap()));
    }

    stats
}

#[test]
fn a_graph_replaces_the_lines() {
    let graph = graph(None, "text");

    assert_eq!(
        (graph.report.clone(), graph.shows_lines()),
        (
            Report::Graph(Bucket {
                name: "1m".to_string(),
                seconds: 60
            }),
            false
        )
    );
}

#[test]
fn a_graph_field_must_be_a_color_key() {
    let error = StatsSettings::build_at(
        &StatsOptions {
            graph: Some("1m".to_string()),
            graph_field: Some("latency".to_string()),
            ..StatsOptions::default()
        },
        Zone::Utc,
        Zone::Utc,
        0,
    )
    .unwrap_err();

    assert_eq!(
        (error.clone(), error.to_string()),
        (
            StatsError::UnknownField("latency".to_string()),
            "Unknown graph field 'latency'".to_string()
        )
    );
}

#[test]
fn a_line_gives_the_number_its_graph_field_starts_with() {
    let settings = graph(Some("duration"), "text");
    let line = |text: &'static str| ParsedLine::new(vec![Token::new(text, TokenKind::Duration)]);

    assert_eq!(
        [
            settings.facts(&line("\"0.250\"")).value,
            settings.facts(&line("120ms")).value,
            settings.facts(&line("-")).value,
            graph(None, "text").facts(&line("120ms")).value,
        ],
        [Some(0.25), Some(120.0), None, None]
    );
}

#[test]
fn each_period_holds_its_lines_errors_and_average() {
    let stats = graph_stats(&graph(Some("duration"), "text"));
    let period = |start: &str, lines, errors, average| Period {
        start: start.to_string(),
        lines,
        errors,
        average,
    };

    assert_eq!(
        stats.periods(60, &Zone::Utc),
        vec![
            period("2021-03-03T00:00:00Z", 3, 0, Some(200.0)),
            period("2021-03-03T00:01:00Z", 1, 0, Some(50.0)),
            period("2021-03-03T00:02:00Z", 0, 0, None),
            period("2021-03-03T00:03:00Z", 4, 2, Some(1000.0)),
        ]
    );
}

#[test]
fn a_sparkline_scales_its_values_to_the_largest_and_leaves_a_space_for_none() {
    assert_eq!(
        (
            sparkline(&[Some(0.0), Some(1.0), Some(3.0), None, Some(4.0)]),
            sparkline(&[Some(0.0), Some(0.0)]),
            sparkline(&[Some(-2.0), Some(2.0)]),
        ),
        ("▁▃▇ █".to_string(), "▁▁".to_string(), "▁█".to_string())
    );
}

#[test]
fn numbers_are_written_without_trailing_zeros() {
    assert_eq!(
        [number(1000.0), number(361.6666), number(0.25)],
        ["1000", "361.667", "0.25"]
    );
}

#[test]
fn the_graph_is_written_as_text_with_a_sparkline_for_each_series() {
    let settings = graph(Some("duration"), "text");

    assert_eq!(
        settings.report(&graph_stats(&settings)).unwrap(),
        concat!(
            "Graph per 1m\n",
            "============\n",
            "2021-03-03T00:00:00Z to 2021-03-03T00:03:00Z\n",
            "lines             ▇▃▁█  max 4\n",
            "errors            ▁▁▁█  max 2\n",
            "average duration  ▃▂ █  max 1000\n",
        )
    );
}

#[test]
fn a_graph_field_no_line_holds_has_no_maximum() {
    let settings = graph(Some("email"), "text");

    assert!(settings
        .report(&graph_stats(&settings))
        .unwrap()
        .ends_with(&format!("average email  {}  max -\n", " ".repeat(4))));
}

#[test]
fn a_graph_without_timestamps_has_only_its_title() {
    assert_eq!(
        graph(None, "text").report(&untimed_stats()).unwrap(),
        "Graph per 1m\n============\n"
    );
}

#[test]
fn the_graph_is_written_as_json() {
    let with_field = graph(Some("duration"), "json");
    let without_field = graph(None, "json");

    assert_eq!(
        (
            with_field.report(&graph_stats(&with_field)).unwrap(),
            without_field.report(&graph_stats(&without_field)).unwrap()
        ),
        (
            concat!(
                r#"{"bucket":"1m","buckets":["#,
                r#"{"start":"2021-03-03T00:00:00Z","lines":3,"errors":0,"average_duration":200},"#,
                r#"{"start":"2021-03-03T00:01:00Z","lines":1,"errors":0,"average_duration":50},"#,
                r#"{"start":"2021-03-03T00:02:00Z","lines":0,"errors":0,"average_duration":null},"#,
                r#"{"start":"2021-03-03T00:03:00Z","lines":4,"errors":2,"average_duration":1000}]}"#,
                "\n"
            )
            .to_string(),
            concat!(
                r#"{"bucket":"1m","buckets":["#,
                r#"{"start":"2021-03-03T00:00:00Z","lines":3,"errors":0},"#,
                r#"{"start":"2021-03-03T00:01:00Z","lines":1,"errors":0},"#,
                r#"{"start":"2021-03-03T00:02:00Z","lines":0,"errors":0},"#,
                r#"{"start":"2021-03-03T00:03:00Z","lines":4,"errors":2}]}"#,
                "\n"
            )
            .to_string()
        )
    );
}

#[test]
fn the_graph_is_written_as_csv() {
    let with_field = graph(Some("duration"), "csv");
    let without_field = graph(None, "csv");

    assert_eq!(
        (
            with_field.report(&graph_stats(&with_field)).unwrap(),
            without_field.report(&graph_stats(&without_field)).unwrap()
        ),
        (
            concat!(
                "start,lines,errors,average_duration\n",
                "2021-03-03T00:00:00Z,3,0,200\n",
                "2021-03-03T00:01:00Z,1,0,50\n",
                "2021-03-03T00:02:00Z,0,0,\n",
                "2021-03-03T00:03:00Z,4,2,1000\n",
            )
            .to_string(),
            concat!(
                "start,lines,errors\n",
                "2021-03-03T00:00:00Z,3,0\n",
                "2021-03-03T00:01:00Z,1,0\n",
                "2021-03-03T00:02:00Z,0,0\n",
                "2021-03-03T00:03:00Z,4,2\n",
            )
            .to_string()
        )
    );
}

#[test]
fn a_sparkline_for_latin1_output_is_drawn_in_ascii() {
    assert_eq!(
        sparkline_with(
            &[Some(0.0), Some(1.0), Some(4.0), None],
            splash::source::Encoding::Latin1
        ),
        "_-# "
    );
}
