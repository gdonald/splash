//! A reader for the subset of TOML splash configuration uses
//!
//! Config files are tables of string keys and scalar values, so the reader
//! handles comments, `[table]` and `[table.sub]` headers, quoted strings, and
//! bare scalars. Every value is returned as text, paired with its full dotted
//! key path.
use std::fmt;

/// Error returned when a config file cannot be read
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for TomlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TomlError {}

/// Reads a config file into `(dotted key, value)` pairs in file order
pub fn parse(text: &str) -> Result<Vec<(String, String)>, TomlError> {
    let mut entries = Vec::new();
    let mut table = String::new();

    for (index, raw_line) in text.lines().enumerate() {
        let number = index + 1;
        let line = strip_comment(raw_line.trim());

        if line.is_empty() {
            continue;
        }

        if let Some(header) = line.strip_prefix('[') {
            table = parse_header(header, number)?;
            continue;
        }

        let (key, value) = parse_pair(line, number)?;

        let path = if table.is_empty() {
            key
        } else {
            format!("{}.{}", table, key)
        };

        entries.push((path, value));
    }

    Ok(entries)
}

fn parse_header(header: &str, number: usize) -> Result<String, TomlError> {
    let name = header.strip_suffix(']').ok_or_else(|| TomlError {
        line: number,
        message: "unterminated table header".to_string(),
    })?;

    let name = name.trim();

    if name.is_empty() || name.split('.').any(|segment| segment.trim().is_empty()) {
        return Err(TomlError {
            line: number,
            message: "empty table name".to_string(),
        });
    }

    Ok(name
        .split('.')
        .map(|segment| segment.trim())
        .collect::<Vec<&str>>()
        .join("."))
}

fn parse_pair(line: &str, number: usize) -> Result<(String, String), TomlError> {
    let (key, value) = line.split_once('=').ok_or_else(|| TomlError {
        line: number,
        message: format!("expected a key = value pair, found '{}'", line),
    })?;

    let key = unquote(key.trim());

    if key.is_empty() {
        return Err(TomlError {
            line: number,
            message: "empty key".to_string(),
        });
    }

    Ok((key, parse_value(value.trim(), number)?))
}

fn parse_value(value: &str, number: usize) -> Result<String, TomlError> {
    if value.is_empty() {
        return Err(TomlError {
            line: number,
            message: "missing value".to_string(),
        });
    }

    if !value.starts_with('"') {
        return Ok(value.to_string());
    }

    let mut characters = value.chars().skip(1);
    let mut parsed = String::new();

    loop {
        let character = characters.next().ok_or_else(|| TomlError {
            line: number,
            message: "unterminated string".to_string(),
        })?;

        match character {
            '"' => return Ok(parsed),
            '\\' => {
                let escaped = characters.next().ok_or_else(|| TomlError {
                    line: number,
                    message: "unterminated string".to_string(),
                })?;

                match escaped {
                    'n' => parsed.push('\n'),
                    't' => parsed.push('\t'),
                    'r' => parsed.push('\r'),
                    '"' => parsed.push('"'),
                    '\\' => parsed.push('\\'),
                    other => {
                        return Err(TomlError {
                            line: number,
                            message: format!("unknown escape '\\{}'", other),
                        })
                    }
                }
            }
            other => parsed.push(other),
        }
    }
}

/// Removes a trailing comment, leaving `#` inside a quoted string alone
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match character {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '#' if !in_string => return line[..index].trim_end(),
            _ => {}
        }
    }

    line
}

fn unquote(key: &str) -> String {
    key.strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(key)
        .to_string()
}
