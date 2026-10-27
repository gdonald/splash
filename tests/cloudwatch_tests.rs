#[path = "support/fake_cloudwatch.rs"]
mod fake_cloudwatch;

use aws_sdk_cloudwatchlogs::config::retry::RetryConfig;
use aws_sdk_cloudwatchlogs::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_cloudwatchlogs::types::FilteredLogEvent;
use aws_sdk_cloudwatchlogs::Config;
use fake_cloudwatch::FakeCloudWatch;
use splash::cloudwatch::{self, Connection, Cursor, Query, FOLLOW_POLL};
use splash::filter::TimeRange;
use splash::listen::Received;
use splash::zone::Zone;
use std::time::Duration;

/// How long a test waits for a line it expects
const LINE_DEADLINE: Duration = Duration::from_secs(10);

fn connection_to(server: &FakeCloudWatch) -> Connection {
    configured(&server.endpoint)
}

fn configured(endpoint: &str) -> Connection {
    Connection::Configured(Box::new(
        Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .credentials_provider(Credentials::new(
                "AKIDEXAMPLE",
                "secret",
                None,
                None,
                "test",
            ))
            .retry_config(RetryConfig::disabled())
            .endpoint_url(endpoint)
            .build(),
    ))
}

fn event(stream: &str, timestamp: i64, id: &str, message: &str) -> FilteredLogEvent {
    FilteredLogEvent::builder()
        .log_stream_name(stream)
        .timestamp(timestamp)
        .event_id(id)
        .message(message)
        .build()
}

fn received(sender: &str, line: &str) -> Received {
    Received {
        sender: sender.to_string(),
        line: line.to_string(),
    }
}

fn read_all(server: &FakeCloudWatch, query: Query) -> (Vec<Received>, Result<(), String>) {
    let (lines, reader) = cloudwatch::start(connection_to(server), query);
    let lines = lines.iter().collect();

    (lines, reader.join().unwrap())
}

const FIRST_PAGE: &str = r#"{"events":[
  {"logStreamName":"web/web/3f9a1c2e","timestamp":1793109601000,"message":"GET /health 200 3ms\n","eventId":"1"},
  {"logStreamName":"web/web/8b7d0e41","timestamp":1793109602000,"message":"GET /orders 200 41ms","eventId":"2"}
],"nextToken":"page-2"}"#;

const SECOND_PAGE: &str = r#"{"events":[
  {"logStreamName":"web/web/3f9a1c2e","timestamp":1793109603000,"message":"POST /orders 201 88ms\nqueued order 5521\n","eventId":"3"}
]}"#;

#[test]
fn a_new_query_reads_every_event_of_the_group_once() {
    assert_eq!(
        Query::new("app-logs"),
        Query {
            group: "app-logs".to_string(),
            start_time: None,
            end_time: None,
            follow: false,
            poll: FOLLOW_POLL,
        }
    );
}

#[test]
fn a_time_range_sets_the_start_and_end_in_milliseconds() {
    let range = TimeRange {
        since: Some(1_793_109_600),
        until: Some(1_793_113_200),
        zone: Zone::Utc,
        now: 0,
    };

    let query = Query::new("app-logs").within(Some(&range));

    assert_eq!(
        (query.start_time, query.end_time),
        (Some(1_793_109_600_000), Some(1_793_113_200_000))
    );
}

#[test]
fn no_time_range_leaves_the_start_and_end_open() {
    let query = Query::new("app-logs").within(None);

    assert_eq!((query.start_time, query.end_time), (None, None));
}

#[test]
fn a_followed_query_keeps_asking_for_events() {
    assert!(Query::new("app-logs").following(true).follow);
}

#[test]
fn each_line_of_a_message_is_sent_from_its_log_stream() {
    let lines = cloudwatch::lines_of(&event(
        "worker/1",
        1,
        "1",
        "retrying job 77\nattempt 2 of 5\n",
    ));

    assert_eq!(
        lines,
        vec![
            received("worker/1", "retrying job 77"),
            received("worker/1", "attempt 2 of 5"),
        ]
    );
}

#[test]
fn an_event_without_a_message_sends_no_lines() {
    let event = FilteredLogEvent::builder()
        .log_stream_name("worker/1")
        .build();

    assert_eq!(cloudwatch::lines_of(&event), vec![]);
}

#[test]
fn an_event_without_a_log_stream_is_sent_from_no_one() {
    let event = FilteredLogEvent::builder().message("cache warmed").build();

    assert_eq!(
        cloudwatch::lines_of(&event),
        vec![received("", "cache warmed")]
    );
}

#[test]
fn a_cursor_starts_at_the_query_start() {
    assert_eq!(Cursor::starting_at(Some(5000)).start_time(), Some(5000));
}

#[test]
fn a_cursor_moves_to_the_newest_event_it_admits() {
    let mut cursor = Cursor::starting_at(None);

    cursor.admit(&event("web", 2000, "1", "a"));
    cursor.admit(&event("web", 1000, "2", "b"));

    assert_eq!(cursor.start_time(), Some(2000));
}

#[test]
fn a_cursor_skips_an_event_it_already_admitted_at_its_latest_timestamp() {
    let mut cursor = Cursor::starting_at(None);
    cursor.admit(&event("web", 2000, "1", "a"));

    assert!(!cursor.admit(&event("web", 2000, "1", "a")));
}

#[test]
fn a_cursor_admits_another_event_at_its_latest_timestamp() {
    let mut cursor = Cursor::starting_at(None);
    cursor.admit(&event("web", 2000, "1", "a"));

    assert!(cursor.admit(&event("web", 2000, "2", "b")));
}

