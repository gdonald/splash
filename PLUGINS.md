# splash plugins

A plugin teaches splash a log format. Most plugins are TOML files: a name and the regular
expressions that read the format's lines, loaded when splash starts, with no build step. The
formats splash ships with are plugins written in Rust, compiled into splash.

- [Plugin files](#plugin-files): the TOML plugin format
- [Writing a plugin](#writing-a-plugin): a plugin file from sample lines to daily use
- [Built-in plugins](#built-in-plugins): the Rust plugin API

---

## Plugin files

### Where splash looks

splash loads every `.toml` file in these directories, in name order:

| Directory | |
| --- | --- |
| `~/.splash/plugins` | Your own plugins |
| `/usr/local/lib/splash/plugins` | Plugins installed for every user |
| `/usr/lib/splash/plugins` | Plugins a package installed |
| `%ProgramFiles%\splash\plugins` | On Windows, in place of the two above |
| `--plugin-dir DIR` | Any other directory, and the option can be repeated |

`splash --list-plugins` prints every plugin it loaded and the directories it searched. A
missing directory is skipped, and so is one splash cannot read.

### Keys

```toml
name = "orders"
version = "1.2.0"
description = "Order service logs"
author = "Platform team"

[patterns]
placed = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) order (?P<transaction>\d+) placed in (?P<duration>\d+ms)$'
other = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) (?P<message>.*)$'
```

| Key | Required | Meaning |
| --- | --- | --- |
| `name` | Yes | The mode name: `--mode orders` reads a log with this plugin |
| `version` | No | `MAJOR.MINOR.PATCH`, `1.0.0` when left out |
| `description` | No | Shown by `--list-plugins` |
| `author` | No | Shown by `--list-plugins` |
| `[patterns]` | Yes | One or more regular expressions, each under a name of your choosing |

Any other key is an error. The name cannot be one splash uses itself: a built-in plugin's name,
`clf`, `ad-hoc`, `auto`, `k8s`, or `systemd`.

Write patterns as literal strings in single quotes. A literal string keeps every backslash as it
is, so `\d` and `\S` reach the regular expression unchanged, and it cannot hold a single quote. A
double-quoted string reads `\n`, `\t`, `\r`, `\"`, and `\\` as escapes and rejects any other
backslash. The pattern syntax is that of the Rust `regex` crate.

### How a line is read

The patterns are tried in the order the file lists them, and the first that matches reads the
line. A line no pattern matches is not shown in that plugin's mode, as with the built-in modes,
and in auto mode it is detected on its own.

Each named group is named for the color key its text is styled with, such as `timestamp`,
`level`, `client`, `status`, or `message`. The README lists every key under
[Color overrides](README.md#color-overrides). Text outside the named groups, including text
before or after an unanchored match, is plain. A group inside another named group, and a group
that matched no text, add nothing. A named group that is not a color key is an error, and an
unnamed group `( )` is allowed anywhere.

What the groups are named for decides more than color:

| Group | Used by |
| --- | --- |
| `level` | `--level`, the error count of `--stats` and `--summary` |
| `timestamp` | `--since`, `--until`, `--tz`, `--relative`, `--merge`, `--histogram` |
| `client`, `ip` | `--filter ip=`, `--lookups`, `--anonymize`, the top addresses of `--stats` |
| `request`, `status` | The top URLs and status counts of `--stats` |
| `userid`, `email` | `--anonymize` |
| any key | `--filter KEY=VALUE`, and the fields of `--output jsonl` and `--output csv` |

### When a file cannot be loaded

splash reports the file and the reason on stderr and goes on without it:

```
Cannot load plugin /home/alice/.splash/plugins/orders.toml: pattern 'placed' names group 'txn', which is not a color key
```

The reasons are a TOML syntax error with its line number, a missing name, a built-in name, a
version that is not three numbers, an unknown key, no `[patterns]`, a pattern that is not a
regular expression, and a named group that is not a color key.

---

## Writing a plugin

This builds a plugin for an order service that writes:

```
2023-10-03 12:00:01 INFO order 4821 placed in 38ms
2023-10-03 12:00:02 WARN payment retried for order 4821
2023-10-03 12:00:03 ERROR order 4822 failed: card declined
```

### Start from the lines

Save a few real lines to a file, `orders.log`, including the unusual ones: errors, lines with
missing fields, and lines another program writes into the same log.

### Write the plugin file

Create `~/.splash/plugins/orders.toml` with a pattern for the most specific line first and a
general pattern last:

```toml
name = "orders"
description = "Order service logs"

[patterns]
placed = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) order (?P<transaction>\d+) placed in (?P<duration>\d+ms)$'
failed = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) order (?P<transaction>\d+) (?P<failure>failed): (?P<message>.*)$'
other = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) (?P<message>.*)$'
```

### Test the patterns

`--test-regex FILE` reads lines from `--path` or stdin and reports, for each line, the pattern
that read it and the text of each group, or that no pattern read it. It exits with status 1 when
a line was not read, so it can run in a test script:

```bash
splash --test-regex ~/.splash/plugins/orders.toml --path orders.log
```

```
1: placed
   timestamp        2023-10-03 12:00:01
   level            INFO
   transaction      4821
   duration         38ms
2: other
   timestamp        2023-10-03 12:00:02
   level            WARN
   message          payment retried for order 4821
3: failed
   timestamp        2023-10-03 12:00:03
   level            ERROR
   transaction      4822
   failure          failed
   message          card declined
3 of 3 lines matched
```

Blank lines are skipped. With several `--path` files, each file's report follows a
`==> PATH <==` header. A plugin file that cannot be loaded exits with an error naming the reason.

### Use it

```bash
splash --mode orders --path /var/log/orders.log
splash --auto --path /var/log/orders.log
splash --mode orders --path /var/log/orders.log --output jsonl --fields transaction,duration
```

Auto mode tries plugin files along with the built-in modes.

### Change it while it runs

`--reload-plugins` watches the plugin directories and loads the plugin files again whenever one
changes, so lines rendered after you save use the new patterns:

```bash
splash --mode orders --path /var/log/orders.log --follow --reload-plugins
```

Each reload is reported on stderr with the plugins and versions it registered:

```
Reloaded plugins: orders 1.0.0
```

A file that cannot be loaded is reported and its plugin keeps the patterns it had, so a typo
does not stop the log. A plugin directory that cannot be watched is skipped. Deleting a file, or renaming its plugin, unregisters the old plugin.
Raising `version` in each save shows in the report which save was loaded.

### Examples

[`examples/plugins/`](examples/plugins/) holds plugins for Gunicorn error logs, Celery worker
logs, and uWSGI request logs. Try one without installing it:

```bash
splash --plugin-dir examples/plugins --mode celery --path /var/log/celery/worker.log
```

---

## Built-in plugins

The formats splash ships with are Rust types implementing the `Plugin` trait in
[`src/plugin.rs`](src/plugin.rs). Write one in Rust when a pattern cannot read the format: JSON
whose fields can come in any order, a value styled by what it says, or a line that holds another
program's line.

### The trait

```rust
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;
    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a>;

    fn name(&self) -> &str { ... }
    fn version(&self) -> &PluginVersion { ... }
    fn parse_display_line<'a>(&self, line: &'a str) -> ParseResult<'a> { ... }
    fn can_parse(&self, line: &str) -> bool { ... }
    fn detect_format(&self, sample_lines: &[&str]) -> f32 { ... }
}
```

| Method | Purpose |
| --- | --- |
| `metadata` | The name, version, description, and author, built with `PluginMetadata::new` |
| `parse_line` | Reads one line of the format, returning `ParseResult::Parsed` with its tokens, or `ParseResult::NoMatch` |
| `parse_display_line` | Reads a line for rendering in the plugin's mode. It defaults to `parse_line`. The docker and kubernetes plugins override it to show the lines containers printed, which detection must not count |
| `can_parse` | Whether `parse_line` reads the line |
| `detect_format` | The share of a sample of lines the plugin reads, from 0.0 to 1.0, which auto mode ranks plugins by |

### Tokens

A `ParsedLine` is a list of `Token`s, each holding a run of the line's text and the `TokenKind`
it is styled with. `Token::new` borrows the text from the line, so parsing copies nothing. The
tokens must hold every character of the line, in order, with the parts that are not fields as
`TokenKind::Plain` or `TokenKind::Punctuation`: rendering, filtering, and the `text` field of
JSON output are built from the tokens.

Modules shared by several plugins read common pieces: `syslog::split_header` reads a syslog
header, `json::parse_object` scans a JSON object, `logfmt::push_pairs` reads `key=value` pairs,
and `container::push_output` colors a line in whatever format reads it.

### Adding one

1. Add `src/NAME.rs` with a `NAME` constant, the plugin type, and a `parse_line` function, and
   declare the module in `src/lib.rs`.
2. Register the plugin in `PluginRegistry::with_builtins` in `src/registry.rs`.
3. Add it to the `--mode` help text in `src/main.rs` and to the README's mode list and Modes
   section.
4. Add sample logs to `tests/examples/`, parsing tests to `tests/NAME_tests.rs`, and tests that
   run splash on the samples to `tests/examples_runner.rs`. Tests never go in `src/`.
