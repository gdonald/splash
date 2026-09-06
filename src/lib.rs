pub mod discovery;
pub mod output;
pub mod parser;
pub mod plugin;
pub mod registry;
pub mod tui;
pub mod viewer;

use crate::discovery::PluginDiscovery;
use crate::output::OutputMode;
use crate::registry::PluginRegistry;

/// Renders a chunk of log text, producing one output line per parsed line.
///
/// Lines the mode has nothing to say about, such as blank lines or non-CLF
/// lines in `clf` mode, are dropped.
pub fn render_contents(contents: &str, mode: &str, output_mode: OutputMode) -> String {
    let mut rendered = String::new();

    for line in render_lines(contents, mode, output_mode) {
        rendered.push_str(&line);
        rendered.push('\n');
    }

    rendered
}

/// Renders a chunk of log text as one string per parsed line.
pub fn render_lines(contents: &str, mode: &str, output_mode: OutputMode) -> Vec<String> {
    contents
        .lines()
        .filter_map(|line| parser::parse_line(line, mode))
        .map(|parsed| output_mode.render(&parsed))
        .collect()
}

/// Describes the registered plugins and where splash looks for more of them.
pub fn plugin_summary(registry: &PluginRegistry, discovery: &PluginDiscovery) -> String {
    let mut summary = String::from("Available Plugins:\n==================\n");

    match registry.describe_plugins() {
        Ok(plugins) if plugins.is_empty() => {
            summary.push_str("No plugins currently registered.\n");
            summary.push_str("\nBuilt-in modes:\n");
            summary.push_str("  - clf (Common Log Format)\n");
            summary.push_str("  - ad-hoc (General pattern matching)\n");
        }
        Ok(plugins) => {
            for description in plugins {
                summary.push_str(&format!("  {}\n", description));
            }
        }
        Err(e) => summary.push_str(&format!("Error listing plugins: {}\n", e)),
    }

    summary.push_str("\nPlugin discovery paths:\n");
    for path in discovery.search_paths() {
        summary.push_str(&format!("  {}\n", path.display()));
    }

    summary
}
