use splash::discovery::PluginDiscovery;
use splash::external::{self, PluginError, PluginSet, TomlPlugin};
use splash::output::TokenKind;
use splash::plugin::{ParseResult, Plugin, PluginVersion};
use splash::registry;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tempfile::TempDir;

const ORDERS: &str = r#"
name = "orders"
version = "2.1.3"
description = "Order service logs"
author = "Platform team"

[patterns]
placed = '^(?P<timestamp>\S+) (?P<level>[A-Z]+) order (?P<transaction>\d+) placed$'
any = '^(?P<timestamp>\S+) (?P<level>[A-Z]+) (?P<message>.*)$'
"#;

fn plugin(text: &str) -> TomlPlugin {
    TomlPlugin::parse(text).unwrap()
}

fn kinds(plugin: &TomlPlugin, line: &str) -> Vec<(TokenKind, String)> {
    match plugin.parse_line(line) {
        ParseResult::Parsed(parsed) => parsed
            .tokens
            .iter()
            .map(|token| (token.kind, token.text.to_string()))
            .collect(),
        _ => panic!("'{}' was not read", line),
    }
}

fn error(text: &str) -> PluginError {
    TomlPlugin::parse(text).unwrap_err()
}

#[test]
fn a_plugin_file_names_and_describes_the_plugin() {
    let orders = plugin(ORDERS);

    assert_eq!(orders.name(), "orders");
    assert_eq!(orders.version(), &PluginVersion::new(2, 1, 3));
    assert_eq!(orders.metadata().description, "Order service logs");
    assert_eq!(orders.metadata().author, "Platform team");
}

#[test]
fn a_plugin_without_a_version_is_version_one() {
    let minimal = plugin("name = \"minimal\"\n[patterns]\nall = '(?P<message>.+)'\n");

    assert_eq!(minimal.version(), &PluginVersion::new(1, 0, 0));
}

#[test]
fn named_groups_are_styled_with_their_color_keys_and_the_rest_is_plain() {
    assert_eq!(
        kinds(&plugin(ORDERS), "12:00:01 INFO order 42 placed"),
        [
            (TokenKind::Timestamp, "12:00:01".to_string()),
            (TokenKind::Plain, " ".to_string()),
            (TokenKind::Level, "INFO".to_string()),
            (TokenKind::Plain, " order ".to_string()),
            (TokenKind::Transaction, "42".to_string()),
            (TokenKind::Plain, " placed".to_string()),
        ]
    );
}

#[test]
fn the_first_pattern_that_matches_reads_the_line() {
    let orders = plugin(ORDERS);

    let (pattern, _) = orders.read("12:00:02 WARN payment retried").unwrap();

    assert_eq!(pattern.name, "any");
}

#[test]
fn text_outside_an_unanchored_match_is_plain() {
    let unanchored = plugin("name = \"unanchored\"\n[patterns]\nid = 'id=(?P<transaction>\\d+)'\n");

    assert_eq!(
        kinds(&unanchored, "before id=7 after"),
        [
            (TokenKind::Plain, "before id=".to_string()),
            (TokenKind::Transaction, "7".to_string()),
            (TokenKind::Plain, " after".to_string()),
        ]
    );
}

#[test]
fn a_group_inside_another_group_and_an_empty_group_take_no_part() {
    let nested = plugin(
        "name = \"nested\"\n[patterns]\nall = '^(?P<request>/(?P<path>\\w+))(?P<status>\\d*) (?P<message>.*)$'\n",
    );

    assert_eq!(
        kinds(&nested, "/api ready"),
        [
            (TokenKind::Request, "/api".to_string()),
            (TokenKind::Plain, " ".to_string()),
            (TokenKind::Message, "ready".to_string()),
        ]
    );
}

#[test]
fn a_line_no_pattern_matches_is_not_read() {
    assert!(matches!(
        plugin(ORDERS).parse_line("no timestamp here"),
        ParseResult::NoMatch
    ));
}

#[test]
fn a_plugin_file_that_is_not_toml_is_an_error() {
    assert_eq!(
        error("name = \"broken\"\n[patterns\n").to_string(),
        "line 2: unterminated table header"
    );
}

#[test]
fn a_plugin_needs_a_name() {
    assert_eq!(error("[patterns]\nall = '.+'\n"), PluginError::MissingName);
    assert_eq!(
        PluginError::MissingName.to_string(),
        "the plugin has no name"
    );
}

#[test]
fn a_plugin_cannot_take_the_name_of_a_built_in_mode() {
    for name in ["syslog", "clf", "auto", "ad-hoc", "k8s"] {
        assert_eq!(
            error(&format!("name = \"{}\"\n[patterns]\nall = '.+'\n", name)),
            PluginError::BuiltinName(name.to_string())
        );
    }

    assert_eq!(
        PluginError::BuiltinName("syslog".to_string()).to_string(),
        "'syslog' is the name of a built-in plugin"
    );
}

