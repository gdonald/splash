//! Sending splash's output to browsers over WebSocket (RFC 6455)
//!
//! Each line splash writes is sent to every connected client as a text
//! message. Clients only receive. Frames they send are never read, and a
//! client whose connection fails is dropped.
use crate::listen;
use crate::source;
use std::io::{self, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Appended to a client's key before hashing it, as RFC 6455 section 1.3 sets
const HANDSHAKE_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// How long a send waits on a client that has stopped reading before
/// dropping it
pub const SEND_TIMEOUT: Duration = Duration::from_secs(1);

/// The first byte of an unfragmented text frame
const TEXT_FRAME: u8 = 0x81;

/// Payload length markers for 16-bit and 64-bit extended lengths
const LENGTH_16: u8 = 126;
const LENGTH_64: u8 = 127;

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The connected clients
#[derive(Default)]
pub struct Broadcaster {
    clients: Mutex<Vec<Box<dyn Write + Send>>>,
}

impl Broadcaster {
    pub fn new() -> Broadcaster {
        Broadcaster::default()
    }

    /// Reads a connection's opening request. A WebSocket handshake is
    /// answered and the connection kept as a client. Any other request is
    /// answered with `426 Upgrade Required`. Returns whether the connection
    /// became a client.
    pub fn accept<S: Read + Write + Send + 'static>(&self, mut stream: S) -> io::Result<bool> {
        let key = read_handshake(&mut BufReader::new(&mut stream))?;

        let Some(key) = key else {
            stream.write_all(
                b"HTTP/1.1 426 Upgrade Required\r\nUpgrade: websocket\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )?;

            return Ok(false);
        };

        // Holding the lock while answering keeps a message from reaching the
        // client before the handshake response does
        let mut clients = self.clients.lock().unwrap();

        write!(
            stream,
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            accept_key(&key)
        )?;
        clients.push(Box::new(stream));

        Ok(true)
    }

    /// Sends each line of `text` to every client as a text message
    pub fn send(&self, text: &str) {
        let mut clients = self.clients.lock().unwrap();

        for line in text.lines() {
            let frame = text_frame(line);

            clients.retain_mut(|client| client.write_all(&frame).is_ok());
        }
    }

    /// The number of connected clients
    pub fn client_count(&self) -> usize {
        self.clients.lock().unwrap().len()
    }
}

/// Listens on `address` for WebSocket clients, returning the address bound
/// and the broadcaster that sends to them
pub fn start(address: &str) -> io::Result<(SocketAddr, Arc<Broadcaster>)> {
    let listener = TcpListener::bind(listen::bind_address(address))?;
    let bound = listener.local_addr()?;
    let broadcaster = Arc::new(Broadcaster::new());
    let accepting = Arc::clone(&broadcaster);

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let broadcaster = Arc::clone(&accepting);

            thread::spawn(move || {
                let _ = stream.set_write_timeout(Some(SEND_TIMEOUT));
                let _ = broadcaster.accept(stream);
            });
        }
    });

    Ok((bound, broadcaster))
}

/// Reads an opening request, returning its `Sec-WebSocket-Key` when it asks
/// to upgrade to a WebSocket
fn read_handshake<R: io::BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let mut upgrade = false;
    let mut key = None;

    while let Some(line) = source::read_line(reader, &mut bytes)? {
        if line.is_empty() {
            break;
        }

        let Some((name, value)) = line.split_once(':') else {
            continue;
        };

        let value = value.trim();

        if name.eq_ignore_ascii_case("upgrade") {
            upgrade = value.eq_ignore_ascii_case("websocket");
        } else if name.eq_ignore_ascii_case("sec-websocket-key") {
            key = Some(value.to_string());
        }
    }

    Ok(key.filter(|_| upgrade))
}

/// The `Sec-WebSocket-Accept` value answering a client's key
pub fn accept_key(key: &str) -> String {
    base64(&sha1(format!("{}{}", key, HANDSHAKE_GUID).as_bytes()))
}

/// An unmasked text frame holding `text`, as a server sends one
pub fn text_frame(text: &str) -> Vec<u8> {
    let payload = text.as_bytes();
    let mut frame = vec![TEXT_FRAME];

    match payload.len() {
        length if length < LENGTH_16 as usize => frame.push(length as u8),
        length if length <= u16::MAX as usize => {
            frame.push(LENGTH_16);
            frame.extend((length as u16).to_be_bytes());
        }
        length => {
            frame.push(LENGTH_64);
            frame.extend((length as u64).to_be_bytes());
        }
    }

    frame.extend(payload);
    frame
}

/// The SHA-1 digest of `data` (RFC 3174), which the WebSocket handshake
/// requires
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    let mut message = data.to_vec();
    message.push(0x80);

    while message.len() % 64 != 56 {
        message.push(0);
    }

    message.extend((data.len() as u64 * 8).to_be_bytes());

    for block in message.chunks(64) {
        let mut words = [0u32; 80];

        for (index, word) in block.chunks(4).enumerate() {
            words[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }

        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = state;

        for (index, word) in words.iter().enumerate() {
            let (mixed, constant) = match index {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };

            let next = a
                .rotate_left(5)
                .wrapping_add(mixed)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);

            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = next;
        }

        for (value, added) in state.iter_mut().zip([a, b, c, d, e]) {
            *value = value.wrapping_add(added);
        }
    }

    let mut digest = [0u8; 20];

    for (bytes, value) in digest.chunks_mut(4).zip(state) {
        bytes.copy_from_slice(&value.to_be_bytes());
    }

    digest
}

/// `data` in standard base64 with padding (RFC 4648)
pub fn base64(data: &[u8]) -> String {
    let mut encoded = String::new();

    for group in data.chunks(3) {
        let bytes = [
            group[0],
            *group.get(1).unwrap_or(&0),
            *group.get(2).unwrap_or(&0),
        ];
        let bits = u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]);

        for position in 0..4 {
            if position <= group.len() {
                let index = (bits >> (18 - 6 * position)) & 0x3F;
                encoded.push(BASE64_ALPHABET[index as usize] as char);
            } else {
                encoded.push('=');
            }
        }
    }

    encoded
}
