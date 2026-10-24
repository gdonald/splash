use splash::config::{Config, Options, Profiles, Settings};
use splash::dates::DateOptions;
use splash::filter::Entry;
use splash::render_contents_settings;
use std::fs;
use std::path::PathBuf;

/// A JSON value, read by a parser that follows RFC 8259 strictly, so output
/// a lenient reader would accept still fails these tests
#[derive(Debug, Clone, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    fn text(&self) -> &str {
        match self {
            Value::Text(text) => text,
            other => panic!("{:?} is not a string", other),
        }
    }

    fn items(&self) -> &[Value] {
        match self {
            Value::Array(items) => items,
            other => panic!("{:?} is not an array", other),
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

fn parse(text: &str) -> Result<Value, String> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        position: 0,
    };

    let value = reader.value()?;
    reader.whitespace();

    match reader.position == reader.bytes.len() {
        true => Ok(value),
        false => Err(format!("text after the value at byte {}", reader.position)),
    }
}

impl Reader<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn fail<T>(&self, wanted: &str) -> Result<T, String> {
        Err(format!("expected {} at byte {}", wanted, self.position))
    }

    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        match self.bytes[self.position..].starts_with(word.as_bytes()) {
            true => {
                self.position += word.len();
                Ok(value)
            }
            false => self.fail(word),
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.whitespace();

        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Value::Text),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => self.fail("a value"),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.position += 1;
        let mut members = Vec::new();
        self.whitespace();

        if self.peek() == Some(b'}') {
            self.position += 1;
            return Ok(Value::Object(members));
        }

        loop {
            self.whitespace();

            if self.peek() != Some(b'"') {
                return self.fail("a member name");
            }

            let name = self.string()?;
            self.whitespace();

            if self.peek() != Some(b':') {
                return self.fail("':'");
            }

            self.position += 1;
            members.push((name, self.value()?));
            self.whitespace();

            match self.peek() {
                Some(b',') => self.position += 1,
                Some(b'}') => {
                    self.position += 1;
                    return Ok(Value::Object(members));
                }
                _ => return self.fail("',' or '}'"),
            }
        }
    }

    fn array(&mut self) -> Result<Value, String> {
        self.position += 1;
        let mut items = Vec::new();
        self.whitespace();

        if self.peek() == Some(b']') {
            self.position += 1;
            return Ok(Value::Array(items));
        }

        loop {
            items.push(self.value()?);
            self.whitespace();

            match self.peek() {
                Some(b',') => self.position += 1,
                Some(b']') => {
                    self.position += 1;
                    return Ok(Value::Array(items));
                }
                _ => return self.fail("',' or ']'"),
            }
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.position;

        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.position += 1;
        }

        self.position - start
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.position;

        if self.peek() == Some(b'-') {
            self.position += 1;
        }

        match self.peek() {
            Some(b'0') => self.position += 1,
            Some(b'1'..=b'9') => {
                self.digits();
            }
            _ => return self.fail("a digit"),
        }

        if self.peek() == Some(b'.') {
            self.position += 1;

            if self.digits() == 0 {
                return self.fail("a fraction digit");
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;

            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }

            if self.digits() == 0 {
                return self.fail("an exponent digit");
            }
        }

        let text = std::str::from_utf8(&self.bytes[start..self.position]).unwrap();

        Ok(Value::Number(text.to_string()))
    }

    fn hex(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.position..self.position + 4)
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .filter(|digits| digits.bytes().all(|digit| digit.is_ascii_hexdigit()));

        match digits {
            Some(digits) => {
                self.position += 4;
                Ok(u32::from_str_radix(digits, 16).unwrap())
            }
            None => self.fail("four hex digits"),
        }
    }

    fn escape(&mut self) -> Result<char, String> {
        let escaped = self.peek();
        self.position += 1;

        match escaped {
            Some(b'"') => Ok('"'),
            Some(b'\\') => Ok('\\'),
            Some(b'/') => Ok('/'),
            Some(b'b') => Ok('\u{8}'),
            Some(b'f') => Ok('\u{c}'),
            Some(b'n') => Ok('\n'),
            Some(b'r') => Ok('\r'),
            Some(b't') => Ok('\t'),
            Some(b'u') => {
                let unit = self.hex()?;

                if !(0xd800..0xdc00).contains(&unit) {
                    return char::from_u32(unit).map_or_else(|| self.fail("a character"), Ok);
                }

                if !self.bytes[self.position..].starts_with(b"\\u") {
                    return self.fail("a low surrogate");
                }

                self.position += 2;
                let low = self.hex()?;

                match (0xdc00..0xe000).contains(&low) {
                    true => Ok(
                        char::from_u32(0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00)).unwrap(),
                    ),
                    false => self.fail("a low surrogate"),
                }
            }
            _ => self.fail("an escape"),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.position += 1;
        let mut text = String::new();

        loop {
            let rest = std::str::from_utf8(&self.bytes[self.position..]).unwrap();
            let Some(character) = rest.chars().next() else {
                return self.fail("a closing quote");
            };

            match character {
                '"' => {
                    self.position += 1;
                    return Ok(text);
                }
                '\\' => {
                    self.position += 1;
                    text.push(self.escape()?);
                }
                control if (control as u32) < 0x20 => return self.fail("an escaped control"),
                _ => {
                    self.position += character.len_utf8();
                    text.push(character);
                }
            }
        }
    }
}

