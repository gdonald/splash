//! Reading CloudWatch Logs
//!
//! `--cloudwatch GROUP` reads the events of a log group through the AWS SDK,
//! which finds the region and credentials as the AWS CLI does: from the
//! `AWS_*` environment variables, `~/.aws/config` and `~/.aws/credentials`,
//! and the instance or container the program runs on. A thread of its own
//! reads the events, page by page, and sends each line of each message with
//! the name of the log stream it was written to. When following, the thread
//! asks again for the events written since the last one it sent.
use crate::filter::TimeRange;
use crate::listen::Received;
use aws_config::BehaviorVersion;
use aws_sdk_cloudwatchlogs::config::http::HttpResponse;
use aws_sdk_cloudwatchlogs::error::{DisplayErrorContext, ProvideErrorMetadata, SdkError};
use aws_sdk_cloudwatchlogs::operation::filter_log_events::FilterLogEventsError;
use aws_sdk_cloudwatchlogs::types::FilteredLogEvent;
use aws_sdk_cloudwatchlogs::{Client, Config};
use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// How long a followed log group waits before asking for newer events
pub const FOLLOW_POLL: Duration = Duration::from_secs(2);

const MILLISECONDS_PER_SECOND: i64 = 1000;

/// The events of a log group to read
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub group: String,
    /// The earliest event to read, in milliseconds since the epoch
    pub start_time: Option<i64>,
    /// The latest event to read, in milliseconds since the epoch
    pub end_time: Option<i64>,
    /// Whether to keep asking for new events once the written ones are read
    pub follow: bool,
    pub poll: Duration,
}

impl Query {
    /// Reads every event of `group` once
    pub fn new(group: &str) -> Query {
        Query {
            group: group.to_string(),
            start_time: None,
            end_time: None,
            follow: false,
            poll: FOLLOW_POLL,
        }
    }

    /// Reads only the events in the time range `--since` and `--until` keep
    pub fn within(self, range: Option<&TimeRange>) -> Query {
        let milliseconds = |seconds: Option<i64>| seconds.map(|s| s * MILLISECONDS_PER_SECOND);

        Query {
            start_time: milliseconds(range.and_then(|range| range.since)),
            end_time: milliseconds(range.and_then(|range| range.until)),
            ..self
        }
    }

    pub fn following(self, follow: bool) -> Query {
        Query { follow, ..self }
    }
}

/// Where the client finds its region, credentials, and endpoint
#[derive(Debug, Clone)]
pub enum Connection {
    /// The AWS environment variables, config files, and instance metadata
    Environment,
    /// A client configuration built by the caller
    Configured(Box<Config>),
}

/// Starts reading the query's events on a thread of its own. The lines stop
/// arriving when the events are read, when following stops because nothing
/// receives the lines, or when a request fails, and the thread returns the
/// failure.
pub fn start(
    connection: Connection,
    query: Query,
) -> (Receiver<Received>, JoinHandle<Result<(), String>>) {
    let (sender, receiver) = mpsc::channel();

    let reader = thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(read(connection, &query, &sender))
    });

    (receiver, reader)
}

async fn read(
    connection: Connection,
    query: &Query,
    sender: &Sender<Received>,
) -> Result<(), String> {
    let client = match connection {
        Connection::Environment => {
            Client::new(&aws_config::load_defaults(BehaviorVersion::latest()).await)
        }
        Connection::Configured(config) => Client::from_conf(*config),
    };

    let mut cursor = Cursor::starting_at(query.start_time);

    loop {
        let start_time = cursor.start_time();
        let mut next_token = None;

        loop {
            let page = client
                .filter_log_events()
                .log_group_name(&query.group)
                .set_start_time(start_time)
                .set_end_time(query.end_time)
                .set_next_token(next_token)
                .send()
                .await
                .map_err(|e| {
                    format!(
                        "Cannot read CloudWatch log group {}: {}",
                        query.group,
                        describe(e)
                    )
                })?;

            for event in page.events().iter().filter(|event| cursor.admit(event)) {
                for line in lines_of(event) {
                    if sender.send(line).is_err() {
                        return Ok(());
                    }
                }
            }

            next_token = page.next_token().map(String::from);

            if next_token.is_none() {
                break;
            }
        }

        if !query.follow {
            return Ok(());
        }

        tokio::time::sleep(query.poll).await;
    }
}

/// The message the service gave for a failed request, or the whole chain
/// of causes for a request that did not reach the service
fn describe(error: SdkError<FilterLogEventsError, HttpResponse>) -> String {
    match error.message() {
        Some(message) => message.to_string(),
        None => DisplayErrorContext(&error).to_string(),
    }
}

/// Each line of an event's message, sent from its log stream
pub fn lines_of(event: &FilteredLogEvent) -> Vec<Received> {
    let stream = event.log_stream_name().unwrap_or_default();

    event
        .message()
        .unwrap_or_default()
        .lines()
        .map(|line| Received {
            sender: stream.to_string(),
            line: line.to_string(),
        })
        .collect()
}

/// The newest event sent so far. Asking again from its timestamp returns
/// the events sent at that timestamp a second time, which are skipped.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Cursor {
    latest: Option<i64>,
    sent_at_latest: HashSet<String>,
}

impl Cursor {
    pub fn starting_at(start_time: Option<i64>) -> Cursor {
        Cursor {
            latest: start_time,
            sent_at_latest: HashSet::new(),
        }
    }

    /// The timestamp the next request starts from
    pub fn start_time(&self) -> Option<i64> {
        self.latest
    }

    /// Whether `event` has not been sent before, noting it as sent
    pub fn admit(&mut self, event: &FilteredLogEvent) -> bool {
        let id = event.event_id().unwrap_or_default().to_string();

        match event.timestamp() {
            Some(timestamp) if self.latest.is_none_or(|latest| timestamp > latest) => {
                self.latest = Some(timestamp);
                self.sent_at_latest = HashSet::from([id]);
                true
            }
            Some(timestamp) if Some(timestamp) == self.latest => self.sent_at_latest.insert(id),
            _ => true,
        }
    }
}
