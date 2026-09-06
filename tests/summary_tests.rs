use splash::discovery::PluginDiscovery;
use splash::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use splash::plugin_summary;
use splash::registry::PluginRegistry;
use std::path::PathBuf;
use std::sync::Arc;

struct NamedPlugin {
    metadata: PluginMetadata,
}

impl NamedPlugin {
    fn new(name: &str) -> Self {
        Self {
            metadata: PluginMetadata::new(
                name,
                PluginVersion::new(1, 2, 3),
                "Summary plugin",
                "Test",
            ),
        }
    }
}

impl Plugin for NamedPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn parse_line(&self, _line: &str) -> ParseResult {
        ParseResult::NoMatch
    }
}

struct PanickingPlugin {
    metadata: PluginMetadata,
}

impl PanickingPlugin {
    fn new() -> Self {
        Self {
            metadata: PluginMetadata::new(
                "panicking",
                PluginVersion::new(1, 0, 0),
                "Panics when asked for its name",
                "Test",
            ),
        }
    }
}

impl Plugin for PanickingPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn name(&self) -> &str {
        panic!("plugin name is unavailable");
    }

    fn parse_line(&self, _line: &str) -> ParseResult {
        ParseResult::NoMatch
    }
}

fn discovery() -> PluginDiscovery {
    PluginDiscovery::with_paths(vec![PathBuf::from("/opt/splash/plugins")])
}

#[test]
fn an_empty_registry_summary_lists_the_built_in_modes() {
    let summary = plugin_summary(&PluginRegistry::new(), &discovery());

    assert_eq!(
        summary,
        "Available Plugins:\n\
         ==================\n\
         No plugins currently registered.\n\
         \n\
         Built-in modes:\n\
         \x20 - clf (Common Log Format)\n\
         \x20 - ad-hoc (General pattern matching)\n\
         \n\
         Plugin discovery paths:\n\
         \x20 /opt/splash/plugins\n"
    );
}

#[test]
fn a_registered_plugin_appears_with_its_version() {
    let registry = PluginRegistry::new();
    registry
        .register(Arc::new(NamedPlugin::new("syslog")))
        .unwrap();

    let summary = plugin_summary(&registry, &discovery());

    assert!(summary.contains("  syslog v1.2.3\n"));
    assert!(!summary.contains("No plugins currently registered"));
}

#[test]
fn a_poisoned_registry_is_reported_in_the_summary() {
    let registry = Arc::new(PluginRegistry::new());
    let poisoner = Arc::clone(&registry);

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::thread::spawn(move || {
        poisoner.register(Arc::new(PanickingPlugin::new())).ok();
    })
    .join();
    std::panic::set_hook(previous_hook);

    let summary = plugin_summary(&registry, &discovery());

    assert!(summary.contains("Error listing plugins: Registry is locked for modifications\n"));
}

#[test]
fn the_summary_lists_every_discovery_path() {
    let discovery = PluginDiscovery::with_paths(vec![
        PathBuf::from("/opt/splash/plugins"),
        PathBuf::from("/usr/share/splash"),
    ]);

    let summary = plugin_summary(&PluginRegistry::new(), &discovery);

    assert!(
        summary.ends_with("Plugin discovery paths:\n  /opt/splash/plugins\n  /usr/share/splash\n")
    );
}
