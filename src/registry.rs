/// Plugin registry for managing and discovering plugins
///
/// This module provides the registry system for loading, storing, and
/// querying available log format plugins.
use crate::plugin::{Plugin, PluginVersion};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Error types for the plugin registry
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum RegistryError {
    PluginNotFound(String),
    PluginAlreadyRegistered(String),
    IncompatibleVersion { plugin: String, required: String },
    RegistryLocked,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::PluginNotFound(name) => write!(f, "Plugin '{}' not found", name),
            RegistryError::PluginAlreadyRegistered(name) => {
                write!(f, "Plugin '{}' is already registered", name)
            }
            RegistryError::IncompatibleVersion { plugin, required } => {
                write!(
                    f,
                    "Plugin '{}' has incompatible version (required: {})",
                    plugin, required
                )
            }
            RegistryError::RegistryLocked => write!(f, "Registry is locked for modifications"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Thread-safe registry for managing plugins
#[allow(dead_code)]
pub struct PluginRegistry {
    plugins: RwLock<HashMap<String, Arc<dyn Plugin>>>,
    disabled: RwLock<Vec<String>>,
}

#[allow(dead_code)]
impl PluginRegistry {
    /// Creates a new empty plugin registry
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            disabled: RwLock::new(Vec::new()),
        }
    }

    /// Registers a new plugin
    pub fn register(&self, plugin: Arc<dyn Plugin>) -> Result<(), RegistryError> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|_| RegistryError::RegistryLocked)?;

        let name = plugin.name().to_string();

        if plugins.contains_key(&name) {
            return Err(RegistryError::PluginAlreadyRegistered(name));
        }

        plugins.insert(name, plugin);
        Ok(())
    }

    /// Unregisters a plugin by name
    pub fn unregister(&self, name: &str) -> Result<(), RegistryError> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|_| RegistryError::RegistryLocked)?;

        plugins
            .remove(name)
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))?;

        Ok(())
    }

    /// Gets a plugin by name
    pub fn get(&self, name: &str) -> Result<Arc<dyn Plugin>, RegistryError> {
        let plugins = self
            .plugins
            .read()
            .map_err(|_| RegistryError::RegistryLocked)?;

        plugins
            .get(name)
            .cloned()
            .ok_or_else(|| RegistryError::PluginNotFound(name.to_string()))
    }

    /// Lists all registered plugin names
    pub fn list_plugins(&self) -> Result<Vec<String>, RegistryError> {
        let plugins = self
            .plugins
            .read()
            .map_err(|_| RegistryError::RegistryLocked)?;

        Ok(plugins.keys().cloned().collect())
    }

    /// Returns the number of registered plugins
    pub fn count(&self) -> usize {
        self.plugins.read().map(|p| p.len()).unwrap_or(0)
    }

    /// Checks if a plugin is registered
    pub fn contains(&self, name: &str) -> bool {
        self.plugins
            .read()
            .map(|p| p.contains_key(name))
            .unwrap_or(false)
    }

    /// Disables a plugin (it will still be registered but won't be used)
    pub fn disable_plugin(&self, name: &str) -> Result<(), RegistryError> {
        // Verify plugin exists
        if !self.contains(name) {
            return Err(RegistryError::PluginNotFound(name.to_string()));
        }

        let mut disabled = self
            .disabled
            .write()
            .map_err(|_| RegistryError::RegistryLocked)?;

        if !disabled.contains(&name.to_string()) {
            disabled.push(name.to_string());
        }

        Ok(())
    }

    /// Enables a previously disabled plugin
    pub fn enable_plugin(&self, name: &str) -> Result<(), RegistryError> {
        let mut disabled = self
            .disabled
            .write()
            .map_err(|_| RegistryError::RegistryLocked)?;

        disabled.retain(|n| n != name);
        Ok(())
    }

    /// Checks if a plugin is disabled
    pub fn is_disabled(&self, name: &str) -> bool {
        self.disabled
            .read()
            .map(|d| d.contains(&name.to_string()))
            .unwrap_or(false)
    }

    /// Lists all enabled plugins
    pub fn list_enabled_plugins(&self) -> Result<Vec<String>, RegistryError> {
        let all_plugins = self.list_plugins()?;
        let disabled = self
            .disabled
            .read()
            .map_err(|_| RegistryError::RegistryLocked)?;

        Ok(all_plugins
            .into_iter()
            .filter(|name| !disabled.contains(name))
            .collect())
    }

    /// Verifies that a plugin meets the minimum version requirement
    pub fn verify_version(
        &self,
        name: &str,
        required_version: &PluginVersion,
    ) -> Result<(), RegistryError> {
        let plugin = self.get(name)?;
        let plugin_version = plugin.version();

        if !plugin_version.is_compatible_with(required_version) {
            return Err(RegistryError::IncompatibleVersion {
                plugin: name.to_string(),
                required: required_version.to_string(),
            });
        }

        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
