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
`protocol`, `status`, and `size`.

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