#[test]
fn the_parser_reads_nested_values() {
    assert_eq!(
        parse(r#" {"a": [1, -2.5e3, true, null], "b": {"c": "\u00e9\ud83d\ude00\n"}} "#),
        Ok(Value::Object(vec![
            (
                "a".to_string(),
                Value::Array(vec![
                    Value::Number("1".to_string()),
                    Value::Number("-2.5e3".to_string()),
                    Value::Bool(true),
                    Value::Null
                ])
            ),
            (
                "b".to_string(),
                Value::Object(vec![("c".to_string(), Value::Text("é😀\n".to_string()))])
            )
        ]))
    );
}

#[test]
fn the_parser_rejects_what_json_does_not_allow() {
    for text in [
        "",
        "{\"a\":1,}",
        "[1,]",
        "{a:1}",
        "{\"a\" 1}",
        "{\"a\":1 \"b\":2}",
        "[1 2]",
        "01",
        "1.",
        "1e",
        "-",
        "\"tab\there\"",
        "\"\\x\"",
        "\"\\u12\"",
        "\"\\ud83d\"",
        "\"\\ud83d\\u0041\"",
        "\"open",
        "tru",
        "{} {}",
        "{\"a\"",
    ] {
        assert!(parse(text).is_err(), "{:?} was accepted", text);
    }
}

#[test]
fn the_parser_reads_the_other_escapes_and_an_empty_object_and_array() {
    assert_eq!(
        parse(r#"[{}, [], "\"\\\/\b\f\r\t", false, 0.5E+2]"#),
        Ok(Value::Array(vec![
            Value::Object(vec![]),
            Value::Array(vec![]),
            Value::Text("\"\\/\u{8}\u{c}\r\t".to_string()),
            Value::Bool(false),
            Value::Number("0.5E+2".to_string())
        ]))
    );
}

fn settings(output: &str, dates: DateOptions) -> Settings {
    let options = Options {
        mode: Some("auto".to_string()),
        output: Some(output.to_string()),
        jobs: Some(1),
        dates,
        ..Options::default()
    };

    Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap()
}

fn samples() -> Vec<(String, String)> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("examples");
    let mut names: Vec<String> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".log"))
        .collect();
    names.sort();

    names
        .into_iter()
        .filter_map(|name| {
            let text = fs::read_to_string(directory.join(&name)).ok()?;

            Some((name, text))
        })
        .collect()
}

fn rendered(contents: &str, output: &str) -> Vec<Value> {
    render_contents_settings(
        contents,
        &settings(output, DateOptions::default()),
        &mut Entry::default(),
    )
    .lines()
    .map(|line| parse(line).unwrap_or_else(|error| panic!("{}: {}", error, line)))
    .collect()
}

