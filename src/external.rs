//! Plugins written as TOML files
//!
//! A plugin file names a log format and gives the regular expressions that
//! read its lines. A pattern's named capture groups are named for the color
//! keys their text is styled with, and the rest of the line stays plain:
//!
//! ```toml
//! name = "myapp"
//! version = "1.2.0"
//! description = "MyApp request logs"
//! author = "Platform team"
//!
//! [patterns]
//! request = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) (?P<request>\S+) (?P<duration>\d+ms)$'
//! ```
//!
//! The patterns are tried in file order, and the first that matches reads
//! the line.
use crate::discovery::PluginDiscovery;
use crate::output::{ParsedLine, Token, TokenKind};
use crate::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use crate::registry::{self, PluginRegistry};
use crate::toml::{self, TomlError};
use notify::{Config as WatchConfig, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;

/// The file extension plugin files carry
pub const EXTENSION: &str = "toml";

/// Why a plugin file could not be loaded
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginError {
    Unreadable(String),
    Syntax(TomlError),
    MissingName,
    BuiltinName(String),
    BadVersion(String),
    UnknownKey(String),
    NoPatterns,
    BadPattern {
        name: String,
        error: String,
    },
    UnknownGroup {
        pattern: String,
        group: String,
    },
    BadRequirement(String),
    MissingDependency(String),
    IncompatibleDependency {
        name: String,
        required: PluginVersion,
        found: PluginVersion,
    },
    DependencyCycle(Vec<String>),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::Unreadable(error) => write!(f, "{}", error),
            PluginError::Syntax(error) => write!(f, "{}", error),
            PluginError::MissingName => write!(f, "the plugin has no name"),
            PluginError::BuiltinName(name) => {
                write!(f, "'{}' is the name of a built-in plugin", name)
            }
            PluginError::BadVersion(version) => write!(
                f,
                "invalid version '{}' (expected MAJOR.MINOR.PATCH, such as 1.0.0)",
                version
            ),
            PluginError::UnknownKey(key) => write!(f, "unknown key '{}'", key),
            PluginError::NoPatterns => write!(f, "the plugin has no [patterns]"),
            PluginError::BadPattern { name, error } => {
                write!(
                    f,
                    "pattern '{}' is not a regular expression: {}",
                    name, error
                )
            }
            PluginError::UnknownGroup { pattern, group } => write!(
                f,
                "pattern '{}' names group '{}', which is not a color key",
                pattern, group
            ),
            PluginError::BadRequirement(text) => write!(
                f,
                "invalid requirement '{}' (expected NAME or NAME MAJOR.MINOR.PATCH)",
                text
            ),
            PluginError::MissingDependency(name) => {
                write!(f, "requires plugin '{}', which is not loaded", name)
            }
            PluginError::IncompatibleDependency {
                name,
                required,
                found,
            } => write!(
                f,
                "requires plugin '{}' {}, but version {} is loaded",
                name, required, found
            ),
            PluginError::DependencyCycle(names) => write!(
                f,
                "its requirements form a cycle among {}",
                names.join(", ")
            ),
        }
    }
}

impl std::error::Error for PluginError {}

/// One of a plugin's patterns, with the name the plugin file gives it
#[derive(Debug, Clone)]
pub struct Pattern {
    pub name: String,
    pub regex: Regex,
}

/// A plugin another plugin requires, and the lowest compatible version
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub name: String,
    pub version: Option<PluginVersion>,
}

/// A plugin read from a TOML file
#[derive(Clone)]
pub struct TomlPlugin {
    metadata: PluginMetadata,
    patterns: Vec<Pattern>,
    requires: Vec<Requirement>,
    dependencies: Vec<Arc<dyn Plugin>>,
}

impl TomlPlugin {
    /// Reads a plugin file
    pub fn load(path: &Path) -> Result<TomlPlugin, PluginError> {
        let text = fs::read_to_string(path)
            .map_err(|error| PluginError::Unreadable(format!("{}: {}", path.display(), error)))?;

        TomlPlugin::parse(&text)
    }

