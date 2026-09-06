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
  -m, --mode <MODE>                      Log Parsing Mode (clf, ad-hoc)
  -p, --path <PATH>                      Path to the log file
  -o, --output <OUTPUT>                  Output format (ansi, curses, html, json, plain) [default:
                                         ansi]
      --list-plugins                     List all available plugins
      --plugin <PLUGIN>                  Use a specific plugin by name
      --disable-plugin <DISABLE_PLUGIN>  Disable a specific plugin by name
  -h, --help                             Print help
  -V, --version                          Print version
```

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

## Features

### Current (v0.1.0)

**Log Format Support**
- Common Log Format (CLF) parsing
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
- Stdin streaming

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