fn written_lines(contents: &str) -> Vec<&str> {
    contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

#[test]
fn json_output_of_every_sample_parses_back_to_its_lines_and_tokens() {
    for (name, contents) in samples() {
        let objects = rendered(&contents, "json");
        let texts: Vec<&str> = objects
            .iter()
            .map(|object| object.get("text").unwrap().text())
            .collect();

        assert_eq!(texts, written_lines(&contents), "{}", name);

        for object in &objects {
            let joined: String = object
                .get("tokens")
                .unwrap()
                .items()
                .iter()
                .map(|token| token.get("text").unwrap().text())
                .collect();

            assert_eq!(joined, object.get("text").unwrap().text(), "{}", name);
        }
    }
}

#[test]
fn jsonl_output_of_every_sample_parses_back_to_its_lines() {
    for (name, contents) in samples() {
        let texts: Vec<String> = rendered(&contents, "jsonl")
            .iter()
            .map(|object| object.get("text").unwrap().text().to_string())
            .collect();

        assert_eq!(texts, written_lines(&contents), "{}", name);
    }
}

#[test]
fn json_output_escapes_control_characters_so_they_parse_back() {
    let line = "alert\u{1}\u{1f} \"quoted\" back\\slash\ttab";

    let objects = rendered(line, "json");

    assert_eq!(objects[0].get("text").unwrap().text(), line);
}

const NESTED: &str = r#"{"time":"2023-10-03T12:00:01Z","level":"info","user":{"id":7,"tags":["admin","ops"]},"msg":"login"}"#;

#[test]
fn jsonl_output_keeps_a_nested_object_in_the_record_member() {
    let objects = rendered(NESTED, "jsonl");

    assert_eq!(objects[0].get("record"), Some(&parse(NESTED).unwrap()));
}

#[test]
fn json_output_text_of_a_nested_object_parses_back_to_the_same_object() {
    let objects = rendered(NESTED, "json");

    assert_eq!(parse(objects[0].get("text").unwrap().text()), parse(NESTED));
}

const SYSLOG_LINE: &str =
    "2023-10-03T12:00:01+02:00 web01 sshd[5101]: Accepted publickey for alice";

fn rewritten(output: &str, dates: DateOptions) -> Value {
    render_contents_settings(SYSLOG_LINE, &settings(output, dates), &mut Entry::default())
        .lines()
        .map(|line| parse(line).unwrap())
        .next()
        .unwrap()
}

#[test]
fn jsonl_output_writes_the_timestamp_in_the_zone_asked_for() {
    let object = rewritten(
        "jsonl",
        DateOptions {
            tz: Some("UTC".to_string()),
            ..DateOptions::default()
        },
    );

    assert_eq!(
        object.get("timestamp").unwrap().text(),
        "2023-10-03T10:00:01Z"
    );
}

#[test]
fn json_output_text_and_tokens_carry_the_rewritten_timestamp() {
    let object = rewritten(
        "json",
        DateOptions {
            tz: Some("UTC".to_string()),
            ..DateOptions::default()
        },
    );

    assert!(object
        .get("text")
        .unwrap()
        .text()
        .starts_with("2023-10-03T10:00:01Z web01"));
    assert_eq!(
        object.get("tokens").unwrap().items()[0],
        parse(r#"{"kind":"timestamp","text":"2023-10-03T10:00:01Z"}"#).unwrap()
    );
}

fn stats_report(contents: &str, format: &str) -> String {
    let options = Options {
        mode: Some("httpd".to_string()),
        output: Some("plain".to_string()),
        jobs: Some(1),
        stats: splash::stats::StatsOptions {
            stats: true,
            format: Some(format.to_string()),
            ..Default::default()
        },
        ..Options::default()
    };
    let settings = Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap();
    let mut stats = splash::stats::Stats::default();

    splash::render_contents_counted(contents, &settings, &mut Entry::default(), &mut stats);

    splash::progress(&settings, &stats)
}

fn access_log() -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/examples/nginx_production.log"),
    )
    .unwrap()
}