    /// Reads the text of a plugin file
    pub fn parse(text: &str) -> Result<TomlPlugin, PluginError> {
        let mut name = None;
        let mut version = PluginVersion::new(1, 0, 0);
        let mut description = String::new();
        let mut author = String::new();
        let mut patterns = Vec::new();
        let mut requires = Vec::new();

        for (key, value) in toml::parse(text).map_err(PluginError::Syntax)? {
            match key.split_once('.') {
                None if key == "name" => name = Some(value),
                None if key == "version" => version = parse_version(&value)?,
                None if key == "description" => description = value,
                None if key == "author" => author = value,
                None if key == "requires" => requires = parse_requirements(&value)?,
                Some(("patterns", pattern)) => patterns.push(compile(pattern, &value)?),
                _ => return Err(PluginError::UnknownKey(key)),
            }
        }

        let name = name.ok_or(PluginError::MissingName)?;

        if registry::is_builtin(&name) {
            return Err(PluginError::BuiltinName(name));
        }

        if patterns.is_empty() {
            return Err(PluginError::NoPatterns);
        }

        Ok(TomlPlugin {
            metadata: PluginMetadata::new(&name, version, &description, &author),
            patterns,
            requires,
            dependencies: Vec::new(),
        })
    }

    /// The plugins this plugin requires, in file order
    pub fn requires(&self) -> &[Requirement] {
        &self.requires
    }

    /// The plugin with each required plugin looked up in a registry, which
    /// reads the lines none of its own patterns read
    pub fn resolve(mut self, registry: &PluginRegistry) -> Result<TomlPlugin, PluginError> {
        self.dependencies = self
            .requires
            .iter()
            .map(|requirement| dependency(registry, requirement))
            .collect::<Result<_, _>>()?;

        Ok(self)
    }

    /// The required plugins found by `resolve`
    pub fn dependencies(&self) -> &[Arc<dyn Plugin>] {
        &self.dependencies
    }

    /// The plugin's patterns, in file order
    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    /// The first pattern that matches a line, with the line's tokens
    pub fn read<'a>(&self, line: &'a str) -> Option<(&Pattern, ParsedLine<'a>)> {
        self.patterns
            .iter()
            .find_map(|pattern| Some((pattern, tokens(&pattern.regex, line)?)))
    }
}

impl Plugin for TomlPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a> {
        if let Some((_, parsed)) = self.read(line) {
            return ParseResult::Parsed(parsed);
        }

        self.dependencies
            .iter()
            .map(|plugin| plugin.parse_line(line))
            .find(|result| matches!(result, ParseResult::Parsed(_)))
            .unwrap_or(ParseResult::NoMatch)
    }
}

impl fmt::Debug for TomlPlugin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TomlPlugin")
            .field("metadata", &self.metadata)
            .field("patterns", &self.patterns)
            .field("requires", &self.requires)
            .finish()
    }
}

/// Reads a comma-separated list of requirements, each `NAME` or
/// `NAME MAJOR.MINOR.PATCH`
fn parse_requirements(text: &str) -> Result<Vec<Requirement>, PluginError> {
    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| {
            let words: Vec<&str> = item.split_whitespace().collect();
            let bad = || PluginError::BadRequirement(item.to_string());

            match words.as_slice() {
                [name] => Ok(Requirement {
                    name: name.to_string(),
                    version: None,
                }),
                [name, version] => Ok(Requirement {
                    name: name.to_string(),
                    version: Some(parse_version(version).map_err(|_| bad())?),
                }),
                _ => Err(bad()),
            }
        })
        .collect()
}

fn dependency(
    registry: &PluginRegistry,
    requirement: &Requirement,
) -> Result<Arc<dyn Plugin>, PluginError> {
    let plugin = registry
        .get(&requirement.name)
        .map_err(|_| PluginError::MissingDependency(requirement.name.clone()))?;

    match &requirement.version {
        Some(required) if !plugin.version().is_compatible_with(required) => {
            Err(PluginError::IncompatibleDependency {
                name: requirement.name.clone(),
                required: required.clone(),
                found: plugin.version().clone(),
            })
        }
        _ => Ok(plugin),
    }
}