#[test]
fn a_version_must_have_three_numbers() {
    for version in ["1.2", "one.two.three", "1.2.3.4"] {
        assert_eq!(
            error(&format!(
                "name = \"versioned\"\nversion = \"{}\"\n[patterns]\nall = '.+'\n",
                version
            )),
            PluginError::BadVersion(version.to_string())
        );
    }

    assert_eq!(
        PluginError::BadVersion("1.2".to_string()).to_string(),
        "invalid version '1.2' (expected MAJOR.MINOR.PATCH, such as 1.0.0)"
    );
}

#[test]
fn an_unknown_key_is_an_error() {
    assert_eq!(
        error("name = \"typo\"\ndescripton = \"x\"\n"),
        PluginError::UnknownKey("descripton".to_string())
    );
    assert_eq!(
        error("name = \"typo\"\n[colors]\nlevel = \"red\"\n"),
        PluginError::UnknownKey("colors.level".to_string())
    );
    assert_eq!(
        PluginError::UnknownKey("colors.level".to_string()).to_string(),
        "unknown key 'colors.level'"
    );
}

#[test]
fn a_plugin_needs_a_pattern() {
    assert_eq!(error("name = \"empty\"\n"), PluginError::NoPatterns);
    assert_eq!(
        PluginError::NoPatterns.to_string(),
        "the plugin has no [patterns]"
    );
}

#[test]
fn a_pattern_must_be_a_regular_expression() {
    let found = error("name = \"bad\"\n[patterns]\nopen = '(unclosed'\n");

    assert!(matches!(&found, PluginError::BadPattern { name, .. } if name == "open"));
    assert!(found
        .to_string()
        .starts_with("pattern 'open' is not a regular expression: "));
}

#[test]
fn a_named_group_must_be_a_color_key() {
    assert_eq!(
        error("name = \"bad\"\n[patterns]\nall = '(?P<colour>.+)'\n"),
        PluginError::UnknownGroup {
            pattern: "all".to_string(),
            group: "colour".to_string()
        }
    );
    assert_eq!(
        PluginError::UnknownGroup {
            pattern: "all".to_string(),
            group: "colour".to_string()
        }
        .to_string(),
        "pattern 'all' names group 'colour', which is not a color key"
    );
}

#[test]
fn a_missing_plugin_file_is_an_error() {
    let found = TomlPlugin::load(Path::new("missing/plugin.toml")).unwrap_err();

    assert!(matches!(found, PluginError::Unreadable(_)));
    assert!(found.to_string().starts_with("missing/plugin.toml: "));
}

fn plugin_file(directory: &Path, file: &str, name: &str, pattern: &str) -> PathBuf {
    let path = directory.join(file);

    fs::write(
        &path,
        format!("name = \"{}\"\n[patterns]\nall = '{}'\n", name, pattern),
    )
    .unwrap();

    path
}

fn reads(name: &str, line: &str) -> Option<Vec<TokenKind>> {
    match registry::builtins().get(name).ok()?.parse_line(line) {
        ParseResult::Parsed(parsed) => Some(parsed.tokens.iter().map(|token| token.kind).collect()),
        _ => None,
    }
}

#[test]
fn loaded_plugins_are_registered_for_every_mode_to_read() {
    let directory = TempDir::new().unwrap();
    let path = plugin_file(
        directory.path(),
        "a.toml",
        "set-registered",
        "^(?P<level>[A-Z]+)$",
    );
    let mut plugins = PluginSet::new();

    let failures = plugins.load(&[path]);

    assert!(failures.is_empty());
    assert_eq!(plugins.names(), ["set-registered"]);
    assert_eq!(
        reads("set-registered", "INFO"),
        Some(vec![TokenKind::Level])
    );
}

#[test]
fn a_file_that_cannot_be_loaded_is_reported_and_leaves_its_old_plugin() {
    let directory = TempDir::new().unwrap();
    let path = plugin_file(
        directory.path(),
        "a.toml",
        "set-kept",
        "^(?P<level>[A-Z]+)$",
    );
    let mut plugins = PluginSet::new();
    plugins.load(std::slice::from_ref(&path));

    fs::write(&path, "name = \"set-kept\"\n").unwrap();
    let failures = plugins.load(std::slice::from_ref(&path));

    assert_eq!(
        failures,
        [format!(
            "Cannot load plugin {}: the plugin has no [patterns]",
            path.display()
        )]
    );
    assert_eq!(reads("set-kept", "INFO"), Some(vec![TokenKind::Level]));
}

