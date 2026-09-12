use splash::config::{Config, Profiles};
use splash::discovery::PluginDiscovery;
use splash::plugin::{ParseResult, Plugin, PluginMetadata, PluginVersion};
use splash::registry::PluginRegistry;
use splash::theme::{Theme, PRESETS};
use splash::{plugin_summary, profile_summary, theme_summary};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

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
    let summary = plugin_summary(&PluginRegistry::new(), &discovery(), &Config::default());

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

    let summary = plugin_summary(&registry, &discovery(), &Config::default());

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

    let summary = plugin_summary(&registry, &discovery(), &Config::default());

    assert!(summary.contains("Error listing plugins: Registry is locked for modifications\n"));
}

#[test]
fn the_summary_lists_every_discovery_path() {
    let discovery = PluginDiscovery::with_paths(vec![
        PathBuf::from("/opt/splash/plugins"),
        PathBuf::from("/usr/share/splash"),
    ]);

    let summary = plugin_summary(&PluginRegistry::new(), &discovery, &Config::default());

    assert!(
        summary.ends_with("Plugin discovery paths:\n  /opt/splash/plugins\n  /usr/share/splash\n")
    );
}

fn configured() -> Config {
    Config::parse(
        "[plugins.syslog]\nfacility = \"cyan\"\n\n[plugins.squid]\nenabled = \"false\"\n",
        "test.toml",
    )
    .unwrap()
}

#[test]
fn the_summary_lists_the_settings_configured_for_each_plugin() {
    let summary = plugin_summary(&PluginRegistry::new(), &discovery(), &configured());

    assert!(summary.ends_with(
        "Plugin configuration:\n  squid (disabled)\n    enabled = false\n  syslog (enabled)\n    facility = cyan\n"
    ));
}

#[test]
fn the_summary_has_no_configuration_section_without_configured_plugins() {
    let summary = plugin_summary(&PluginRegistry::new(), &discovery(), &Config::default());

    assert!(!summary.contains("Plugin configuration"));
}

#[test]
fn the_profile_summary_lists_the_saved_profiles() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());
    profiles.save("night", &Theme::dark()).unwrap();

    let summary = profile_summary(&profiles);

    assert!(summary.starts_with("Saved Color Profiles:\n=====================\n  night\n"));
    assert!(summary.ends_with(&format!(
        "\nProfile directory:\n  {}\n",
        profiles.root().display()
    )));
}

#[test]
fn the_profile_summary_says_when_nothing_is_saved() {
    let home = TempDir::new().unwrap();

    let summary = profile_summary(&Profiles::in_home(home.path()));

    assert!(summary.contains("No color profiles saved.\n"));
}

#[test]
fn the_theme_summary_lists_every_preset() {
    let summary = theme_summary();

    assert!(summary.starts_with("Available Themes:\n=================\n"));

    for name in PRESETS {
        assert!(
            summary.contains(&format!("  {}\n", name)),
            "missing {}",
            name
        );
    }
}