/// How many lines hold each value a pattern's first group captures, most
/// frequent first, then in value order
fn tally(contents: &str, pattern: &str) -> Vec<(String, usize)> {
    let regex = regex::Regex::new(pattern).unwrap();
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();

    for line in contents.lines() {
        if let Some(caps) = regex.captures(line) {
            *counts.entry(caps[1].to_string()).or_default() += 1;
        }
    }

    let mut counted: Vec<(String, usize)> = counts.into_iter().collect();
    counted.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    counted
}

fn counted(report: &Value, key: &str) -> Vec<(String, usize)> {
    report
        .get(key)
        .unwrap()
        .items()
        .iter()
        .map(|item| {
            let count = match item.get("count").unwrap() {
                Value::Number(count) => count.parse().unwrap(),
                other => panic!("{:?} is not a count", other),
            };

            (item.get("value").unwrap().text().to_string(), count)
        })
        .collect()
}

#[test]
fn the_json_statistics_report_parses_back_to_the_counts_of_the_log() {
    let contents = access_log();
    let report = parse(stats_report(&contents, "json").trim_end()).unwrap();

    let mut statuses = tally(&contents, r#"" (\d{3}) "#);
    statuses.sort_by(|left, right| left.0.cmp(&right.0));

    assert_eq!(counted(&report, "statuses"), statuses);
    assert_eq!(
        counted(&report, "top_addresses"),
        tally(&contents, r"^(\S+) ")
    );
    assert_eq!(
        report.get("lines"),
        Some(&Value::Number(contents.lines().count().to_string()))
    );
}

#[test]
fn the_csv_statistics_report_holds_the_same_status_counts() {
    let contents = access_log();
    let report = stats_report(&contents, "csv");

    let mut statuses = tally(&contents, r#"" (\d{3}) "#);
    statuses.sort_by(|left, right| left.0.cmp(&right.0));

    let rows: Vec<(String, usize)> = report
        .lines()
        .filter_map(|row| row.strip_prefix("status,"))
        .map(|row| {
            let (status, count) = row.split_once(',').unwrap();
            (status.to_string(), count.parse().unwrap())
        })
        .collect();

    assert_eq!(report.lines().next(), Some("metric,key,value"));
    assert_eq!(rows, statuses);
}

fn report_with(stats: splash::stats::StatsOptions) -> (String, String) {
    let options = Options {
        mode: Some("httpd".to_string()),
        output: Some("plain".to_string()),
        jobs: Some(1),
        stats,
        ..Options::default()
    };
    let settings = Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::new(PathBuf::from("unused")),
    )
    .unwrap();
    let mut stats = splash::stats::Stats::default();

    splash::render_contents_counted(&access_log(), &settings, &mut Entry::default(), &mut stats);

    (
        splash::progress(&settings, &stats),
        splash::closing(&settings, &stats),
    )
}

#[test]
fn the_json_summary_parses_back() {
    let (_, closing) = report_with(splash::stats::StatsOptions {
        summary: true,
        format: Some("json".to_string()),
        ..Default::default()
    });

    let summary = parse(closing.trim_end()).unwrap();

    assert_eq!(summary.get("lines"), Some(&Value::Number("10".to_string())));
    assert_eq!(summary.get("errors"), Some(&Value::Number("1".to_string())));
}

#[test]
fn the_json_histogram_parses_back_to_a_count_of_every_line() {
    let (progress, _) = report_with(splash::stats::StatsOptions {
        histogram: Some("1m".to_string()),
        format: Some("json".to_string()),
        ..Default::default()
    });

    let histogram = parse(progress.trim_end()).unwrap();
    let buckets = histogram.get("buckets").unwrap().items();

    assert_eq!(histogram.get("bucket").unwrap().text(), "1m");
    assert_eq!(buckets.len(), 1);
    assert_eq!(
        buckets[0].get("count"),
        Some(&Value::Number("10".to_string()))
    );
}