#[test]
fn a_cursor_admits_an_event_at_the_start_time() {
    let mut cursor = Cursor::starting_at(Some(2000));

    assert!(cursor.admit(&event("web", 2000, "1", "a")));
}

#[test]
fn a_cursor_admits_an_event_without_a_timestamp() {
    let mut cursor = Cursor::starting_at(Some(2000));
    let event = FilteredLogEvent::builder()
        .event_id("1")
        .message("a")
        .build();

    assert!(cursor.admit(&event));
}

#[test]
fn every_page_of_events_is_read() {
    let server = FakeCloudWatch::start(&[FIRST_PAGE, SECOND_PAGE]);

    let (lines, result) = read_all(&server, Query::new("app-logs"));

    assert_eq!(
        (lines, result),
        (
            vec![
                received("web/web/3f9a1c2e", "GET /health 200 3ms"),
                received("web/web/8b7d0e41", "GET /orders 200 41ms"),
                received("web/web/3f9a1c2e", "POST /orders 201 88ms"),
                received("web/web/3f9a1c2e", "queued order 5521"),
            ],
            Ok(())
        )
    );
}

#[test]
fn the_next_page_is_asked_for_with_the_token_of_the_last() {
    let server = FakeCloudWatch::start(&[FIRST_PAGE, SECOND_PAGE]);

    let _ = read_all(&server, Query::new("app-logs"));

    let requests = server.requests();

    assert!(
        requests[1].contains(r#""nextToken":"page-2""#),
        "{:?}",
        requests
    );
}

#[test]
fn the_group_and_time_range_are_asked_for() {
    let server = FakeCloudWatch::start(&[SECOND_PAGE]);
    let query = Query {
        start_time: Some(1_793_109_600_000),
        end_time: Some(1_793_113_200_000),
        ..Query::new("app-logs")
    };

    let _ = read_all(&server, query);

    assert_eq!(
        server.requests(),
        vec![r#"{"logGroupName":"app-logs","startTime":1793109600000,"endTime":1793113200000}"#]
    );
}

#[test]
fn a_failed_request_names_the_group_and_the_error() {
    let server = FakeCloudWatch::answering(vec![(
        400,
        r#"{"__type":"ResourceNotFoundException","message":"The specified log group does not exist."}"#
            .to_string(),
    )]);

    let (lines, result) = read_all(&server, Query::new("missing-logs"));

    assert_eq!(
        (lines, result),
        (
            vec![],
            Err("Cannot read CloudWatch log group missing-logs: The specified log group does not exist.".to_string())
        )
    );
}

#[test]
fn a_request_that_cannot_connect_names_the_cause() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);

    let (lines, reader) = cloudwatch::start(configured(&endpoint), Query::new("app-logs"));
    let message = reader.join().unwrap().unwrap_err();

    assert_eq!(
        (
            lines.iter().count(),
            message.starts_with("Cannot read CloudWatch log group app-logs: dispatch failure")
        ),
        (0, true),
        "{}",
        message
    );
}

#[test]
fn reading_stops_when_nothing_receives_the_lines() {
    let server = FakeCloudWatch::start(&[FIRST_PAGE]);
    let (lines, reader) = cloudwatch::start(
        connection_to(&server),
        Query::new("app-logs").following(true),
    );

    drop(lines);

    assert_eq!(reader.join().unwrap(), Ok(()));
}

#[test]
fn a_followed_group_sends_the_events_written_since_the_last_one_sent() {
    let repeated_and_new = r#"{"events":[
      {"logStreamName":"web/web/8b7d0e41","timestamp":1793109602000,"message":"GET /orders 200 41ms","eventId":"2"},
      {"logStreamName":"web/web/8b7d0e41","timestamp":1793109609000,"message":"GET /cart 200 12ms","eventId":"4"}
    ]}"#;
    let first_only = r#"{"events":[
      {"logStreamName":"web/web/8b7d0e41","timestamp":1793109602000,"message":"GET /orders 200 41ms","eventId":"2"}
    ]}"#;
    let server = FakeCloudWatch::start(&[first_only, repeated_and_new]);
    let query = Query {
        poll: Duration::from_millis(20),
        ..Query::new("app-logs").following(true)
    };

    let (lines, _reader) = cloudwatch::start(connection_to(&server), query);
    let first = lines.recv_timeout(LINE_DEADLINE).unwrap();
    let second = lines.recv_timeout(LINE_DEADLINE).unwrap();

    assert_eq!(
        vec![first, second],
        vec![
            received("web/web/8b7d0e41", "GET /orders 200 41ms"),
            received("web/web/8b7d0e41", "GET /cart 200 12ms"),
        ]
    );
}

#[test]
fn a_followed_group_asks_again_from_the_last_event_sent() {
    let first_only = r#"{"events":[
      {"logStreamName":"web/web/8b7d0e41","timestamp":1793109602000,"message":"GET /orders 200 41ms","eventId":"2"}
    ]}"#;
    let new = r#"{"events":[
      {"logStreamName":"web/web/8b7d0e41","timestamp":1793109609000,"message":"GET /cart 200 12ms","eventId":"4"}
    ]}"#;
    let server = FakeCloudWatch::start(&[first_only, new]);
    let query = Query {
        poll: Duration::from_millis(20),
        ..Query::new("app-logs").following(true)
    };

    let (lines, _reader) = cloudwatch::start(connection_to(&server), query);
    lines.recv_timeout(LINE_DEADLINE).unwrap();
    lines.recv_timeout(LINE_DEADLINE).unwrap();

    assert!(server.requests()[1].contains(r#""startTime":1793109602000"#));
}
