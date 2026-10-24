//! Forwarding output to other systems
//!
//! Everything splash writes is also sent to one destination: a TCP
//! connection, UDP datagrams of one line each, or HTTP POST requests. A
//! thread of its own does the sending, so a slow destination does not hold
//! up the output, and splash waits for it before exiting.
use std::fmt;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};

/// Where forwarded output goes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// `tcp://HOST:PORT`, one connection that is opened again after it fails
    Tcp(String),
    /// `udp://HOST:PORT`, one datagram per line
    Udp(String),
    /// `http://HOST:PORT/PATH`, one POST request per chunk of output
    Http { address: String, path: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardError {
    BadUrl(String),
    UnsupportedScheme(String),
}

impl fmt::Display for ForwardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForwardError::BadUrl(url) => write!(
                f,
                "Invalid forward URL '{}' (expected tcp://HOST:PORT, udp://HOST:PORT, or http://HOST:PORT/PATH)",
                url
            ),
            ForwardError::UnsupportedScheme(scheme) => write!(
                f,
                "Cannot forward over {} (expected tcp, udp, or http)",
                scheme
            ),
        }
    }
}

impl std::error::Error for ForwardError {}

impl Destination {
    pub fn parse(url: &str) -> Result<Destination, ForwardError> {
        let bad = || ForwardError::BadUrl(url.to_string());
        let (scheme, rest) = url.split_once("://").ok_or_else(bad)?;

        let (address, path) = match rest.find('/') {
            Some(slash) => (&rest[..slash], &rest[slash..]),
            None => (rest, "/"),
        };

        if !address.contains(':') || address.ends_with(':') {
            return Err(bad());
        }

        match scheme {
            "tcp" if path == "/" => Ok(Destination::Tcp(address.to_string())),
            "udp" if path == "/" => Ok(Destination::Udp(address.to_string())),
            "tcp" | "udp" => Err(bad()),
            "http" => Ok(Destination::Http {
                address: address.to_string(),
                path: path.to_string(),
            }),
            _ => Err(ForwardError::UnsupportedScheme(scheme.to_string())),
        }
    }
}

impl fmt::Display for Destination {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Destination::Tcp(address) => write!(f, "tcp://{}", address),
            Destination::Udp(address) => write!(f, "udp://{}", address),
            Destination::Http { address, path } => write!(f, "http://{}{}", address, path),
        }
    }
}

/// Sends output to a destination from a thread of its own
pub struct Forwarder {
    sender: Mutex<Option<Sender<String>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Forwarder {
    /// Starts the sending thread. `report` is told of a failure to deliver,
    /// once until a delivery succeeds again.
    pub fn start(destination: Destination, report: impl Fn(&str) + Send + 'static) -> Forwarder {
        let (sender, receiver) = mpsc::channel::<String>();

        let worker = thread::spawn(move || {
            let mut connection = Connection::new(destination);
            let mut failing = false;

            for text in receiver {
                match connection.deliver(&text) {
                    Ok(()) => failing = false,
                    Err(error) if !failing => {
                        failing = true;
                        report(&format!(
                            "Cannot forward to {}: {}",
                            connection.destination, error
                        ));
                    }
                    Err(_) => {}
                }
            }
        });

        Forwarder {
            sender: Mutex::new(Some(sender)),
            worker: Mutex::new(Some(worker)),
        }
    }

    /// Queues text for the destination
    pub fn send(&self, text: &str) {
        if let Some(sender) = self.sender.lock().unwrap().as_ref() {
            let _ = sender.send(text.to_string());
        }
    }

    /// Waits until everything queued has been sent or has failed
    pub fn finish(&self) {
        self.sender.lock().unwrap().take();

        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

struct Connection {
    destination: Destination,
    stream: Option<TcpStream>,
}

impl Connection {
    fn new(destination: Destination) -> Connection {
        Connection {
            destination,
            stream: None,
        }
    }

    fn deliver(&mut self, text: &str) -> io::Result<()> {
        match &self.destination {
            Destination::Tcp(address) => {
                let stream = match self.stream.take() {
                    Some(stream) => stream,
                    None => TcpStream::connect(address)?,
                };

                (&stream).write_all(text.as_bytes())?;
                self.stream = Some(stream);

                Ok(())
            }
            Destination::Udp(address) => {
                let socket = UdpSocket::bind("0.0.0.0:0")?;

                for line in text.lines() {
                    socket.send_to(line.as_bytes(), address)?;
                }

                Ok(())
            }
            Destination::Http { address, path } => post(address, path, text),
        }
    }
}

/// Posts `body` and fails unless the response status is 2xx
fn post(address: &str, path: &str, body: &str) -> io::Result<()> {
    let mut stream = TcpStream::connect(address)?;

    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;

    let mut status_line = String::new();
    BufReader::new(&stream).read_line(&mut status_line)?;

    let status = status_line.trim_end();

    match status.split(' ').nth(1) {
        Some(code) if code.starts_with('2') => Ok(()),
        _ => Err(io::Error::other(format!(
            "the server answered '{}'",
            status
        ))),
    }
}
