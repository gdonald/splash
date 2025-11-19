# splash

A fast, modern log colorizer built in Rust.

[![Tests](https://img.shields.io/badge/tests-27%20passing-brightgreen)]()
[![Clippy](https://img.shields.io/badge/clippy-passing-brightgreen)]()

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
  -m, --mode <MODE>  Log Parsing Mode (clf, ad-hoc)
  -p, --path <PATH>  Path to the log file
  -h, --help         Print help information
  -V, --version      Print version information
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

---

## License

[https://github.com/gdonald/splash/blob/main/LICENSE](https://github.com/gdonald/splash/blob/main/LICENSE)