fn parse_version(text: &str) -> Result<PluginVersion, PluginError> {
    let parts: Vec<u32> = text
        .split('.')
        .map(|part| part.parse())
        .collect::<Result<_, _>>()
        .map_err(|_| PluginError::BadVersion(text.to_string()))?;

    match parts.as_slice() {
        [major, minor, patch] => Ok(PluginVersion::new(*major, *minor, *patch)),
        _ => Err(PluginError::BadVersion(text.to_string())),
    }
}

/// Compiles a pattern, checking that each named group is a color key
fn compile(name: &str, text: &str) -> Result<Pattern, PluginError> {
    let regex = Regex::new(text).map_err(|error| PluginError::BadPattern {
        name: name.to_string(),
        error: error.to_string(),
    })?;

    if let Some(group) = regex
        .capture_names()
        .flatten()
        .find(|group| TokenKind::from_name(group).is_none())
    {
        return Err(PluginError::UnknownGroup {
            pattern: name.to_string(),
            group: group.to_string(),
        });
    }

    Ok(Pattern {
        name: name.to_string(),
        regex,
    })
}

/// The line split into the text of each named group that matched, styled
/// with its color key, and plain text around them. A group inside another
/// group takes no part.
fn tokens<'a>(regex: &Regex, line: &'a str) -> Option<ParsedLine<'a>> {
    let caps = regex.captures(line)?;

    let mut groups: Vec<(usize, usize, TokenKind)> = regex
        .capture_names()
        .flatten()
        .filter_map(|group| {
            let found = caps.name(group)?;
            let kind = TokenKind::from_name(group)?;

            Some((found.start(), found.end(), kind))
        })
        .collect();

    groups.sort_by_key(|(start, _, _)| *start);

    let mut tokens = Vec::new();
    let mut cursor = 0;

    for (start, end, kind) in groups {
        if start < cursor || start == end {
            continue;
        }

        if start > cursor {
            tokens.push(Token::new(&line[cursor..start], TokenKind::Plain));
        }

        tokens.push(Token::new(&line[start..end], kind));
        cursor = end;
    }

    if cursor < line.len() {
        tokens.push(Token::new(&line[cursor..], TokenKind::Plain));
    }

    Some(ParsedLine::new(tokens))
}

/// The plugin files registered with the plugins every mode is read with,
/// and the name each one registered
#[derive(Debug, Default)]
pub struct PluginSet {
    names: HashMap<PathBuf, String>,
}

impl PluginSet {
    pub fn new() -> PluginSet {
        PluginSet::default()
    }

    /// Loads the plugin files and registers each plugin, returning a report
    /// for each file that could not be loaded. A plugin is registered after
    /// the plugin files it requires. A file that cannot be loaded leaves the
    /// plugin it registered before. The plugin of a file that is gone, or
    /// that now names another plugin, is unregistered.
    pub fn load(&mut self, paths: &[PathBuf]) -> Vec<String> {
        let registry = registry::builtins();
        let mut failures = Vec::new();

        self.names.retain(|path, name| {
            let kept = paths.contains(path);

            if !kept {
                let _ = registry.unregister(name);
            }

            kept
        });

        let mut pending = Vec::new();

        for path in paths {
            match TomlPlugin::load(path) {
                Ok(plugin) => pending.push((path.clone(), plugin)),
                Err(error) => failures.push(failure(path, &error)),
            }
        }

        while !pending.is_empty() {
            let waiting_names: HashSet<String> = pending
                .iter()
                .map(|(_, plugin)| plugin.name().to_string())
                .collect();

            let (ready, waiting): (Vec<_>, Vec<_>) =
                pending.into_iter().partition(|(_, plugin)| {
                    plugin
                        .requires()
                        .iter()
                        .all(|requirement| !waiting_names.contains(&requirement.name))
                });

            if ready.is_empty() {
                let mut names: Vec<String> = waiting_names.into_iter().collect();
                names.sort();

                for (path, _) in &waiting {
                    failures.push(failure(path, &PluginError::DependencyCycle(names.clone())));
                }

                break;
            }

            for (path, plugin) in ready {
                match plugin.resolve(registry) {
                    Ok(plugin) => self.register(path, plugin),
                    Err(error) => failures.push(failure(&path, &error)),
                }
            }

            pending = waiting;
        }

        failures
    }

