use splash::alert::{self, Alert, AlertError, Notifier, Patterns, Targets};
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

fn patterns(patterns: &[&str]) -> Patterns {
    let patterns: Vec<String> = patterns.iter().map(|pattern| pattern.to_string()).collect();

    Patterns::build(&patterns).unwrap()
}

fn alert(pattern: &str, line: &str) -> Alert {
    Alert {
        pattern: pattern.to_string(),
        line: line.to_string(),
    }
}

#[test]
fn test_each_line_matching_a_pattern_raises_an_alert_naming_the_first_pattern_it_matches() {
    let patterns = patterns(&["ERROR", "disk (full|failing)"]);

    assert_eq!(
        patterns.alerts("INFO started\nERROR disk full\nWARN disk failing\n"),
        vec![
            alert("ERROR", "ERROR disk full"),
            alert("disk (full|failing)", "WARN disk failing"),
        ]
    );
}

#[test]
fn test_an_alert_pattern_that_is_not_a_regular_expression_is_refused() {
    let error = Patterns::build(&["disk (full".to_string()]).unwrap_err();

    assert!(matches!(&error, AlertError::BadPattern { pattern, .. } if pattern == "disk (full"));
    assert!(error
        .to_string()
        .starts_with("Invalid alert pattern 'disk (full': "));
}

#[test]
fn test_an_alert_is_posted_as_a_json_object() {
    assert_eq!(
        alert("ERROR", "ERROR \"disk\" full\tnow").json(),
        r#"{"pattern":"ERROR","line":"ERROR \"disk\" full\tnow"}"#
    );
}

#[test]
fn test_a_webhook_is_an_http_url() {
    assert_eq!(
        Targets::new(Some("http://127.0.0.1:9000/hooks/logs"), true),
        Ok(Targets {
            webhook: Some(("127.0.0.1:9000".to_string(), "/hooks/logs".to_string())),
            desktop: true
        })
    );
    assert_eq!(
        Targets::new(None, true),
        Ok(Targets {
            webhook: None,
            desktop: true
        })
    );
}

#[test]
fn test_a_webhook_that_is_not_an_http_url_is_refused() {
    for url in ["tcp://127.0.0.1:9000", "hooks"] {
        let error = Targets::new(Some(url), false).unwrap_err();

        assert_eq!(
            (error.clone(), error.to_string()),
            (
                AlertError::BadWebhook(url.to_string()),
                format!(
                    "Invalid webhook URL '{}' (expected http://HOST:PORT/PATH)",
                    url
                )
            )
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn test_a_desktop_notification_is_shown_by_osascript() {
    let command = alert::desktop_command(r#"ERROR "disk" C:\temp"#);

    assert_eq!(command.get_program(), "osascript");
    assert_eq!(
        command.get_args().collect::<Vec<&OsStr>>(),
        [
            "-e",
            r#"display notification "ERROR \"disk\" C:\\temp" with title "splash""#
        ]
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn test_a_desktop_notification_is_shown_by_notify_send() {
    let command = alert::desktop_command("ERROR disk full");

    assert_eq!(command.get_program(), "notify-send");
    assert_eq!(
        command.get_args().collect::<Vec<&OsStr>>(),
        ["splash", "ERROR disk full"]
    );
}

/// An HTTP server that answers each request with the next of `statuses`,
/// returning the requests it read
fn http_server(statuses: Vec<&'static str>) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();

    let server = thread::spawn(move || {
        statuses
            .into_iter()
            .map(|status| {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&stream);
                let mut request = String::new();
                let mut length = 0;

                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    request.push_str(&line);

                    if let Some(value) = line.strip_prefix("Content-Length: ") {
                        length = value.trim().parse().unwrap();
                    }

                    if line == "\r\n" {
                        break;
                    }
                }

                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).unwrap();
                request.push_str(&String::from_utf8_lossy(&body));

                (&stream).write_all(status.as_bytes()).unwrap();

                request
            })
            .collect()
    });

    (address, server)
}

fn notifier(webhook: &str) -> (Notifier, Arc<Mutex<Vec<String>>>) {
    let reports = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&reports);
    let targets = Targets::new(Some(webhook), false).unwrap();

    let notifier = Notifier::start(targets, move |report| {
        kept.lock().unwrap().push(report.to_string())
    });

    (notifier, reports)
}

#[test]
fn test_each_alert_is_posted_to_the_webhook() {
    let (address, server) = http_server(vec!["HTTP/1.1 204 No Content\r\n\r\n"]);
    let (notifier, reports) = notifier(&format!("http://{}/hooks", address));

    notifier.send(alert("ERROR", "ERROR disk full"));
    notifier.finish();

    assert_eq!(
        server.join().unwrap(),
        [format!(
            "POST /hooks HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: 44\r\nConnection: close\r\n\r\n{{\"pattern\":\"ERROR\",\"line\":\"ERROR disk full\"}}",
            address
        )]
    );
    assert!(reports.lock().unwrap().is_empty());
}

#[test]
fn test_each_alert_the_webhook_refuses_is_reported() {
    let (address, server) = http_server(vec![
        "HTTP/1.1 500 Internal Server Error\r\n\r\n",
        "HTTP/1.1 500 Internal Server Error\r\n\r\n",
    ]);
    let (notifier, reports) = notifier(&format!("http://{}/", address));

    notifier.send(alert("ERROR", "ERROR one"));
    notifier.send(alert("ERROR", "ERROR two"));
    notifier.finish();
    server.join().unwrap();

    let report = format!(
        "Cannot send an alert to http://{}/: the server answered 'HTTP/1.1 500 Internal Server Error'",
        address
    );

    assert_eq!(*reports.lock().unwrap(), [report.clone(), report]);
}

#[test]
fn test_an_alert_sent_after_finishing_is_dropped() {
    let (address, server) = http_server(vec![]);
    let (notifier, reports) = notifier(&format!("http://{}/", address));

    notifier.finish();
    notifier.send(alert("ERROR", "ERROR disk full"));

    assert!(server.join().unwrap().is_empty());
    assert!(reports.lock().unwrap().is_empty());
}
