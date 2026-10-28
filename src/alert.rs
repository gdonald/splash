//! Notifications sent when an input line matches a pattern
//!
//! Each line matching an alert pattern is posted to a webhook as JSON, shown
//! as a desktop notification, or both. A thread of its own does the sending,
//! so a slow webhook does not hold up the output, and splash waits for it
//! before exiting.
use crate::forward::{self, Destination};
use crate::output::escape_json;
use regex::Regex;
use std::fmt;
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};

/// The title of a desktop notification
pub const TITLE: &str = "splash";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertError {
    BadPattern { pattern: String, message: String },
    BadWebhook(String),
}

impl fmt::Display for AlertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AlertError::BadPattern { pattern, message } => {
                write!(f, "Invalid alert pattern '{}': {}", pattern, message)
            }
            AlertError::BadWebhook(url) => write!(
                f,
                "Invalid webhook URL '{}' (expected http://HOST:PORT/PATH)",
                url
            ),
        }
    }
}

impl std::error::Error for AlertError {}

/// A line that matched an alert pattern, and the pattern
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub pattern: String,
    pub line: String,
}

impl Alert {
    /// The JSON object posted to a webhook
    pub fn json(&self) -> String {
        format!(
            "{{\"pattern\":\"{}\",\"line\":\"{}\"}}",
            escape_json(&self.pattern),
            escape_json(&self.line)
        )
    }
}

/// The patterns that raise an alert
#[derive(Debug, Clone)]
pub struct Patterns {
    patterns: Vec<(String, Regex)>,
}

impl Patterns {
    pub fn build(patterns: &[String]) -> Result<Patterns, AlertError> {
        let patterns = patterns
            .iter()
            .map(|pattern| match Regex::new(pattern) {
                Ok(regex) => Ok((pattern.clone(), regex)),
                Err(error) => Err(AlertError::BadPattern {
                    pattern: pattern.clone(),
                    message: error.to_string(),
                }),
            })
            .collect::<Result<_, _>>()?;

        Ok(Patterns { patterns })
    }

    /// An alert for each line of `text` that matches a pattern, naming the
    /// first pattern it matches
    pub fn alerts(&self, text: &str) -> Vec<Alert> {
        text.lines()
            .filter_map(|line| {
                let (pattern, _) = self
                    .patterns
                    .iter()
                    .find(|(_, regex)| regex.is_match(line))?;

                Some(Alert {
                    pattern: pattern.clone(),
                    line: line.to_string(),
                })
            })
            .collect()
    }
}

/// Where alerts are sent
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targets {
    pub webhook: Option<(String, String)>,
    pub desktop: bool,
}

impl Targets {
    /// Reads `--alert-webhook`, which must be an `http://` URL
    pub fn new(webhook: Option<&str>, desktop: bool) -> Result<Targets, AlertError> {
        let webhook = match webhook.map(|url| (url, Destination::parse(url))) {
            None => None,
            Some((_, Ok(Destination::Http { address, path }))) => Some((address, path)),
            Some((url, _)) => return Err(AlertError::BadWebhook(url.to_string())),
        };

        Ok(Targets { webhook, desktop })
    }
}

/// The command that shows a desktop notification of `line`
#[cfg(target_os = "macos")]
pub fn desktop_command(line: &str) -> Command {
    let quoted = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
    let mut command = Command::new("osascript");

    command.arg("-e").arg(format!(
        "display notification {} with title {}",
        quoted(line),
        quoted(TITLE)
    ));

    command
}

/// The command that shows a desktop notification of `line`
#[cfg(not(target_os = "macos"))]
pub fn desktop_command(line: &str) -> Command {
    let mut command = Command::new("notify-send");

    command.arg(TITLE).arg(line);
    command
}

/// Sends alerts to their targets from a thread of its own
pub struct Notifier {
    sender: Mutex<Option<Sender<Alert>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Notifier {
    /// Starts the sending thread. `report` is told of each failure to
    /// deliver an alert.
    pub fn start(targets: Targets, report: impl Fn(&str) + Send + 'static) -> Notifier {
        let (sender, receiver) = mpsc::channel::<Alert>();

        let worker = thread::spawn(move || {
            for alert in receiver {
                if let Some((address, path)) = &targets.webhook {
                    if let Err(error) =
                        forward::post(address, path, "application/json", &alert.json())
                    {
                        report(&format!(
                            "Cannot send an alert to http://{}{}: {}",
                            address, path, error
                        ));
                    }
                }

                if targets.desktop {
                    match desktop_command(&alert.line).status() {
                        Ok(status) if status.success() => {}
                        Ok(status) => {
                            report(&format!("Cannot show a desktop notification: {}", status))
                        }
                        Err(error) => {
                            report(&format!("Cannot show a desktop notification: {}", error))
                        }
                    }
                }
            }
        });

        Notifier {
            sender: Mutex::new(Some(sender)),
            worker: Mutex::new(Some(worker)),
        }
    }

    /// Queues an alert for its targets
    pub fn send(&self, alert: Alert) {
        if let Some(sender) = self.sender.lock().unwrap().as_ref() {
            let _ = sender.send(alert);
        }
    }

    /// Waits until every queued alert has been sent or has failed
    pub fn finish(&self) {
        self.sender.lock().unwrap().take();

        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}
