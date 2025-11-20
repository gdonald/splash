use std::fs;
/// Plugin discovery system
///
/// This module handles discovering and loading plugins from various directories
/// including system paths and user-specific locations.
use std::path::{Path, PathBuf};

/// Error types for plugin discovery
#[derive(Debug)]
#[allow(dead_code)]
pub enum DiscoveryError {
    DirectoryNotFound(PathBuf),
    PermissionDenied(PathBuf),
    IoError(std::io::Error),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::DirectoryNotFound(path) => {
                write!(f, "Plugin directory not found: {}", path.display())
            }
            DiscoveryError::PermissionDenied(path) => {
                write!(f, "Permission denied accessing: {}", path.display())
            }
            DiscoveryError::IoError(e) => write!(f, "IO error during discovery: {}", e),
        }
    }
}

impl std::error::Error for DiscoveryError {}

impl From<std::io::Error> for DiscoveryError {
    fn from(error: std::io::Error) -> Self {
        DiscoveryError::IoError(error)
    }
}

/// Manages plugin discovery from multiple sources
#[allow(dead_code)]
pub struct PluginDiscovery {
    search_paths: Vec<PathBuf>,
}

#[allow(dead_code)]
impl PluginDiscovery {
    /// Creates a new plugin discovery manager with default search paths
    pub fn new() -> Self {
        let mut discovery = Self {
            search_paths: Vec::new(),
        };

        // Add default search paths
        discovery.add_default_paths();
        discovery
    }

    /// Creates a plugin discovery manager with custom search paths
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self {
            search_paths: paths,
        }
    }

    /// Adds default plugin search paths
    fn add_default_paths(&mut self) {
        // User-specific plugin directory: ~/.splash/plugins/
        if let Some(home) = dirs::home_dir() {
            self.search_paths.push(home.join(".splash").join("plugins"));
        }

        // System-wide plugin directories
        #[cfg(unix)]
        {
            self.search_paths
                .push(PathBuf::from("/usr/local/lib/splash/plugins"));
            self.search_paths
                .push(PathBuf::from("/usr/lib/splash/plugins"));
        }

        #[cfg(windows)]
        {
            if let Some(program_files) = std::env::var_os("ProgramFiles") {
                let mut path = PathBuf::from(program_files);
                path.push("splash");
                path.push("plugins");
                self.search_paths.push(path);
            }
        }
    }

    /// Adds a custom search path
    pub fn add_path<P: AsRef<Path>>(&mut self, path: P) {
        self.search_paths.push(path.as_ref().to_path_buf());
    }

    /// Returns all configured search paths
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    /// Discovers plugin files in all search paths
    /// Returns a list of paths to potential plugin files
    pub fn discover_plugins(&self) -> Result<Vec<PathBuf>, DiscoveryError> {
        let mut plugin_files = Vec::new();

        for search_path in &self.search_paths {
            if !search_path.exists() {
                // Skip non-existent paths silently
                continue;
            }

            if !search_path.is_dir() {
                continue;
            }

            // Read directory entries
            let entries = match fs::read_dir(search_path) {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    // Skip directories we can't read
                    continue;
                }
                Err(e) => return Err(DiscoveryError::IoError(e)),
            };

            for entry in entries {
                let entry = entry?;
                let path = entry.path();

                // Look for .so (Linux), .dylib (macOS), or .dll (Windows) files
                if self.is_plugin_file(&path) {
                    plugin_files.push(path);
                }
            }
        }

        Ok(plugin_files)
    }

    /// Checks if a path appears to be a plugin file based on extension
    pub fn is_plugin_file(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        if let Some(extension) = path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            matches!(ext.as_str(), "so" | "dylib" | "dll")
        } else {
            false
        }
    }

    /// Finds a specific plugin by name in the search paths
    pub fn find_plugin(&self, name: &str) -> Result<Option<PathBuf>, DiscoveryError> {
        let plugin_files = self.discover_plugins()?;

        for path in plugin_files {
            if let Some(file_name) = path.file_stem() {
                let file_name_str = file_name.to_string_lossy();

                // Match plugin name (e.g., "libsyslog" or "syslog" matches "syslog")
                if file_name_str == name || file_name_str == format!("lib{}", name) {
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }
}

impl Default for PluginDiscovery {
    fn default() -> Self {
        Self::new()
    }
}
