use splash::discovery::{DiscoveryError, PluginDiscovery};
use std::fs::{create_dir_all, File};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_discovery_new() {
    let discovery = PluginDiscovery::new();
    assert!(!discovery.search_paths().is_empty());
}

#[test]
fn test_discovery_with_custom_paths() {
    let paths = vec![
        PathBuf::from("/custom/path1"),
        PathBuf::from("/custom/path2"),
    ];
    let discovery = PluginDiscovery::with_paths(paths.clone());
    assert_eq!(discovery.search_paths(), &paths);
}

#[test]
fn test_add_custom_path() {
    let mut discovery = PluginDiscovery::with_paths(vec![]);
    assert_eq!(discovery.search_paths().len(), 0);

    discovery.add_path("/custom/path");
    assert_eq!(discovery.search_paths().len(), 1);
    assert_eq!(discovery.search_paths()[0], PathBuf::from("/custom/path"));
}

#[test]
fn test_is_plugin_file() {
    let temp_dir = TempDir::new().unwrap();
    let discovery = PluginDiscovery::new();

    // Create test files
    let so_file = temp_dir.path().join("plugin.so");
    let dylib_file = temp_dir.path().join("plugin.dylib");
    let dll_file = temp_dir.path().join("plugin.dll");
    let txt_file = temp_dir.path().join("readme.txt");

    File::create(&so_file).unwrap();
    File::create(&dylib_file).unwrap();
    File::create(&dll_file).unwrap();
    File::create(&txt_file).unwrap();

    assert!(discovery.is_plugin_file(&so_file));
    assert!(discovery.is_plugin_file(&dylib_file));
    assert!(discovery.is_plugin_file(&dll_file));
    assert!(!discovery.is_plugin_file(&txt_file));
}

#[test]
fn test_discover_plugins() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("plugins");
    create_dir_all(&plugin_dir).unwrap();

    // Create some plugin files
    File::create(plugin_dir.join("plugin1.so")).unwrap();
    File::create(plugin_dir.join("plugin2.dylib")).unwrap();
    File::create(plugin_dir.join("readme.txt")).unwrap();

    let discovery = PluginDiscovery::with_paths(vec![plugin_dir.clone()]);
    let plugins = discovery.discover_plugins().unwrap();

    assert_eq!(plugins.len(), 2);

    // Check that we found the right files
    let plugin_names: Vec<String> = plugins
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .collect();

    assert!(plugin_names.contains(&"plugin1.so".to_string()));
    assert!(plugin_names.contains(&"plugin2.dylib".to_string()));
}

#[test]
fn test_discover_plugins_nonexistent_dir() {
    let discovery = PluginDiscovery::with_paths(vec![PathBuf::from("/nonexistent/path")]);
    let result = discovery.discover_plugins();

    // Should return empty list, not error, for nonexistent directories
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[test]
fn test_find_plugin() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("plugins");
    create_dir_all(&plugin_dir).unwrap();

    // Create plugin files with different naming conventions
    File::create(plugin_dir.join("syslog.so")).unwrap();
    File::create(plugin_dir.join("libapache.dylib")).unwrap();

    let discovery = PluginDiscovery::with_paths(vec![plugin_dir.clone()]);

    // Find by exact name
    let syslog = discovery.find_plugin("syslog").unwrap();
    assert!(syslog.is_some());
    assert_eq!(
        syslog.unwrap().file_name().unwrap().to_str().unwrap(),
        "syslog.so"
    );

    // Find by name without lib prefix
    let apache = discovery.find_plugin("apache").unwrap();
    assert!(apache.is_some());
    assert_eq!(
        apache.unwrap().file_name().unwrap().to_str().unwrap(),
        "libapache.dylib"
    );

    // Non-existent plugin
    let nonexistent = discovery.find_plugin("nonexistent").unwrap();
    assert!(nonexistent.is_none());
}

#[test]
fn test_discover_from_multiple_paths() {
    let temp_dir = TempDir::new().unwrap();
    let dir1 = temp_dir.path().join("plugins1");
    let dir2 = temp_dir.path().join("plugins2");

    create_dir_all(&dir1).unwrap();
    create_dir_all(&dir2).unwrap();

    File::create(dir1.join("plugin1.so")).unwrap();
    File::create(dir2.join("plugin2.so")).unwrap();

    let discovery = PluginDiscovery::with_paths(vec![dir1, dir2]);
    let plugins = discovery.discover_plugins().unwrap();

    assert_eq!(plugins.len(), 2);
}

#[test]
fn test_directory_not_found_error_names_the_directory() {
    let error = DiscoveryError::DirectoryNotFound(PathBuf::from("/opt/splash/plugins"));

    assert_eq!(
        error.to_string(),
        "Plugin directory not found: /opt/splash/plugins"
    );
}

#[test]
fn test_permission_denied_error_names_the_directory() {
    let error = DiscoveryError::PermissionDenied(PathBuf::from("/root/plugins"));

    assert_eq!(
        error.to_string(),
        "Permission denied accessing: /root/plugins"
    );
}

#[test]
fn test_io_error_reports_the_underlying_failure() {
    let error = DiscoveryError::IoError(std::io::Error::other("disk fell over"));

    assert_eq!(
        error.to_string(),
        "IO error during discovery: disk fell over"
    );
}

#[test]
fn test_an_io_error_converts_into_a_discovery_error() {
    let error: DiscoveryError = std::io::Error::other("disk fell over").into();

    assert!(matches!(error, DiscoveryError::IoError(_)));
}

#[test]
fn test_a_discovery_error_is_a_standard_error() {
    let error = DiscoveryError::DirectoryNotFound(PathBuf::from("/opt/splash/plugins"));
    let as_error: &dyn std::error::Error = &error;

    assert!(as_error.source().is_none());
}

#[test]
fn test_default_discovery_matches_a_new_discovery() {
    assert_eq!(
        PluginDiscovery::default().search_paths(),
        PluginDiscovery::new().search_paths()
    );
}

#[test]
fn test_a_directory_is_not_a_plugin_file() {
    let temp = TempDir::new().unwrap();
    let discovery = PluginDiscovery::with_paths(vec![temp.path().to_path_buf()]);

    assert!(!discovery.is_plugin_file(temp.path()));
}

#[test]
fn test_a_file_without_an_extension_is_not_a_plugin_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("plugin");
    File::create(&path).unwrap();
    let discovery = PluginDiscovery::with_paths(vec![temp.path().to_path_buf()]);

    assert!(!discovery.is_plugin_file(&path));
}

#[test]
fn test_discovery_skips_a_search_path_that_is_a_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("not-a-directory");
    File::create(&path).unwrap();
    let discovery = PluginDiscovery::with_paths(vec![path]);

    assert_eq!(discovery.discover_plugins().unwrap(), Vec::<PathBuf>::new());
}
