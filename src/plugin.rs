/// Plugin system for log format parsers
///
/// This module provides the plugin trait and infrastructure for implementing
/// custom log format parsers that can be dynamically loaded and registered.
use std::fmt;

/// Version information for a plugin
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct PluginVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[allow(dead_code)]
impl PluginVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Check if this version is compatible with another version
    /// Compatible if major versions match and this version >= other version
    pub fn is_compatible_with(&self, other: &PluginVersion) -> bool {
        if self.major != other.major {
            return false;
        }
        if self.minor < other.minor {
            return false;
        }
        if self.minor == other.minor && self.patch < other.patch {
            return false;
        }
        true
    }
}

impl fmt::Display for PluginVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Metadata about a plugin
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PluginMetadata {
    pub name: String,
    pub version: PluginVersion,
    pub description: String,
    pub author: String,
}

#[allow(dead_code)]
impl PluginMetadata {
    pub fn new(name: &str, version: PluginVersion, description: &str, author: &str) -> Self {
        Self {
            name: name.to_string(),
            version,
            description: description.to_string(),
            author: author.to_string(),
        }
    }
}

/// Result of parsing a log line
#[allow(dead_code)]
pub enum ParseResult {
    /// Successfully parsed with colorized output
    Parsed(String),
    /// Line doesn't match this plugin's format
    NoMatch,
    /// Error occurred during parsing
    Error(String),
}

/// Trait that all log format plugins must implement
#[allow(dead_code)]
pub trait Plugin: Send + Sync {
    /// Returns metadata about this plugin
    fn metadata(&self) -> &PluginMetadata;

    /// Returns the name of this plugin (convenience method)
    fn name(&self) -> &str {
        &self.metadata().name
    }

    /// Returns the version of this plugin (convenience method)
    fn version(&self) -> &PluginVersion {
        &self.metadata().version
    }

    /// Attempts to parse a single log line
    /// Returns ParseResult indicating success, no match, or error
    fn parse_line(&self, line: &str) -> ParseResult;

    /// Returns true if this plugin can likely parse the given line
    /// Used for auto-detection. Default implementation tries to parse.
    fn can_parse(&self, line: &str) -> bool {
        matches!(self.parse_line(line), ParseResult::Parsed(_))
    }

    /// Returns a confidence score (0.0 to 1.0) that this plugin can parse the given lines
    /// Higher score means more confident. Used for auto-detection.
    fn detect_format(&self, sample_lines: &[&str]) -> f32 {
        if sample_lines.is_empty() {
            return 0.0;
        }

        let matches = sample_lines
            .iter()
            .filter(|line| self.can_parse(line))
            .count();

        matches as f32 / sample_lines.len() as f32
    }
}
