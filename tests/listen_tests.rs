use splash::listen::{self, Lines, Received, ALL_INTERFACES};
use std::io::{self, Cursor, Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::mpsc;
use std::time::Duration;

/// The address the in-memory connections are sent from
const SENDER: &str = "10.0.0.5";

/// A connection that reads `input` and keeps what is written to it
struct Connection {
    input: Cursor<Vec<u8>>,
    written: Vec<u8>,
}

impl Connection {
    fn new(input: &[u8]) -> Connection {
        Connection {
            input: Cursor::new(input.to_vec()),
            written: Vec::new(),
        }
    }
}

impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.input.read(buffer)
    }
}

impl Write for Connection {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.written.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// What serving a connection that sends `input` returns, sends, and writes
struct Served {
    result: io::Result<()>,
    lines: Vec<String>,
    response: String,
}

fn serve(input: &[u8]) -> Served {
    let mut connection = Connection::new(input);
    let (sender, receiver) = mpsc::channel();

    let result = listen::serve(&mut connection, &Lines::new(&sender, SENDER));

    drop(sender);

    Served {
        result,
        lines: receiver.iter().map(|received| received.line).collect(),
        response: String::from_utf8_lossy(&connection.written).to_string(),
    }
}

fn response_with_status(status: &str) -> String {
    format!(
        "HTTP/1.1 {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        status
    )
}

#[test]
fn a_port_alone_binds_every_interface() {
    assert_eq!(
        listen::bind_address(":8080"),
        format!("{}:8080", ALL_INTERFACES)
    );
}

#[test]
fn a_host_and_port_are_bound_as_given() {
    assert_eq!(listen::bind_address("127.0.0.1:8080"), "127.0.0.1:8080");
}

#[test]
fn raw_tcp_lines_are_sent_without_their_line_endings() {
    let served = serve(b"service started\r\nrequest served\nshutting down");

    assert!(served.result.is_ok());
    assert_eq!(
        served.lines,
        ["service started", "request served", "shutting down"]
    );
    assert_eq!(served.response, "");
}

#[test]
fn an_empty_connection_sends_nothing() {
    let served = serve(b"");

    assert!(served.result.is_ok());
    assert!(served.lines.is_empty());
}

#[test]
fn a_raw_connection_stops_when_nothing_receives_its_lines() {
    let mut connection = Connection::new(b"service started\nrequest served\n");
    let (sender, receiver) = mpsc::channel();
    drop(receiver);

    assert!(listen::serve(&mut connection, &Lines::new(&sender, SENDER)).is_ok());
}

/// A connection that sends the same line forever
struct EndlessLines;

impl Read for EndlessLines {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let line = b"request served\n";
        let count = buffer.len().min(line.len());

        buffer[..count].copy_from_slice(&line[..count]);

        Ok(count)
    }
}

impl Write for EndlessLines {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_raw_connection_stops_when_its_lines_stop_being_received() {
    let (sender, receiver) = mpsc::channel();

    let receive_one = std::thread::spawn(move || receiver.recv().unwrap());

