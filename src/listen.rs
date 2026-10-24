//! Receiving log lines over the network
//!
//! A connection sends lines either as raw TCP, or as the bodies of HTTP POST
//! or PUT requests. Each connection is read on its own thread and sends whole
//! lines, so lines from different connections never mix.
//!
//! As a syslog receiver, splash takes messages over UDP, one per datagram
//! (RFC 5426), and over TCP, each framed by an octet count or a line break
//! (RFC 6587).
use crate::source;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::mpsc::{self, Receiver, SendError, Sender};
use std::thread;

/// The host a `--listen` address written as only `:PORT` binds to
pub const ALL_INTERFACES: &str = "0.0.0.0";

/// The address `--syslog` listens on when it names none: every interface,
/// on the port syslog messages are sent to
pub const SYSLOG_ADDRESS: &str = ":514";

/// The largest UDP datagram
const MAX_DATAGRAM: usize = 65535;

/// The methods whose request bodies hold log lines
const BODY_METHODS: [&str; 2] = ["POST", "PUT"];

const NO_CONTENT: &str = "204 No Content";
const BAD_REQUEST: &str = "400 Bad Request";
const METHOD_NOT_ALLOWED: &str = "405 Method Not Allowed";
const LENGTH_REQUIRED: &str = "411 Length Required";

/// A line received over the network, with the address of the host that
/// sent it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    pub sender: String,
    pub line: String,
}

/// Where the lines of one connection go, with the address that sent them
pub struct Lines<'a> {
    sender: &'a Sender<Received>,
    from: &'a str,
}

impl<'a> Lines<'a> {
    pub fn new(sender: &'a Sender<Received>, from: &'a str) -> Lines<'a> {
        Lines { sender, from }
    }

    fn send(&self, line: String) -> Result<(), SendError<Received>> {
        self.sender.send(Received {
            sender: self.from.to_string(),
            line,
        })
    }
}

/// The address of the host at the other end of a connection
fn peer(stream: &TcpStream) -> String {
    stream
        .peer_addr()
        .map(|address| address.ip().to_string())
        .unwrap_or_default()
}

/// The address to bind for a `--listen` address: `:PORT` names every
/// interface, and `HOST:PORT` is kept
pub fn bind_address(address: &str) -> String {
    match address.strip_prefix(':') {
        Some(port) => format!("{}:{}", ALL_INTERFACES, port),
        None => address.to_string(),
    }
}

/// Listens on `address`, returning the address bound, which names the port
/// chosen when `address` asks for port 0, and the lines every connection
/// sends
pub fn start(address: &str) -> io::Result<(SocketAddr, Receiver<Received>)> {
    let listener = TcpListener::bind(bind_address(address))?;
    let bound = listener.local_addr()?;
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let sender = sender.clone();

            thread::spawn(move || serve(&stream, &Lines::new(&sender, &peer(&stream))));
        }
    });

    Ok((bound, receiver))
}

/// Reads one connection, sending each line it holds. A connection whose
/// first line is an HTTP request line is answered as HTTP, and any other
/// connection is read as lines until it closes.
pub fn serve<S: Read + Write>(stream: S, lines: &Lines) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();

    let Some(first) = source::read_line(&mut reader, &mut bytes)? else {
        return Ok(());
    };

    match request_method(&first) {
        Some(method) => serve_http(&method, reader, lines),
        None if lines.send(first).is_err() => Ok(()),
        None => send_lines(reader, lines),
    }
}

/// The method of an HTTP request line such as `POST /logs HTTP/1.1`
fn request_method(line: &str) -> Option<String> {
    let mut parts = line.split(' ');

    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(method), Some(_), Some(version), None) if version.starts_with("HTTP/1.") => {
            Some(method.to_string())
        }
        _ => None,
    }
}

/// How a request says where its body ends
enum Body {
    Length(u64),
    Chunked,
    Missing,
}

/// Reads the rest of a request, sends the lines its body holds, and writes
/// the response
fn serve_http<S: Read + Write>(
    method: &str,
    mut reader: BufReader<S>,
    lines: &Lines,
) -> io::Result<()> {
    let status = match read_request(method, &mut reader, lines) {
        Ok(status) => status,
        Err(error) if error.kind() == io::ErrorKind::InvalidData => BAD_REQUEST,
        Err(error) => return Err(error),
    };

    let response = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");

    reader.get_mut().write_all(response.as_bytes())
}

/// Reads a request's headers and body, sending the lines the body holds,
/// and returns the response status
fn read_request<R: BufRead>(
    method: &str,
    reader: &mut R,
    lines: &Lines,
) -> io::Result<&'static str> {
    let body = read_headers(reader)?;

    if !BODY_METHODS.contains(&method) {
        return Ok(METHOD_NOT_ALLOWED);
    }

    match body {
        Body::Missing => return Ok(LENGTH_REQUIRED),
        Body::Length(length) => send_lines(reader.take(length), lines)?,
        Body::Chunked => send_lines(BufReader::new(Chunked::new(reader)), lines)?,
    }

    Ok(NO_CONTENT)
}

