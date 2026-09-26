# splash

A fast, modern log colorizer built in Rust.

###

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/gdonald/splash/blob/main/LICENSE) [![CI](https://github.com/gdonald/splash/workflows/CI/badge.svg)](https://github.com/gdonald/splash/actions) [![codecov](https://codecov.io/gh/gdonald/splash/graph/badge.svg?token=GQ4LA1VMRE)](https://codecov.io/gh/gdonald/splash)

## Quick Start

```bash
# Install
cargo install --path .

# Colorize a log file (Common Log Format)
splash --mode clf --path /var/log/apache2/access.log

# Colorize Apache and nginx access and error logs
splash --mode httpd --path /var/log/nginx/error.log

# Colorize a Squid access log, marking cache hits and misses
splash --mode squid --path /var/log/squid/access.log

# Colorize a Varnish transaction log
varnishlog | splash --mode varnish

# Colorize a HAProxy log
splash --mode haproxy --path /var/log/haproxy.log

# Colorize Caddy's structured JSON logs
splash --mode caddy --path /var/log/caddy/access.log

# Colorize mail server logs
splash --mode postfix --path /var/log/mail.log
splash --mode exim --path /var/log/exim4/mainlog
splash --mode dovecot --path /var/log/dovecot.log
splash --mode fetchmail --path ~/.fetchmail.log
splash --mode procmail --path ~/.procmail.log

# Colorize with ad-hoc mode (auto-detects patterns)
splash --mode ad-hoc --path /var/log/syslog

# Pipe from stdin
tail -f /var/log/nginx/access.log | splash --mode clf

# Or just
cat logfile.log | splash
```

---

## Usage

```
Usage: splash [OPTIONS]

Options:
  -m, --mode <MODE>                      Log Parsing Mode (caddy, clf, dovecot, exim, fetchmail,
                                         haproxy, httpd, postfix, procmail, squid, varnish, ad-hoc)
  -p, --path <PATH>                      Path to the log file
  -o, --output <OUTPUT>                  Output format (ansi, curses, html, json, plain)
  -j, --jobs <JOBS>                      Worker threads used to render a file (default: one per
                                         core)
      --config <CONFIG>                  Config file to read instead of ~/.splash/config.toml or
                                         ~/.splashrc
      --theme <THEME>                    Color theme (dark, light, solarized, dracula)
      --color <KEY=COLOR>                Override one token color, as KEY=COLOR (repeatable)
      --profile <PROFILE>                Load a saved color profile
      --save-profile <NAME>              Save the resolved colors as a profile and exit
      --list-profiles                    List saved color profiles
      --list-themes                      List available color themes
      --list-plugins                     List all available plugins
      --plugin <PLUGIN>                  Use a specific plugin by name
      --disable-plugin <DISABLE_PLUGIN>  Disable a specific plugin by name
  -h, --help                             Print help
  -V, --version                          Print version
```

---

## Configuration

splash reads `~/.splash/config.toml`, falling back to `~/.splashrc`. Both use the same format.
`--config PATH` reads a different file instead.

```toml
# Defaults used when the command line does not say otherwise
mode = "clf"
output = "ansi"
theme = "dracula"
jobs = "4"

# Repaint individual fields
[colors]
ip = "bright cyan"
status = "white bold"

# Settings for one plugin
[plugins.syslog]
enabled = "true"
facility = "cyan"
```

Command line options win over the config file. Colors are layered: the theme or profile first,
then the file's `[colors]` table, then any `--color` overrides.

### Themes

`--theme NAME` selects one of the presets `dark` (the default), `light`, `solarized`, and
`dracula`. `--list-themes` prints them. A theme sets a color for every token kind and the page
colors used by HTML output.

```bash
splash --mode clf --path access.log --theme solarized
```

### Color overrides

`--color KEY=COLOR` repaints one token kind. The key is the kind name used in JSON output and in
HTML class names: `plain`, `punctuation`, `ip`, `number`, `datetime`, `tz_offset`, `http_verb`,
`http_version`, `client`, `user_identifier`, `userid`, `timestamp`, `method`, `request`,
`protocol`, `status`, `size`, `referer`, `user_agent`, `vhost`, `duration`, `level`,
`module`, `pid`, `message`, `cache_hit`, `cache_miss`, `cache_result`, `hierarchy`, and
`content_type`, `transaction`, `tag`, `header`, `host`, `frontend`, `backend`, `server`,
`timers`, `termination`, and `counters`.

A color is one of the sixteen ANSI color names (`red`, `bright cyan`, `gray`), a hex value
(`#ff5555`), or either of those followed by `bold`.

```bash
splash --mode clf --path access.log --color ip=bright_cyan --color status="white bold"
```

### Color profiles

`--save-profile NAME` writes the colors splash resolved to `~/.splash/profiles/NAME.toml`, and
`--profile NAME` loads them back. `--list-profiles` prints the saved profiles.

```bash
splash --theme solarized --color ip=green --save-profile work
splash --mode clf --path access.log --profile work
```

A profile is a config file holding a `theme` key and a `[colors]` table, so it can also be edited
by hand.

### Per-plugin settings

A `[plugins.NAME]` table holds settings for one plugin. `enabled = "false"` turns a plugin off.
`--list-plugins` prints each configured plugin and its settings.

---

## Modes

### Common Log Format (CLF)

Parses and colorizes logs in the [Common Log Format](https://en.wikipedia.org/wiki/Common_Log_Format) used by Apache, nginx, and other web servers.

**Example:**
```bash
splash --mode clf --path /var/log/apache2/access.log
```

**Format:**
```
127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326
```

**Note:** Nothing will be shown if the log file is not actually formatted in CLF format. Use ad-hoc mode if you are unsure.

### Apache and nginx (httpd)

Reads the four shapes an Apache or nginx log takes. Each line is tried against them in turn and the
first that fits wins, so an access log and an error log can be colorized with the same mode.

**Example:**
```bash
splash --mode httpd --path /var/log/apache2/access.log
splash --mode httpd --path /var/log/nginx/error.log
```

**Combined access log**, the Common Log Format plus a quoted referer and user agent:
```
127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326 "http://www.example.com/start.html" "Mozilla/5.0"
```

**vhost combined access log**, which prefixes the server name:
```
example.com:80 10.0.0.42 - - [10/Oct/2000:13:55:37 -0700] "GET / HTTP/1.1" 200 512 "-" "curl/8.4.0"
```

**Extended access log**, whose trailing fields follow the user agent. A field of digits alone is
the microseconds the server spent on the request, colored as `duration`:
```
127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.1" 200 4512 "-" "Mozilla/5.0" 1532
```

**Apache error log**, whose module, level, process, and client are each colored separately:
```
[Wed Oct 11 14:32:52.123456 2023] [core:error] [pid 35708:tid 4328636416] [client 72.15.99.187:1234] AH00128: File does not exist
```

**nginx error log**:
```
2023/10/11 14:32:52 [error] 1234#0: *1 open() failed (2: No such file or directory), client: 10.0.0.9
```

A plain Common Log Format line is read too, and addresses inside an error message are colored. A
line in none of these formats is dropped.

### Squid

Reads a Squid access log in either shape it is written in, and colors the result code by whether
Squid served the request from its cache. A code ending in `_HIT` is a cache hit, one ending in
`_MISS` is a cache miss, and anything else, such as `TCP_DENIED`, gets a neutral style.

**Example:**
```bash
splash --mode squid --path /var/log/squid/access.log
```

**Native format**, which leads with a Unix timestamp and pads its fields to fixed widths:
```
1614729600.123    123 10.0.0.5 TCP_MISS/200 4512 GET http://example.com/ - HIER_DIRECT/93.184.216.34 text/html
```

**httpd-emulated format**, written when `emulate_httpd_log` is on, which appends the result code
and the hierarchy code to a Common Log Format line:
```
10.0.0.5 - - [09/Mar/2021:12:00:00 +0000] "GET http://example.com/ HTTP/1.1" 200 4512 TCP_MISS:HIER_DIRECT
```

The padding between native fields is kept, so plain output reproduces the file. A line in neither
format is dropped.

### Varnish

Reads what `varnishlog` writes. A transaction header names the transaction and its id, and the
records under it are indented with one dash per level of nesting. Each record's payload is colored
by the tag that introduces it, so a `ReqMethod` payload is painted as a method and a `RespStatus`
payload as a status. A tag splash has no field for keeps its payload as message text, with any
addresses inside it colored.

**Example:**
```bash
varnishlog | splash --mode varnish
varnishlog -g raw | splash --mode varnish
```

**Transaction format:**
```
*   << Request  >> 32770
-   ReqMethod      GET
-   ReqURL         /index.html
-   ReqHeader      Host: example.com
-   RespStatus     200
-   End
```

**Raw format**, written by `varnishlog -g raw`, which leads each record with its transaction id and
the side it came from, `c` for the client and `b` for the backend:
```
        32770 ReqMethod      c GET
        32771 BerespStatus   b 503
```

The padding varnishlog uses to align its tags is kept, so plain output reproduces the stream. A
line in neither format is dropped.

### HAProxy

Reads the three kinds of line HAProxy writes, with or without the syslog header that names the
host and the process. A line logged straight to stdout, as it is in a container, is read the same
way.

**Example:**
```bash
splash --mode haproxy --path /var/log/haproxy.log
```

**`option httplog`**, which adds the status, the captured cookies, and the quoted request to the
connection fields:
```
Feb  6 12:14:14 gateway haproxy[14389]: 10.0.1.2:33317 [06/Feb/2009:12:14:14.655] http-in static/srv1 10/0/30/69/109 200 2750 - - ---- 1/1/1/1/0 0/0 "GET /index.html HTTP/1.1"
```

**`option tcplog`**, which stops at the connection counters:
```
Feb  6 12:12:56 gateway haproxy[14387]: 10.0.1.2:33313 [06/Feb/2009:12:12:51.443] fnt bck/srv1 0/0/5007 212 -- 0/0/0/0/3 0/0
```

**Error lines**, which name the listener that refused the connection and then say what went wrong:
```
Feb  6 12:12:56 gateway haproxy[14387]: 127.0.0.1:34550 [06/Feb/2009:12:12:51.443] frt/f1: invalid request
```

The frontend, the backend, and the server each get their own color, as do the timers, the
termination state, and the connection and queue counters. Captured headers before the quoted
request are kept as message text. A line in none of the three formats is dropped.

### Caddy

Reads the JSON objects Caddy's structured encoder writes, one per line. The line is scanned as
JSON rather than matched against a pattern, so a field is colored wherever it sits in the object,
including inside the nested `request`, `headers`, and `tls` objects.

**Example:**
```bash
splash --mode caddy --path /var/log/caddy/access.log
```

**Format:**
```json
{"level":"info","ts":1646861401.52,"logger":"http.log.access","msg":"handled request","request":{"remote_ip":"127.0.0.1","proto":"HTTP/2.0","method":"GET","host":"localhost","uri":"/"},"duration":0.0009,"size":10900,"status":200}
```

Every key is colored as a key, and a value takes its color from the key above it: `level`,
`ts`, `logger`, `msg`, `error`, `remote_ip`, `client_ip`, `proto`, `method`, `host`, `uri`,
`status`, `size`, `bytes_read`, `duration`, and `user_id`. A key splash has no field for keeps its
value in the default style for that value's type, so nothing Caddy adds goes uncolored. Spacing
between the pieces of the object is kept, so plain output reproduces the file. A line that is not
a single well formed JSON object is dropped.

### Mail servers

The five mail modes share how they read a message. A `key=value` field has its key colored as a
`header` and its value colored by the key: `from=` and `to=` are `email`, `relay=` and `H=` are
`host` with the bracketed address after the name colored as `ip`, `size=` is `size`, `delay=` is
`duration`, and `session=` and `id=` are `transaction`. A value in quotes or angle brackets keeps
them as punctuation. An address in angle brackets, or standing alone, is colored as `email`.

Whether the mail went through is colored with three styles: `success`, `warning`, and `failure`.
Each mode applies them to the words its program uses, listed below. A field splash has no style
for keeps its value as message text, with any IP addresses in it colored. A line the mode does not
recognize is dropped.

A syslog header, in the traditional `Oct  3 12:00:01` form or the RFC 3339 form, is colored as a
timestamp, a host, a program, and a process id. The Postfix, Exim, Dovecot, and Fetchmail modes
only read syslog lines logged by their own program.

#### Postfix

Reads the lines every Postfix daemon sends to syslog. The queue id is colored as `queue_id`, in
the short hexadecimal form, the long form `enable_long_queue_ids` turns on, or `NOQUEUE`.
`status=sent` is a success, `status=deferred` a warning, and `status=bounced` or `expired` a
failure. A leading `warning:` or `hold:` is a warning, and `reject:`, `discard:`,
`milter-reject:`, `error:`, `fatal:`, and `panic:` are failures. In the message, `removed` is a
success, `lost connection` and `timeout` are warnings, and `Relay access denied`,
`Recipient address rejected`, `Sender address rejected`, and `authentication failed` are failures.

**Example:**
```bash
splash --mode postfix --path /var/log/mail.log
```

```
Oct  3 12:00:02 mail postfix/smtp[1236]: 4F2A1C0123: to=<bob@example.org>, relay=mx.example.org[93.184.216.34]:25, delay=0.52, delays=0.1/0/0.2/0.22, dsn=2.0.0, status=sent (250 2.0.0 OK)
Oct  3 12:05:12 mail postfix/smtpd[1243]: NOQUEUE: reject: RCPT from unknown[192.0.2.10]: 554 5.7.1 <erin@example.org>: Relay access denied; from=<spam@example.biz> to=<erin@example.org> proto=ESMTP helo=<example.biz>
```

#### Exim

Reads Exim's main log, and the same lines sent to syslog. The message id is colored as
`queue_id`, in the form Exim wrote before 4.97 and the longer one it writes since. The flag after
it is colored by what it says: `<=`, `=>`, and `->` are successes, `*>` and `==` are warnings, and
`**` is a failure. The router `R=` and the transport `T=` are colored as `module`. `Completed` is
a success, `Frozen` and `retry time not reached` are warnings, and `rejected` and `SMTP error` are
failures.

**Example:**
```bash
splash --mode exim --path /var/log/exim4/mainlog
```

```
2023-10-03 12:00:01 1qnXYZ-000ABC-12 <= alice@example.com H=mail.example.com [10.0.0.5] P=esmtps S=4512 id=20231003120001.abc@example.com
2023-10-03 12:00:02 1qnXYZ-000ABC-12 => bob@example.org R=dnslookup T=remote_smtp H=mx.example.org [93.184.216.34] C="250 2.0.0 OK"
2023-10-03 12:05:11 1qnXZb-000ABE-4G ** erin@example.com R=dnslookup T=remote_smtp H=mx.example.com [198.51.100.4]: SMTP error from remote mail server after RCPT TO:<erin@example.com>: 550 5.1.1 User unknown
```

A timestamp with milliseconds and a time zone, and the process id `log_selector = +pid` adds, are
read too.

#### Dovecot

Reads Dovecot's lines from syslog and from its own log file. The service is colored as `module`,
the user in parentheses after it as `userid` (or `pid` when it is a process id), and the ids in
angle brackets as `pid` and then `transaction`. A level such as `Info:` or `Error:` is colored as
`level`. `Login` and `saved mail to` are successes, and `auth failed`, `Aborted login`,
`Password mismatch`, `unknown user`, and `Quota exceeded` are failures.

**Example:**
```bash
splash --mode dovecot --path /var/log/dovecot.log
```

```
Oct  3 12:00:01 mail dovecot: imap-login: Login: user=<alice>, method=PLAIN, rip=10.0.0.5, lip=10.0.0.1, mpid=4321, TLS, session=<Xy7AbC>
Oct 03 12:00:05 imap(alice)<4321><Xy7AbC>: Info: Disconnected: Logged out in=1024 out=65536
```

#### Fetchmail

Reads fetchmail's lines from syslog and from its own log file, where they carry a `fetchmail: `
prefix, including the per-message `reading message` and `skipping message` lines that carry no
prefix. The user and server a poll was for are colored as `userid` and `host`, a size in octets
as `size`, counts and version numbers as `number`, and a date as `timestamp`. `flushed` and
`SUCCESS` are successes, `not flushed`, `skipped`, `timeout`, and `LOCKBUSY` are warnings, and
`Authorization failure`, `AUTHFAIL`, `SOCKET`, `PROTOCOL`, and `error` are failures.

**Example:**
```bash
splash --mode fetchmail --path ~/.fetchmail.log
```

```
fetchmail: 3 messages (1 seen) for alice at mail.example.com (12345 octets).
reading message alice@mail.example.com:1 of 3 (4096 octets) flushed
Oct  3 12:00:03 laptop fetchmail[2200]: Query status=3 (AUTHFAIL)
```

#### Procmail

Reads procmail's `LOGFILE`. Each delivered mail is logged as a `From` line with the envelope
sender and the date, a `Subject:` line, and a `Folder:` line with where the mail went, colored as
`path`, and its size. Diagnostics carry a `procmail: ` prefix and, under `VERBOSE=on`, a process
id. In a diagnostic, a quoted path is colored as `path` and a quoted assignment has its variable
colored as `header`. `Match on` is a success, `No match on`, `Skipped`, and `Timeout` are warnings,
and `Couldn't`, `Error`, `Unable`, `Lock failure`, and `Bad substitution` are failures.

**Example:**
```bash
splash --mode procmail --path ~/.procmail.log
```

```
From alice@example.com  Tue Oct  3 12:00:01 2023
 Subject: Quarterly report
  Folder: /home/bob/Mail/inbox						   4512
procmail: Match on "^From:.*alice@example.com"
procmail: Couldn't create "/var/mail/bob"
```

The spacing between fields is kept in every mail mode, so plain output reproduces the file.

### Ad-hoc Mode

Automatically detects and highlights patterns in unstructured logs:

- **IP addresses** (IPv4)
- **HTTP verbs** (GET, POST, PUT, DELETE, etc.)
- **Numbers**
- **Timestamps**
- **Special characters** (quotes, brackets)

**Example:**
```bash
splash --mode ad-hoc --path /var/log/syslog
```

**Default mode:** If no mode is specified, `ad-hoc` is used by default.

---

## Output Formats

Every parsing mode can be written out in five formats, selected with `--output`.

### ansi

ANSI escape sequences for a color terminal. This is the default.

```bash
splash --mode clf --path access.log
```

### curses

A full-screen scrollable viewer. splash reads the whole input, colorizes it, and shows one
screenful at a time with a status line giving the visible range.

```bash
splash --mode clf --path access.log --output curses
```

| Key | Action |
| --- | --- |
| `j`, down arrow | Scroll down one line |
| `k`, up arrow | Scroll up one line |
| `f`, space, page down | Scroll down one screen |
| `b`, page up | Scroll up one screen |
| `g`, home | Jump to the first line |
| `G`, end | Jump to the last screen |
| `q`, escape | Quit |

The viewer needs a terminal. Redirecting its output exits with an error naming the other modes.

### html

A standalone HTML document. Each token becomes a `<span>` with a `splash-*` class, and the
document carries a stylesheet defining a color for every class, so the colors can be changed
by editing the CSS.

```bash
cat access.log | splash --mode clf --output html > access.html
```

### json

One JSON object per line, holding the line text and the styled tokens it was split into.
In `clf` mode the token kinds are the Common Log Format field names.

```bash
cat access.log | splash --mode clf --output json
```

```json
{"text":"127.0.0.1 - frank ...","tokens":[{"kind":"client","text":"127.0.0.1"}]}
```

### plain

The parsed line with no styling, for scripting.

```bash
cat access.log | splash --mode clf --output plain
```

---

## Performance

Parsed tokens borrow the text of the line they came from, so a line is read once and never copied
on its way to the renderer. Patterns are compiled the first time they are needed and kept for the
rest of the run, and patterns supplied at run time are cached by `parser::cached_pattern`.

Files are read whole and rendered before splash starts watching for new lines. A file of 64 KiB or
more is memory mapped rather than read into memory, and its lines are split across worker threads.
`--jobs N` sets the worker count, which defaults to one per core. Inputs under 512 lines are
rendered on one thread, where splitting the work costs more than it saves.

```bash
splash --mode clf --path /var/log/apache2/access.log --jobs 8
```

### Benchmarks

`cargo bench` times splash against itself at several settings and, when ccze is installed, against
ccze over the same log.

```
Rendering 20000 log lines
  splash clf ansi                 20000 lines in    33.636 ms        594603 lines/sec    1.00x
  splash clf ansi -j 18           20000 lines in     5.355 ms       3734740 lines/sec    6.28x
  splash clf plain                20000 lines in    18.454 ms       1083771 lines/sec    1.82x
  splash ad-hoc ansi              20000 lines in    36.841 ms        542869 lines/sec    0.91x
```

The numbers above are from one machine, on a generated Common Log Format sample. Run the benchmark
on yours before drawing conclusions from them.

---

## Features

### Current (v0.1.0)

**Log Format Support**
- Common Log Format (CLF) parsing
- Apache and nginx access logs: combined, vhost combined, and extended
- Apache and nginx error logs
- Squid access logs, native and httpd-emulated, with cache hit and miss highlighting
- Varnish transaction and raw logs, with each record payload colored by its tag
- HAProxy logs from `option httplog` and `option tcplog`, plus error lines
- Caddy structured JSON logs, scanned as JSON and colored by key
- Postfix syslog lines, with queue ids, addresses, relays, and delivery status
- Exim main logs, with message ids and arrival, delivery, deferral, and failure flags
- Dovecot syslog and log file lines, with the service, user, and session
- Fetchmail syslog and log file lines, with the user, server, sizes, and flush state
- Procmail delivery abstracts and diagnostics
- Ad-hoc pattern detection

**Pattern Highlighting**
- IP addresses (192.168.x.x, 10.x.x.x, etc.)
- HTTP methods (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, CONNECT, TRACE)
- HTTP status codes (200, 404, 500, etc.)
- Timestamps (multiple formats)
- Numbers
- Quotes and brackets

**Input Sources**
- File input with live watching
- Memory-mapped reads for files of 64 KiB or more
- Stdin streaming

**Performance**
- Parsing that borrows the input rather than copying it
- Patterns compiled on first use and cached
- Rendering split across worker threads, set with `--jobs`
- `cargo bench` benchmark suite, comparing to ccze when it is installed

**Configuration**
- `~/.splash/config.toml` and `~/.splashrc` config files
- Per-plugin settings
- Color themes: dark, light, solarized, dracula
- Per-field color overrides from the command line
- Saved color profiles

**Output Formats**
- ANSI colors (default)
- Curses viewer with scrolling and vim-style keys
- HTML with a customizable stylesheet
- JSON with named token kinds
- Plain text

---

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/gdonald/splash.git
cd splash

# Build and install
cargo install --path .

# Run tests
cargo test
```

### Requirements

- Rust 1.70+ (edition 2021)
- Cargo

---

## Development

### Running Tests

```bash
# All tests (27 tests)
cargo test

# Integration tests only
cargo test --test examples_runner

# Specific test
cargo test test_clf_basic_parsing

# With output
cargo test -- --nocapture

# Single-threaded (for debugging)
cargo test -- --test-threads=1
```

### Running Clippy

```bash
# Check for warnings
cargo clippy

# Treat warnings as errors
cargo clippy -- -D warnings
```

### Manual Testing

```bash
# Test CLF mode
cargo run -- --mode clf --path tests/examples/clf_basic.log

# Test ad-hoc mode
cargo run -- --mode ad-hoc --path tests/examples/adhoc_mixed.log

# Test stdin
cat tests/examples/real_apache.log | cargo run -- --mode clf

# Test with real logs
tail -f /var/log/syslog | cargo run
```
