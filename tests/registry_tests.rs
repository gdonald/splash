use splash::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use splash::registry::{PluginRegistry, RegistryError};
use std::sync::Arc;

struct MockPlugin {
    metadata: PluginMetadata,
}

impl MockPlugin {
    fn new(name: &str, major: u32, minor: u32, patch: u32) -> Self {
        Self {
            metadata: PluginMetadata::new(
                name,
                PluginVersion::new(major, minor, patch),
                "Mock plugin",
                "Test",
            ),
        }
    }
}

impl Plugin for MockPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line(&self, _line: &str) -> ParseResult {
        ParseResult::NoMatch
    }
}

#[test]
fn test_registry_new() {
    let registry = PluginRegistry::new();
    assert_eq!(registry.count(), 0);
}

#[test]
fn test_registry_register() {
    let registry = PluginRegistry::new();
    let plugin = Arc::new(MockPlugin::new("test", 1, 0, 0));

    assert!(registry.register(plugin).is_ok());
    assert_eq!(registry.count(), 1);
    assert!(registry.contains("test"));
}

#[test]
fn test_registry_duplicate_registration() {
    let registry = PluginRegistry::new();
    let plugin1 = Arc::new(MockPlugin::new("test", 1, 0, 0));
    let plugin2 = Arc::new(MockPlugin::new("test", 1, 0, 0));

    assert!(registry.register(plugin1).is_ok());
    assert_eq!(
        registry.register(plugin2),
        Err(RegistryError::PluginAlreadyRegistered("test".to_string()))
    );
}

#[test]
fn test_registry_get() {
    let registry = PluginRegistry::new();
    let plugin = Arc::new(MockPlugin::new("test", 1, 0, 0));

    registry.register(plugin).unwrap();

    let retrieved = registry.get("test");
    assert!(retrieved.is_ok());
    assert_eq!(retrieved.unwrap().name(), "test");
}

#[test]
fn test_registry_get_not_found() {
    let registry = PluginRegistry::new();
    let result = registry.get("nonexistent");
    assert!(result.is_err());
    match result {
        Err(RegistryError::PluginNotFound(name)) => {
            assert_eq!(name, "nonexistent");
        }
        _ => panic!("Expected PluginNotFound error"),
    }
}

#[test]
fn test_registry_unregister() {
    let registry = PluginRegistry::new();
    let plugin = Arc::new(MockPlugin::new("test", 1, 0, 0));

    registry.register(plugin).unwrap();
    assert_eq!(registry.count(), 1);

    assert!(registry.unregister("test").is_ok());
    assert_eq!(registry.count(), 0);
    assert!(!registry.contains("test"));
}

#[test]
fn test_registry_list_plugins() {
    let registry = PluginRegistry::new();
    let plugin1 = Arc::new(MockPlugin::new("plugin1", 1, 0, 0));
    let plugin2 = Arc::new(MockPlugin::new("plugin2", 1, 0, 0));

    registry.register(plugin1).unwrap();
    registry.register(plugin2).unwrap();

    let mut plugins = registry.list_plugins().unwrap();
    plugins.sort();

    assert_eq!(plugins, vec!["plugin1", "plugin2"]);
}

#[test]
fn test_registry_disable_enable() {
    let registry = PluginRegistry::new();
    let plugin = Arc::new(MockPlugin::new("test", 1, 0, 0));

    registry.register(plugin).unwrap();

    assert!(!registry.is_disabled("test"));
    assert!(registry.disable_plugin("test").is_ok());
    assert!(registry.is_disabled("test"));
    assert!(registry.enable_plugin("test").is_ok());
    assert!(!registry.is_disabled("test"));
}

#[test]
fn test_registry_list_enabled() {
    let registry = PluginRegistry::new();
    let plugin1 = Arc::new(MockPlugin::new("plugin1", 1, 0, 0));
    let plugin2 = Arc::new(MockPlugin::new("plugin2", 1, 0, 0));

    registry.register(plugin1).unwrap();
    registry.register(plugin2).unwrap();

    registry.disable_plugin("plugin1").unwrap();

    let enabled = registry.list_enabled_plugins().unwrap();
    assert_eq!(enabled, vec!["plugin2"]);
}

#[test]
fn test_registry_version_verification() {
    let registry = PluginRegistry::new();
    let plugin = Arc::new(MockPlugin::new("test", 1, 2, 3));

    registry.register(plugin).unwrap();

    // Compatible versions
    assert!(registry
        .verify_version("test", &PluginVersion::new(1, 2, 3))
        .is_ok());
    assert!(registry
        .verify_version("test", &PluginVersion::new(1, 2, 0))
        .is_ok());
    assert!(registry
        .verify_version("test", &PluginVersion::new(1, 0, 0))
        .is_ok());

    // Incompatible versions
    assert!(registry
        .verify_version("test", &PluginVersion::new(1, 3, 0))
        .is_err());
    assert!(registry
        .verify_version("test", &PluginVersion::new(2, 0, 0))
        .is_err());
}