/// Reads header lines up to the blank line ending them
fn read_headers<R: BufRead>(reader: &mut R) -> io::Result<Body> {
    let mut bytes = Vec::new();
    let mut body = Body::Missing;

    while let Some(line) = source::read_line(reader, &mut bytes)? {
        if line.is_empty() {
            break;
        }

        let Some((name, value)) = line.split_once(':') else {
            continue;
        };

        let value = value.trim();

        if name.eq_ignore_ascii_case("transfer-encoding") && value.eq_ignore_ascii_case("chunked") {
            body = Body::Chunked;
        } else if name.eq_ignore_ascii_case("content-length") {
            match value.parse() {
                Ok(length) => body = Body::Length(length),
                Err(_) => return Err(invalid("Content-Length is not a number")),
            }
        }
    }

    Ok(body)
}

/// Sends each line a reader holds until it ends or nothing receives them
fn send_lines<R: BufRead>(mut reader: R, lines: &Lines) -> io::Result<()> {
    let mut bytes = Vec::new();

    while let Some(line) = source::read_line(&mut reader, &mut bytes)? {
        if lines.send(line).is_err() {
            break;
        }
    }

    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// The data of a body sent with chunked transfer encoding
struct Chunked<R> {
    inner: R,
    /// Bytes left in the current chunk
    remaining: u64,
    started: bool,
    finished: bool,
}

impl<R: BufRead> Chunked<R> {
    fn new(inner: R) -> Chunked<R> {
        Chunked {
            inner,
            remaining: 0,
            started: false,
            finished: false,
        }
    }

    /// Reads the size line that starts the next chunk, after the line break
    /// ending the chunk before it
    fn start_chunk(&mut self) -> io::Result<()> {
        let mut bytes = Vec::new();

        if self.started {
            source::read_line(&mut self.inner, &mut bytes)?;
        }

        self.started = true;

        let Some(line) = source::read_line(&mut self.inner, &mut bytes)? else {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        };

        let size = line.split(';').next().unwrap_or_default().trim();

        self.remaining = u64::from_str_radix(size, 16)
            .map_err(|_| invalid("chunk size is not a hexadecimal number"))?;
        self.finished = self.remaining == 0;

        Ok(())
    }
}

impl<R: BufRead> Read for Chunked<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 && !self.finished {
            self.start_chunk()?;
        }

        if self.finished {
            return Ok(0);
        }

        let limit = self.remaining.min(buffer.len() as u64) as usize;
        let count = self.inner.read(&mut buffer[..limit])?;

        if count == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }

        self.remaining -= count as u64;

        Ok(count)
    }
}

/// Receives syslog messages over TCP and UDP on the same address, returning
/// the address bound, which names the port chosen when `address` asks for
/// port 0, and the lines of every message
pub fn start_syslog(address: &str) -> io::Result<(SocketAddr, Receiver<Received>)> {
    let listener = TcpListener::bind(bind_address(address))?;
    let bound = listener.local_addr()?;
    let socket = UdpSocket::bind(bound)?;
    let (sender, receiver) = mpsc::channel();
    let datagrams = sender.clone();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let sender = sender.clone();

            thread::spawn(move || serve_syslog(&stream, &Lines::new(&sender, &peer(&stream))));
        }
    });

    thread::spawn(move || receive_datagrams(&socket, &datagrams));

    Ok((bound, receiver))
}

/// Sends the lines of each datagram the socket receives, until nothing
/// receives them
pub fn receive_datagrams(socket: &UdpSocket, sender: &Sender<Received>) {
    let mut buffer = vec![0u8; MAX_DATAGRAM];

    while let Ok((count, from)) = socket.recv_from(&mut buffer) {
        let from = from.ip().to_string();

        if send_message(&buffer[..count], &Lines::new(sender, &from)).is_err() {
            break;
        }
    }
}

/// Reads the syslog messages a TCP connection sends, sending each line they
/// hold. A frame that opens with a digit gives the length of the message
/// after a space, and any other frame ends at a line break.
pub fn serve_syslog<R: Read>(stream: R, lines: &Lines) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();

    while let Some(first) = reader.fill_buf()?.first().copied() {
        bytes.clear();

        if first.is_ascii_digit() {
            reader.read_until(b' ', &mut bytes)?;

            let length = std::str::from_utf8(&bytes)
                .ok()
                .and_then(|text| text.trim_end().parse::<usize>().ok())
                .ok_or_else(|| invalid("syslog frame length is not a number"))?;

            bytes.resize(length, 0);
            reader.read_exact(&mut bytes)?;
        } else {
            reader.read_until(b'\n', &mut bytes)?;
        }

        if send_message(&bytes, lines).is_err() {
            break;
        }
    }

    Ok(())
}

/// Sends each line of a message
fn send_message(bytes: &[u8], lines: &Lines) -> Result<(), SendError<Received>> {
    String::from_utf8_lossy(bytes)
        .lines()
        .try_for_each(|line| lines.send(line.to_string()))
}
