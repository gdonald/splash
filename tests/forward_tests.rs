use splash::forward::{Destination, ForwardError, Forwarder};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[test]
fn a_tcp_url_names_an_address() {
    assert_eq!(
        Destination::parse("tcp://127.0.0.1:5170"),
        Ok(Destination::Tcp("127.0.0.1:5170".to_string()))
    );
}

#[test]
fn a_udp_url_names_an_address() {
    assert_eq!(
        Destination::parse("udp://logs.example.com:514"),
        Ok(Destination::Udp("logs.example.com:514".to_string()))
    );
}

#[test]
fn an_http_url_names_an_address_and_a_path() {
    assert_eq!(
        Destination::parse("http://127.0.0.1:8080/ingest/web"),
        Ok(Destination::Http {
            address: "127.0.0.1:8080".to_string(),
            path: "/ingest/web".to_string()
        })
    );
}

#[test]
fn an_http_url_without_a_path_posts_to_the_root() {
    assert_eq!(
        Destination::parse("http://127.0.0.1:8080"),
        Ok(Destination::Http {
            address: "127.0.0.1:8080".to_string(),
            path: "/".to_string()
        })
    );
}

#[test]
fn malformed_urls_are_rejected() {
    for url in [
        "127.0.0.1:8080",
        "tcp://127.0.0.1",
        "tcp://127.0.0.1:",
        "tcp://127.0.0.1:5170/path",
        "udp://127.0.0.1:514/path",
    ] {
        assert_eq!(
            Destination::parse(url),
            Err(ForwardError::BadUrl(url.to_string())),
            "{}",
            url
        );
    }
}

#[test]
fn an_unsupported_scheme_is_rejected() {
    assert_eq!(
        Destination::parse("https://logs.example.com:443/"),
        Err(ForwardError::UnsupportedScheme("https".to_string()))
    );
}

#[test]
fn forward_errors_explain_what_is_expected() {
    assert_eq!(
        ForwardError::BadUrl("logs".to_string()).to_string(),
        "Invalid forward URL 'logs' (expected tcp://HOST:PORT, udp://HOST:PORT, or http://HOST:PORT/PATH)"
    );
    assert_eq!(
        ForwardError::UnsupportedScheme("https".to_string()).to_string(),
        "Cannot forward over https (expected tcp, udp, or http)"
    );
}

#[test]
fn destinations_display_as_their_urls() {
    for url in [
        "tcp://127.0.0.1:5170",
        "udp://127.0.0.1:514",
        "http://127.0.0.1:8080/ingest",
    ] {
        assert_eq!(Destination::parse(url).unwrap().to_string(), url);
    }
}

/// A forwarder whose failure reports are kept for the test to read
fn forwarder(destination: Destination) -> (Forwarder, Arc<Mutex<Vec<String>>>) {
    let reports = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&reports);

    let forwarder = Forwarder::start(destination, move |report| {
        kept.lock().unwrap().push(report.to_string())
    });

    (forwarder, reports)
}

#[test]
fn output_is_forwarded_over_one_tcp_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (forwarder, reports) = forwarder(Destination::Tcp(address));

    forwarder.send("service started\n");
    forwarder.send("request served\n");
    forwarder.finish();

    let mut received = String::new();
    listener
        .accept()
        .unwrap()
        .0
        .read_to_string(&mut received)
        .unwrap();

    assert_eq!(received, "service started\nrequest served\n");
    assert!(reports.lock().unwrap().is_empty());
}

#[test]
fn output_is_forwarded_over_udp_one_line_per_datagram() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let address = socket.local_addr().unwrap().to_string();
    let (forwarder, _) = forwarder(Destination::Udp(address));

    forwarder.send("service started\nrequest served\n");
    forwarder.finish();

    let mut buffer = [0u8; 1024];
    let mut datagrams = Vec::new();

    for _ in 0..2 {
        let count = socket.recv(&mut buffer).unwrap();
        datagrams.push(String::from_utf8_lossy(&buffer[..count]).to_string());
    }

    assert_eq!(datagrams, ["service started", "request served"]);
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

#[test]
fn output_is_posted_to_an_http_destination() {
    let (address, server) = http_server(vec!["HTTP/1.1 204 No Content\r\n\r\n"]);
    let (forwarder, reports) = forwarder(Destination::Http {
        address: address.clone(),
        path: "/ingest".to_string(),
    });

    forwarder.send("service started\n");
    forwarder.finish();

    assert_eq!(
        server.join().unwrap(),
        [format!(
            "POST /ingest HTTP/1.1\r\nHost: {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 16\r\nConnection: close\r\n\r\nservice started\n",
            address
        )]
    );
    assert!(reports.lock().unwrap().is_empty());
}

#[test]
fn a_failure_is_reported_once_until_a_delivery_succeeds() {
    let (address, server) = http_server(vec![
        "HTTP/1.1 503 Service Unavailable\r\n\r\n",
        "HTTP/1.1 503 Service Unavailable\r\n\r\n",
        "HTTP/1.1 200 OK\r\n\r\n",
        "garbage",
    ]);
    let (forwarder, reports) = forwarder(Destination::Http {
        address: address.clone(),
        path: "/".to_string(),
    });

    for line in ["one\n", "two\n", "three\n", "four\n"] {
        forwarder.send(line);
    }

    forwarder.finish();
    server.join().unwrap();

    assert_eq!(
        *reports.lock().unwrap(),
        [
            format!(
                "Cannot forward to http://{}/: the server answered 'HTTP/1.1 503 Service Unavailable'",
                address
            ),
            format!(
                "Cannot forward to http://{}/: the server answered 'garbage'",
                address
            ),
        ]
    );
}

#[test]
fn a_destination_that_refuses_connections_is_reported() {
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .to_string();
    let (forwarder, reports) = forwarder(Destination::Tcp(address.clone()));

    forwarder.send("service started\n");
    forwarder.finish();

    let reports = reports.lock().unwrap();

    assert_eq!(reports.len(), 1);
    assert!(reports[0].starts_with(&format!("Cannot forward to tcp://{}: ", address)));
}

#[test]
fn output_sent_after_finishing_is_dropped() {
    let (forwarder, reports) = forwarder(Destination::Udp("127.0.0.1:9".to_string()));

    forwarder.finish();
    forwarder.send("too late\n");
    forwarder.finish();

    assert!(reports.lock().unwrap().is_empty());
}
