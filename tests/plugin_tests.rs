use splash::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};

#[test]
fn test_plugin_version_display() {
    let version = PluginVersion::new(1, 2, 3);
    assert_eq!(version.to_string(), "1.2.3");
}

#[test]
fn test_plugin_version_compatibility() {
    let v1_2_3 = PluginVersion::new(1, 2, 3);
    let v1_2_4 = PluginVersion::new(1, 2, 4);
    let v1_3_0 = PluginVersion::new(1, 3, 0);
    let v2_0_0 = PluginVersion::new(2, 0, 0);

    // Same version is compatible
    assert!(v1_2_3.is_compatible_with(&v1_2_3));

    // Higher patch version is compatible
    assert!(v1_2_4.is_compatible_with(&v1_2_3));
    assert!(!v1_2_3.is_compatible_with(&v1_2_4));

    // Higher minor version is compatible
    assert!(v1_3_0.is_compatible_with(&v1_2_3));
    assert!(!v1_2_3.is_compatible_with(&v1_3_0));

    // Different major version is not compatible
    assert!(!v2_0_0.is_compatible_with(&v1_2_3));
    assert!(!v1_2_3.is_compatible_with(&v2_0_0));
}

#[test]
fn test_plugin_metadata_creation() {
    let version = PluginVersion::new(1, 0, 0);
    let metadata = PluginMetadata::new(
        "test-plugin",
        version.clone(),
        "A test plugin",
        "Test Author",
    );

    assert_eq!(metadata.name, "test-plugin");
    assert_eq!(metadata.version, version);
    assert_eq!(metadata.description, "A test plugin");
    assert_eq!(metadata.author, "Test Author");
}

// Mock plugin for testing
struct MockPlugin {
    metadata: PluginMetadata,
}

impl MockPlugin {
    fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                "mock",
                PluginVersion::new(1, 0, 0),
                "Mock plugin for testing",
                "Test",
            ),
        }
    }
}

impl Plugin for MockPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line(&self, line: &str) -> ParseResult {
        if line.starts_with("MOCK:") {
            ParseResult::Parsed(line.to_string())
        } else {
            ParseResult::NoMatch
        }
    }
}

#[test]
fn test_plugin_trait_basic() {
    let plugin = MockPlugin::new();

    assert_eq!(plugin.name(), "mock");
    assert_eq!(plugin.version().to_string(), "1.0.0");
}

#[test]
fn test_plugin_can_parse() {
    let plugin = MockPlugin::new();

    assert!(plugin.can_parse("MOCK: test line"));
    assert!(!plugin.can_parse("OTHER: test line"));
}

#[test]
fn test_plugin_detect_format() {
    let plugin = MockPlugin::new();

    let all_match = vec!["MOCK: line1", "MOCK: line2", "MOCK: line3"];
    assert_eq!(plugin.detect_format(&all_match), 1.0);

    let half_match = vec!["MOCK: line1", "OTHER: line2"];
    assert_eq!(plugin.detect_format(&half_match), 0.5);

    let no_match = vec!["OTHER: line1", "OTHER: line2"];
    assert_eq!(plugin.detect_format(&no_match), 0.0);

    let empty: Vec<&str> = vec![];
    assert_eq!(plugin.detect_format(&empty), 0.0);
}