    assert!(listen::serve(EndlessLines, &Lines::new(&sender, SENDER)).is_ok());
    assert_eq!(
        receive_one.join().unwrap(),
        Received {
            sender: SENDER.to_string(),
            line: "request served".to_string()
        }
    );
}

#[test]
fn a_post_body_is_sent_as_lines() {
    let served = serve(
        b"POST /logs HTTP/1.1\r\nHost: localhost\r\nContent-Length: 30\r\n\r\nservice started\nrequest served",
    );

    assert!(served.result.is_ok());
    assert_eq!(served.lines, ["service started", "request served"]);
    assert_eq!(served.response, response_with_status("204 No Content"));
}

#[test]
fn a_put_body_is_sent_as_lines() {
    let served = serve(b"PUT /logs HTTP/1.0\r\ncontent-length: 16\r\n\r\nservice started\n");

    assert_eq!(served.lines, ["service started"]);
    assert_eq!(served.response, response_with_status("204 No Content"));
}

#[test]
fn bytes_after_the_content_length_are_not_sent() {
    let served = serve(b"POST / HTTP/1.1\r\nContent-Length: 6\r\n\r\nalpha\nbeta\n");

    assert_eq!(served.lines, ["alpha"]);
}

#[test]
fn header_lines_without_a_colon_are_skipped() {
    let served = serve(b"POST / HTTP/1.1\r\nnot a header\r\nContent-Length: 6\r\n\r\nalpha\n");

    assert_eq!(served.lines, ["alpha"]);
}

#[test]
fn a_get_request_is_refused() {
    let served = serve(b"GET /logs HTTP/1.1\r\nHost: localhost\r\n\r\n");

    assert!(served.lines.is_empty());
    assert_eq!(
        served.response,
        response_with_status("405 Method Not Allowed")
    );
}

#[test]
fn a_post_without_a_length_is_refused() {
    let served = serve(b"POST /logs HTTP/1.1\r\nHost: localhost\r\n\r\nservice started\n");

    assert!(served.lines.is_empty());
    assert_eq!(served.response, response_with_status("411 Length Required"));
}

#[test]
fn a_content_length_that_is_not_a_number_is_a_bad_request() {
    let served = serve(b"POST /logs HTTP/1.1\r\nContent-Length: many\r\n\r\nservice started\n");

    assert!(served.lines.is_empty());
    assert_eq!(served.response, response_with_status("400 Bad Request"));
}

#[test]
fn a_chunked_body_is_sent_as_lines_across_chunk_boundaries() {
    let served = serve(
        b"POST /logs HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n8\r\nservice \r\n10;name=value\r\nstarted\nrequest \r\n7\r\nserved\n\r\n0\r\n\r\n",
    );

    assert!(served.result.is_ok());
    assert_eq!(served.lines, ["service started", "request served"]);
    assert_eq!(served.response, response_with_status("204 No Content"));
}

#[test]
fn a_chunk_size_that_is_not_hexadecimal_is_a_bad_request() {
    let served = serve(
        b"POST /logs HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n6\r\nalpha\n\r\nzz\r\nbeta\n\r\n0\r\n\r\n",
    );

    assert_eq!(served.lines, ["alpha"]);
    assert_eq!(served.response, response_with_status("400 Bad Request"));
}

#[test]
fn a_chunked_body_cut_off_inside_a_chunk_is_an_error() {
    let served = serve(b"POST /logs HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n10\r\nalpha\n");

    assert_eq!(
        served.result.unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
    assert_eq!(served.response, "");
}

#[test]
fn a_chunked_body_cut_off_before_a_chunk_size_is_an_error() {
    let served =
        serve(b"POST /logs HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n6\r\nalpha\n\r\n");

    assert_eq!(
        served.result.unwrap_err().kind(),
        io::ErrorKind::UnexpectedEof
    );
}

#[test]
fn a_request_line_needs_an_http_version() {
    let served = serve(b"POST /logs SPDY/3\nservice started\n");

    assert_eq!(served.lines, ["POST /logs SPDY/3", "service started"]);
}

#[test]
fn a_request_line_has_three_parts() {
    let served = serve(b"POST /logs HTTP/1.1 extra\n");

    assert_eq!(served.lines, ["POST /logs HTTP/1.1 extra"]);
}

#[test]
fn a_listener_receives_lines_from_a_connection() {
    let (address, lines) = listen::start("127.0.0.1:0").unwrap();

    let mut connection = TcpStream::connect(address).unwrap();
    connection
        .write_all(b"service started\nrequest served\n")
        .unwrap();
    drop(connection);

    let wait = Duration::from_secs(10);

    let first = lines.recv_timeout(wait).unwrap();
    let second = lines.recv_timeout(wait).unwrap();

    assert_eq!(first.line, "service started");
    assert_eq!(second.line, "request served");
    assert_eq!(first.sender, "127.0.0.1");
}

#[test]
fn a_listener_answers_http_requests() {
    let (address, lines) = listen::start("127.0.0.1:0").unwrap();

    let mut connection = TcpStream::connect(address).unwrap();
    connection
        .write_all(b"POST /logs HTTP/1.1\r\nContent-Length: 16\r\n\r\nservice started\n")
        .unwrap();

    let mut response = String::new();
    connection.read_to_string(&mut response).unwrap();

    assert_eq!(response, response_with_status("204 No Content"));
    assert_eq!(
        lines.recv_timeout(Duration::from_secs(10)).unwrap().line,
        "service started"
    );
}

#[test]
fn an_address_that_cannot_be_bound_is_an_error() {
    assert!(listen::start("not an address").is_err());
}

const RFC5424: &str = "<34>1 2003-10-11T22:14:15.003Z mymachine.example.com su - ID47 - 'su root' failed for lonvick on /dev/pts/8";

const RFC3164: &str = "<13>Oct  3 12:00:01 web01 sshd[4101]: Accepted publickey for alice from 10.0.0.5 port 52144 ssh2";

/// What reading a syslog connection that sends `input` returns and sends
fn serve_syslog(input: &[u8]) -> (io::Result<()>, Vec<String>) {
    let (sender, receiver) = mpsc::channel();

    let result = listen::serve_syslog(input, &Lines::new(&sender, SENDER));

    drop(sender);

    (
        result,
        receiver.iter().map(|received| received.line).collect(),
    )
}

fn octet_counted(message: &str) -> String {
    format!("{} {}", message.len(), message)
}

#[test]
fn octet_counted_syslog_frames_are_read() {
    let input = format!("{}{}", octet_counted(RFC5424), octet_counted(RFC3164));

    let (result, lines) = serve_syslog(input.as_bytes());

    assert!(result.is_ok());
    assert_eq!(lines, [RFC5424, RFC3164]);
}

#[test]
fn syslog_frames_ending_at_line_breaks_are_read() {
    let input = format!("{}\r\n{}", RFC3164, RFC5424);

    let (_, lines) = serve_syslog(input.as_bytes());

    assert_eq!(lines, [RFC3164, RFC5424]);
}

#[test]
fn both_framings_can_share_a_connection() {
    let input = format!("{}\n{}", RFC3164, octet_counted(RFC5424));

    let (_, lines) = serve_syslog(input.as_bytes());

    assert_eq!(lines, [RFC3164, RFC5424]);
}

#[test]
fn a_syslog_message_holding_several_lines_sends_each_one() {
    let input = octet_counted("<11>app: panic: index out of range\ngoroutine 1 [running]:\n");

    let (_, lines) = serve_syslog(input.as_bytes());

    assert_eq!(
        lines,
        [
            "<11>app: panic: index out of range",
            "goroutine 1 [running]:"
        ]
    );
}

#[test]
fn a_syslog_frame_length_that_is_not_a_number_is_an_error() {
    let (result, lines) = serve_syslog(b"12x <13>web01 app: ready");

    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert!(lines.is_empty());
}

#[test]
fn a_syslog_frame_cut_off_before_its_length_is_an_error() {
    let (result, _) = serve_syslog(b"80 <13>web01 app: ready");

    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn an_empty_syslog_connection_sends_nothing() {
    let (result, lines) = serve_syslog(b"");

    assert!(result.is_ok());
    assert!(lines.is_empty());
}

#[test]
fn a_syslog_connection_stops_when_nothing_receives_its_lines() {
    let (sender, receiver) = mpsc::channel();
    drop(receiver);

    let input = format!("{}\n{}\n", RFC3164, RFC3164);

    assert!(listen::serve_syslog(input.as_bytes(), &Lines::new(&sender, SENDER)).is_ok());
}

#[test]
fn datagrams_stop_being_read_when_nothing_receives_their_lines() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (sender, receiver) = mpsc::channel();
    drop(receiver);

    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .send_to(RFC3164.as_bytes(), socket.local_addr().unwrap())
        .unwrap();

    listen::receive_datagrams(&socket, &sender);
}

#[test]
fn a_syslog_receiver_reads_udp_and_tcp_on_one_port() {
    let (address, lines) = listen::start_syslog("127.0.0.1:0").unwrap();
    let wait = Duration::from_secs(10);

    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .send_to(format!("{}\n", RFC3164).as_bytes(), address)
        .unwrap();
    let from_udp = lines.recv_timeout(wait).unwrap();

    TcpStream::connect(address)
        .unwrap()
        .write_all(octet_counted(RFC5424).as_bytes())
        .unwrap();
    let from_tcp = lines.recv_timeout(wait).unwrap();

    assert_eq!(
        from_udp,
        Received {
            sender: "127.0.0.1".to_string(),
            line: RFC3164.to_string()
        }
    );
    assert_eq!(
        from_tcp,
        Received {
            sender: "127.0.0.1".to_string(),
            line: RFC5424.to_string()
        }
    );
}

#[test]
fn a_syslog_address_that_cannot_be_bound_is_an_error() {
    assert!(listen::start_syslog("not an address").is_err());
}

#[test]
fn syslog_listens_on_port_514_on_every_interface_by_default() {
    assert_eq!(listen::bind_address(listen::SYSLOG_ADDRESS), "0.0.0.0:514");
}
