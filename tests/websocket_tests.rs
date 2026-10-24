use splash::websocket::{self, Broadcaster};
use std::io::{self, Cursor, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

const HANDSHAKE: &[u8] = b"GET /logs HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n";

const SWITCHING_PROTOCOLS: &str = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";

const UPGRADE_REQUIRED: &str = "HTTP/1.1 426 Upgrade Required\r\nUpgrade: websocket\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

/// How a connection's writes go
#[derive(Clone, Copy, PartialEq)]
enum Writes {
    Succeed,
    FailAfterHandshake,
    Fail,
}

/// A connection that reads `input` and keeps what is written to it where
/// the test can see it after the broadcaster has taken the connection
struct Connection {
    input: Cursor<Vec<u8>>,
    written: Arc<Mutex<Vec<u8>>>,
    writes: Writes,
}

impl Connection {
    fn new(input: &[u8], writes: Writes) -> (Connection, Arc<Mutex<Vec<u8>>>) {
        let written = Arc::new(Mutex::new(Vec::new()));

        let connection = Connection {
            input: Cursor::new(input.to_vec()),
            written: Arc::clone(&written),
            writes,
        };

        (connection, written)
    }
}

impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.input.read(buffer)
    }
}

impl Write for Connection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut written = self.written.lock().unwrap();
        let handshake_done = written.ends_with(b"\r\n\r\n");

        match self.writes {
            Writes::Fail => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
            Writes::FailAfterHandshake if handshake_done => {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }
            _ => written.write(buffer),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn text(written: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&written.lock().unwrap()).to_string()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

#[test]
fn sha1_of_nothing_matches_the_published_digest() {
    assert_eq!(
        hex(&websocket::sha1(b"")),
        "da39a3ee5e6b4b0d3255bfef95601890afd80709"
    );
}

#[test]
fn sha1_of_abc_matches_the_published_digest() {
    assert_eq!(
        hex(&websocket::sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn sha1_of_a_message_needing_two_blocks_matches_the_published_digest() {
    assert_eq!(
        hex(&websocket::sha1(
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
        )),
        "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
    );
}

#[test]
fn base64_pads_each_partial_group() {
    assert_eq!(websocket::base64(b""), "");
    assert_eq!(websocket::base64(b"f"), "Zg==");
    assert_eq!(websocket::base64(b"fo"), "Zm8=");
    assert_eq!(websocket::base64(b"foo"), "Zm9v");
    assert_eq!(websocket::base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn the_accept_key_matches_the_rfc_6455_example() {
    assert_eq!(
        websocket::accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
        "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
    );
}

#[test]
fn a_short_text_frame_holds_its_length_in_one_byte() {
    assert_eq!(
        websocket::text_frame("Hello"),
        [0x81, 0x05, b'H', b'e', b'l', b'l', b'o']
    );
}

#[test]
fn a_text_frame_of_125_bytes_holds_its_length_in_one_byte() {
    let frame = websocket::text_frame(&"a".repeat(125));

    assert_eq!(frame[..2], [0x81, 125]);
    assert_eq!(frame.len(), 127);
}

#[test]
fn a_text_frame_of_126_bytes_holds_its_length_in_two_bytes() {
    let frame = websocket::text_frame(&"a".repeat(126));

    assert_eq!(frame[..4], [0x81, 126, 0, 126]);
    assert_eq!(frame.len(), 130);
}

#[test]
fn a_text_frame_over_65535_bytes_holds_its_length_in_eight_bytes() {
    let frame = websocket::text_frame(&"a".repeat(65536));

    assert_eq!(frame[..10], [0x81, 127, 0, 0, 0, 0, 0, 1, 0, 0]);
    assert_eq!(frame.len(), 65546);
}

#[test]
fn a_handshake_is_answered_and_the_connection_kept() {
    let broadcaster = Broadcaster::new();
    let (connection, written) = Connection::new(HANDSHAKE, Writes::Succeed);

    assert!(broadcaster.accept(connection).unwrap());
    assert_eq!(text(&written), SWITCHING_PROTOCOLS);
    assert_eq!(broadcaster.client_count(), 1);
}

#[test]
fn each_line_sent_reaches_a_client_as_a_text_message() {
    let broadcaster = Broadcaster::new();
    let (connection, written) = Connection::new(HANDSHAKE, Writes::Succeed);
    broadcaster.accept(connection).unwrap();

    broadcaster.send("alpha\nbeta\n");

    let messages = written.lock().unwrap()[SWITCHING_PROTOCOLS.len()..].to_vec();

    assert_eq!(
        messages,
        [
            websocket::text_frame("alpha"),
            websocket::text_frame("beta")
        ]
        .concat()
    );
}

#[test]
fn a_client_whose_connection_fails_is_dropped() {
    let broadcaster = Broadcaster::new();
    let (connection, _) = Connection::new(HANDSHAKE, Writes::FailAfterHandshake);
    broadcaster.accept(connection).unwrap();

    broadcaster.send("alpha\n");

    assert_eq!(broadcaster.client_count(), 0);
}

#[test]
fn a_request_that_does_not_ask_for_a_websocket_needs_an_upgrade() {
    let broadcaster = Broadcaster::new();
    let (connection, written) = Connection::new(
        b"GET / HTTP/1.1\r\nHost: localhost\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        Writes::Succeed,
    );

    assert!(!broadcaster.accept(connection).unwrap());
    assert_eq!(text(&written), UPGRADE_REQUIRED);
    assert_eq!(broadcaster.client_count(), 0);
}

#[test]
fn a_websocket_request_without_a_key_needs_an_upgrade() {
    let broadcaster = Broadcaster::new();
    let (connection, written) = Connection::new(
        b"GET / HTTP/1.1\r\nbroken header line\r\nUpgrade: websocket\r\n\r\n",
        Writes::Succeed,
    );

    assert!(!broadcaster.accept(connection).unwrap());
    assert_eq!(text(&written), UPGRADE_REQUIRED);
}

#[test]
fn a_connection_closed_before_its_request_ends_needs_an_upgrade() {
    let broadcaster = Broadcaster::new();
    let (connection, written) = Connection::new(b"GET / HTTP/1.1\r\n", Writes::Succeed);

    assert!(!broadcaster.accept(connection).unwrap());
    assert_eq!(text(&written), UPGRADE_REQUIRED);
}

#[test]
fn a_handshake_whose_answer_cannot_be_written_is_an_error() {
    let broadcaster = Broadcaster::new();
    let (connection, _) = Connection::new(HANDSHAKE, Writes::Fail);

    assert!(broadcaster.accept(connection).is_err());
    assert_eq!(broadcaster.client_count(), 0);
}

#[test]
fn a_started_broadcaster_sends_to_a_connected_browser() {
    let (address, broadcaster) = websocket::start("127.0.0.1:0").unwrap();

    let mut browser = TcpStream::connect(address).unwrap();
    browser.write_all(HANDSHAKE).unwrap();

    let mut response = vec![0u8; SWITCHING_PROTOCOLS.len()];
    browser.read_exact(&mut response).unwrap();

    broadcaster.send("alpha\n");

    let mut message = [0u8; 7];
    browser.read_exact(&mut message).unwrap();

    assert_eq!(String::from_utf8_lossy(&response), SWITCHING_PROTOCOLS);
    assert_eq!(message.to_vec(), websocket::text_frame("alpha"));
}

#[test]
fn an_address_that_cannot_be_bound_is_an_error() {
    assert!(websocket::start("not an address").is_err());
}
