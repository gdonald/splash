use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

/// What the server answers once the given responses are used up
const NO_EVENTS: &str = r#"{"events":[],"searchedLogStreams":[]}"#;

/// A CloudWatch Logs endpoint on a port the system chose, answering each
/// request with the next of its responses and then with no events
pub struct FakeCloudWatch {
    pub endpoint: String,
    requests: Receiver<String>,
}

impl FakeCloudWatch {
    /// Answers with each JSON body as a 200 response
    pub fn start(bodies: &[&str]) -> FakeCloudWatch {
        let responses = bodies.iter().map(|body| (200, body.to_string())).collect();

        FakeCloudWatch::answering(responses)
    }

    /// Answers with each status and JSON body
    pub fn answering(responses: Vec<(u16, String)>) -> FakeCloudWatch {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let responses = Arc::new(Mutex::new(responses.into_iter()));
        let (sender, requests) = mpsc::channel();

        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let responses = Arc::clone(&responses);
                let sender = sender.clone();

                thread::spawn(move || serve(stream, &responses, &sender));
            }
        });

        FakeCloudWatch { endpoint, requests }
    }

    /// The body of each request received so far
    pub fn requests(&self) -> Vec<String> {
        self.requests.try_iter().collect()
    }
}

fn serve(
    stream: TcpStream,
    responses: &Mutex<std::vec::IntoIter<(u16, String)>>,
    requests: &Sender<String>,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = stream;

    while let Some(body) = read_request(&mut reader) {
        let _ = requests.send(body);

        let (status, body) = responses
            .lock()
            .unwrap()
            .next()
            .unwrap_or((200, NO_EVENTS.to_string()));

        let response = format!(
            "HTTP/1.1 {} Answer\r\nContent-Type: application/x-amz-json-1.1\r\nContent-Length: {}\r\n\r\n{}",
            status,
            body.len(),
            body
        );

        if writer.write_all(response.as_bytes()).is_err() {
            return;
        }
    }
}

/// The body of the next request on the connection, or `None` once it closes
fn read_request(reader: &mut BufReader<TcpStream>) -> Option<String> {
    let mut length = 0;

    loop {
        let mut line = String::new();

        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }

        let line = line.trim_end();

        if line.is_empty() {
            break;
        }

        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap();
            }
        }
    }

    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;

    Some(String::from_utf8(body).unwrap())
}
