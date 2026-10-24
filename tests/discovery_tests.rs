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

    let toml_file = temp_dir.path().join("myapp.toml");
    let upper_file = temp_dir.path().join("other.TOML");
    let so_file = temp_dir.path().join("plugin.so");
    let txt_file = temp_dir.path().join("readme.txt");
    let bare_file = temp_dir.path().join("README");

    for file in [&toml_file, &upper_file, &so_file, &txt_file, &bare_file] {
        File::create(file).unwrap();
    }

    assert!(discovery.is_plugin_file(&toml_file));
    assert!(discovery.is_plugin_file(&upper_file));
    assert!(!discovery.is_plugin_file(&so_file));
    assert!(!discovery.is_plugin_file(&txt_file));
    assert!(!discovery.is_plugin_file(&bare_file));
    assert!(!discovery.is_plugin_file(temp_dir.path()));
}

#[test]
fn test_discover_plugins() {
    let temp_dir = TempDir::new().unwrap();
    let plugin_dir = temp_dir.path().join("plugins");
    create_dir_all(&plugin_dir).unwrap();

    // Create some plugin files
    File::create(plugin_dir.join("plugin2.toml")).unwrap();
    File::create(plugin_dir.join("plugin1.toml")).unwrap();
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

    assert_eq!(plugin_names, ["plugin1.toml", "plugin2.toml"]);
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

    File::create(plugin_dir.join("myapp.toml")).unwrap();

    let discovery = PluginDiscovery::with_paths(vec![plugin_dir.clone()]);

    let myapp = discovery.find_plugin("myapp").unwrap();
    assert_eq!(
        myapp.unwrap().file_name().unwrap().to_str().unwrap(),
        "myapp.toml"
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

    File::create(dir1.join("plugin1.toml")).unwrap();
    File::create(dir2.join("plugin2.toml")).unwrap();

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

#[cfg(unix)]
#[test]
fn test_a_directory_that_cannot_be_read_is_skipped() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().unwrap();
    let locked = temp_dir.path().join("locked");
    create_dir_all(&locked).unwrap();
    File::create(locked.join("myapp.toml")).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    let plugins = PluginDiscovery::with_paths(vec![locked.clone()]).discover_plugins();

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(plugins.unwrap(), Vec::<PathBuf>::new());
}

#[test]
fn test_a_file_given_as_a_search_path_is_skipped() {
    let temp_dir = TempDir::new().unwrap();
    let file = temp_dir.path().join("myapp.toml");
    File::create(&file).unwrap();

    let plugins = PluginDiscovery::with_paths(vec![file]).discover_plugins();

    assert_eq!(plugins.unwrap(), Vec::<PathBuf>::new());
}