    fn register(&mut self, path: PathBuf, plugin: TomlPlugin) {
        let registry = registry::builtins();
        let name = plugin.name().to_string();

        if let Some(previous) = self.names.insert(path, name.clone()) {
            let _ = registry.unregister(&previous);
        }

        let _ = registry.unregister(&name);
        let _ = registry.register(Arc::new(plugin));
    }

    /// The name and version of each plugin the files registered, in name
    /// order, as `orders 1.2.0`
    pub fn versions(&self) -> Vec<String> {
        self.names()
            .into_iter()
            .filter_map(|name| {
                let plugin = registry::builtins().get(&name).ok()?;

                Some(format!("{} {}", name, plugin.version()))
            })
            .collect()
    }

    /// The names of the plugins the files registered, in name order
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.names.values().cloned().collect();

        names.sort();
        names
    }
}

fn failure(path: &Path, error: &PluginError) -> String {
    format!("Cannot load plugin {}: {}", path.display(), error)
}

/// Reloads the plugin files whenever a file in the search paths changes,
/// reporting the plugins and versions each reload registered and each file that could not
/// be loaded. Watching stops when the returned watcher is dropped.
pub fn watch(
    discovery: PluginDiscovery,
    mut plugins: PluginSet,
    report: impl Fn(&str) + Send + 'static,
) -> notify::Result<RecommendedWatcher> {
    let (sender, receiver) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(sender, WatchConfig::default())?;

    // A directory that cannot be watched is skipped, as discovery skips one it
    // cannot read
    for path in discovery.search_paths().iter().filter(|path| path.is_dir()) {
        let _ = watcher.watch(path, RecursiveMode::NonRecursive);
    }

    thread::spawn(move || {
        // inotify reports the reads a reload makes, which must not start another
        let changes = receiver
            .iter()
            .filter(|event| !matches!(event, Ok(event) if event.kind.is_access()));

        for _ in changes {
            for failure in plugins.load(&discovery.discover_plugins().unwrap_or_default()) {
                report(&failure);
            }

            report(&format!(
                "Reloaded plugins: {}",
                plugins.versions().join(", ")
            ));
        }
    });

    Ok(watcher)
}

/// How a plugin's patterns read some lines: for each line that is not
/// blank, the pattern that read it and the text of each named group that
/// matched, the required plugin that read it, or that nothing read it, then
/// a count of the lines read
pub struct RegexReport {
    pub text: String,
    pub unmatched: usize,
}

/// Tries a plugin's patterns on each line of `text`
pub fn test_patterns(plugin: &TomlPlugin, text: &str) -> RegexReport {
    let mut report = String::new();
    let mut tried = 0;
    let mut unmatched = 0;

    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        tried += 1;

        let Some(pattern) = plugin
            .patterns()
            .iter()
            .find(|pattern| pattern.regex.is_match(line))
        else {
            match plugin
                .dependencies()
                .iter()
                .find(|dependency| dependency.can_parse(line))
            {
                Some(dependency) => report.push_str(&format!(
                    "{}: read by required plugin {}\n",
                    index + 1,
                    dependency.name()
                )),
                None => {
                    unmatched += 1;
                    report.push_str(&format!("{}: no pattern matched\n   {}\n", index + 1, line));
                }
            }

            continue;
        };

        report.push_str(&format!("{}: {}\n", index + 1, pattern.name));

        let caps = pattern.regex.captures(line).unwrap();

        for group in pattern.regex.capture_names().flatten() {
            if let Some(found) = caps.name(group) {
                report.push_str(&format!("   {:<16} {}\n", group, found.as_str()));
            }
        }
    }

    report.push_str(&format!(
        "{} of {} lines matched\n",
        tried - unmatched,
        tried
    ));

    RegexReport {
        text: report,
        unmatched,
    }
}