#[test]
fn a_reloaded_file_replaces_its_plugin() {
    let directory = TempDir::new().unwrap();
    let path = plugin_file(
        directory.path(),
        "a.toml",
        "set-replaced",
        "^(?P<level>[A-Z]+)$",
    );
    let mut plugins = PluginSet::new();
    plugins.load(std::slice::from_ref(&path));

    plugin_file(
        directory.path(),
        "a.toml",
        "set-replaced",
        "^(?P<message>[A-Z]+)$",
    );
    plugins.load(&[path]);

    assert_eq!(
        reads("set-replaced", "INFO"),
        Some(vec![TokenKind::Message])
    );
}

#[test]
fn a_renamed_plugin_unregisters_its_old_name() {
    let directory = TempDir::new().unwrap();
    let path = plugin_file(directory.path(), "a.toml", "set-old-name", ".+");
    let mut plugins = PluginSet::new();
    plugins.load(std::slice::from_ref(&path));

    plugin_file(directory.path(), "a.toml", "set-new-name", ".+");
    plugins.load(&[path]);

    assert!(!registry::builtins().contains("set-old-name"));
    assert!(registry::builtins().contains("set-new-name"));
    assert_eq!(plugins.names(), ["set-new-name"]);
}

#[test]
fn the_plugin_of_a_removed_file_is_unregistered() {
    let directory = TempDir::new().unwrap();
    let path = plugin_file(directory.path(), "a.toml", "set-removed", ".+");
    let mut plugins = PluginSet::new();
    plugins.load(&[path]);

    plugins.load(&[]);

    assert!(!registry::builtins().contains("set-removed"));
    assert!(plugins.names().is_empty());
}

/// Waits for the first report `wanted` accepts, as one change to a file can
/// be reported as several events, each starting a reload
fn wait_for_report(reports: &mpsc::Receiver<String>, wanted: impl Fn(&str) -> bool) {
    while !wanted(&reports.recv_timeout(Duration::from_secs(30)).unwrap()) {}
}

#[test]
fn watched_plugin_files_are_reloaded_when_one_changes() {
    let directory = TempDir::new().unwrap();
    let path = plugin_file(
        directory.path(),
        "a.toml",
        "set-watched",
        "^(?P<level>[A-Z]+)$",
    );
    let discovery = PluginDiscovery::with_paths(vec![directory.path().to_path_buf()]);
    let mut plugins = PluginSet::new();
    plugins.load(std::slice::from_ref(&path));
    let (sender, reports) = mpsc::channel();

    let _watcher = external::watch(discovery, plugins, move |report| {
        let _ = sender.send(report.to_string());
    })
    .unwrap();

    plugin_file(
        directory.path(),
        "a.toml",
        "set-watched",
        "^(?P<message>[A-Z]+)$",
    );

    wait_for_report(&reports, |report| {
        report == "Reloaded plugins: set-watched 1.0.0"
            && reads("set-watched", "INFO") == Some(vec![TokenKind::Message])
    });
}

#[test]
fn a_watched_file_that_cannot_be_loaded_is_reported() {
    let directory = TempDir::new().unwrap();
    let discovery = PluginDiscovery::with_paths(vec![directory.path().to_path_buf()]);
    let (sender, reports) = mpsc::channel();

    let _watcher = external::watch(discovery, PluginSet::new(), move |report| {
        let _ = sender.send(report.to_string());
    })
    .unwrap();

    let path = directory.path().join("broken.toml");
    fs::write(&path, "name = \"set-broken\"\n").unwrap();
    let expected = format!(
        "Cannot load plugin {}: the plugin has no [patterns]",
        path.display()
    );

    wait_for_report(&reports, |report| report == expected);
}

#[test]
fn the_report_names_the_pattern_and_groups_that_read_each_line() {
    let report = external::test_patterns(
        &plugin(ORDERS),
        "12:00:01 INFO order 42 placed\n\nno timestamp\n",
    );

    assert_eq!(
        report.text,
        "1: placed\n   timestamp        12:00:01\n   level            INFO\n   transaction      42\n3: no pattern matched\n   no timestamp\n1 of 2 lines matched\n"
    );
    assert_eq!(report.unmatched, 1);
}

#[test]
fn the_report_leaves_out_a_group_that_did_not_match() {
    let optional = plugin(
        "name = \"optional\"\n[patterns]\nall = '^(?P<level>[A-Z]+)(?: (?P<message>.+))?$'\n",
    );

    let report = external::test_patterns(&optional, "INFO\n");

    assert_eq!(
        report.text,
        "1: all\n   level            INFO\n1 of 1 lines matched\n"
    );
}

#[test]
fn versions_name_each_registered_plugin_with_its_version() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("a.toml");
    fs::write(
        &path,
        "name = \"set-versioned\"\nversion = \"3.1.4\"\n[patterns]\nall = '.+'\n",
    )
    .unwrap();
    let mut plugins = PluginSet::new();

    plugins.load(&[path]);

    assert_eq!(plugins.versions(), ["set-versioned 3.1.4"]);
}
