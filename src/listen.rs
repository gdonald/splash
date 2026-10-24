//! Receiving log lines over the network
//!
//! A connection sends lines either as raw TCP, or as the bodies of HTTP POST
//! or PUT requests. Each connection is read on its own thread and sends whole
//! lines, so lines from different connections never mix.
use crate::source;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

/// The host a `--listen` address written as only `:PORT` binds to
pub const ALL_INTERFACES: &str = "0.0.0.0";

/// The methods whose request bodies hold log lines
const BODY_METHODS: [&str; 2] = ["POST", "PUT"];

const NO_CONTENT: &str = "204 No Content";
const BAD_REQUEST: &str = "400 Bad Request";
const METHOD_NOT_ALLOWED: &str = "405 Method Not Allowed";
const LENGTH_REQUIRED: &str = "411 Length Required";

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
pub fn start(address: &str) -> io::Result<(SocketAddr, Receiver<String>)> {
    let listener = TcpListener::bind(bind_address(address))?;
    let bound = listener.local_addr()?;
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let sender = sender.clone();

            thread::spawn(move || serve(&stream, &sender));
        }
    });

    Ok((bound, receiver))
}

/// Reads one connection, sending each line it holds. A connection whose
/// first line is an HTTP request line is answered as HTTP, and any other
/// connection is read as lines until it closes.
pub fn serve<S: Read + Write>(stream: S, sender: &Sender<String>) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();

    let Some(first) = source::read_line(&mut reader, &mut bytes)? else {
        return Ok(());
    };

    match request_method(&first) {
        Some(method) => serve_http(&method, reader, sender),
        None if sender.send(first).is_err() => Ok(()),
        None => send_lines(reader, sender),
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
    sender: &Sender<String>,
) -> io::Result<()> {
    let status = match read_request(method, &mut reader, sender) {
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
    sender: &Sender<String>,
) -> io::Result<&'static str> {
    let body = read_headers(reader)?;

    if !BODY_METHODS.contains(&method) {
        return Ok(METHOD_NOT_ALLOWED);
    }

    match body {
        Body::Missing => return Ok(LENGTH_REQUIRED),
        Body::Length(length) => send_lines(reader.take(length), sender)?,
        Body::Chunked => send_lines(BufReader::new(Chunked::new(reader)), sender)?,
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
fn send_lines<R: BufRead>(mut reader: R, sender: &Sender<String>) -> io::Result<()> {
    let mut bytes = Vec::new();

    while let Some(line) = source::read_line(&mut reader, &mut bytes)? {
        if sender.send(line).is_err() {
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
