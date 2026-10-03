/// Plugin registry for managing and discovering plugins
///
/// This module provides the registry system for loading, storing, and
/// querying available log format plugins.
use crate::apache_error::ApacheErrorPlugin;
use crate::apm::ApmPlugin;
use crate::auth::AuthPlugin;
use crate::caddy::CaddyPlugin;
use crate::ci::CiPlugin;
use crate::cloud_init::CloudInitPlugin;
use crate::cron::CronPlugin;
use crate::distcc::DistccPlugin;
use crate::dmesg::DmesgPlugin;
use crate::docker::DockerPlugin;
use crate::dovecot::DovecotPlugin;
use crate::elasticsearch::ElasticsearchPlugin;
use crate::exim::EximPlugin;
use crate::fetchmail::FetchmailPlugin;
use crate::ftpstats::FtpstatsPlugin;
use crate::git::GitPlugin;
use crate::haproxy::HaproxyPlugin;
use crate::httpd::HttpdPlugin;
use crate::icecast::IcecastPlugin;
use crate::journalctl::JournalctlPlugin;
use crate::kubernetes::KubernetesPlugin;
use crate::mongodb::MongodbPlugin;
use crate::mysql::MysqlPlugin;
use crate::nginx_error::NginxErrorPlugin;
use crate::oops::OopsPlugin;
use crate::php::PhpPlugin;
use crate::plugin::{Plugin, PluginVersion};
use crate::postfix::PostfixPlugin;
use crate::postgresql::PostgresqlPlugin;
use crate::procmail::ProcmailPlugin;
use crate::proftpd::ProftpdPlugin;
use crate::pureftpd::PureftpdPlugin;
use crate::redis::RedisPlugin;
use crate::resolved::ResolvedPlugin;
use crate::squid::SquidPlugin;
use crate::ssh::SshPlugin;
use crate::sudo::SudoPlugin;
use crate::sulog::SulogPlugin;
use crate::super_log::SuperPlugin;
use crate::syslog::SyslogPlugin;
use crate::terraform::TerraformPlugin;
use crate::ulogd::UlogdPlugin;
use crate::varnish::VarnishPlugin;
use crate::vsftpd::VsftpdPlugin;
use crate::xferlog::XferlogPlugin;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

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
    /// Creates a registry holding every plugin splash ships with
    pub fn with_builtins() -> Self {
        let registry = PluginRegistry::new();

        registry
            .register(Arc::new(ApacheErrorPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(ApmPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(AuthPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(CaddyPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(CiPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(CloudInitPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(CronPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(DistccPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(DmesgPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(DockerPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(DovecotPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(ElasticsearchPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(EximPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(FetchmailPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(FtpstatsPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(GitPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(HaproxyPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(HttpdPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(IcecastPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(JournalctlPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(KubernetesPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(MongodbPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(MysqlPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(NginxErrorPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(OopsPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(PhpPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(PostfixPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(PostgresqlPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(ProcmailPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(ProftpdPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(PureftpdPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(RedisPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(ResolvedPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(SquidPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(SshPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(SudoPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(SulogPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(SuperPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(SyslogPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(TerraformPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(UlogdPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(VarnishPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(VsftpdPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
            .register(Arc::new(XferlogPlugin::new()))
            .expect("built-in plugin names are distinct");

        registry
    }

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

    /// Lists each registered plugin as "name vversion"
    pub fn describe_plugins(&self) -> Result<Vec<String>, RegistryError> {
        let plugins = self
            .plugins
            .read()
            .map_err(|_| RegistryError::RegistryLocked)?;

        let mut descriptions: Vec<String> = plugins
            .values()
            .map(|plugin| format!("{} v{}", plugin.name(), plugin.version()))
            .collect();

        descriptions.sort();

        Ok(descriptions)
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

/// The registry every parsing mode is looked up in, built once per run
static BUILTINS: LazyLock<PluginRegistry> = LazyLock::new(PluginRegistry::with_builtins);

/// The shared registry of plugins splash ships with
pub fn builtins() -> &'static PluginRegistry {
    &BUILTINS
}
