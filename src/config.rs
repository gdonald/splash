//! Configuration files, color profiles, and settings resolution
//!
//! splash reads `~/.splash/config.toml`, falling back to `~/.splashrc`. The
//! file sets the default mode and output format, overrides individual token
//! colors, and holds a table of settings per plugin. Command line options win
//! over the file, and saved color profiles live in `~/.splash/profiles/`.
use crate::output::{OutputMode, UnknownOutputMode};
use crate::theme::{Theme, ThemeError};
use crate::toml::{self, TomlError};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Config file locations searched under the home directory, in order
pub const CONFIG_FILES: [&str; 2] = [".splash/config.toml", ".splashrc"];

/// Directory holding saved color profiles, under the home directory
pub const PROFILE_DIR: &str = ".splash/profiles";

/// Error returned when configuration cannot be read or applied
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    Io(String),
    Syntax { path: String, error: TomlError },
    UnknownKey(String),
    Theme(ThemeError),
    Output(UnknownOutputMode),
    BadJobs(String),
    ProfileNotFound { name: String, path: String },
    BadOverride(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(message) => write!(f, "{}", message),
            ConfigError::Syntax { path, error } => write!(f, "{}: {}", path, error),
            ConfigError::UnknownKey(key) => write!(f, "Unknown configuration key '{}'", key),
            ConfigError::Theme(error) => write!(f, "{}", error),
            ConfigError::Output(error) => write!(f, "{}", error),
            ConfigError::BadJobs(value) => write!(
                f,
                "Invalid worker count '{}' (expected a whole number of one or more)",
                value
            ),
            ConfigError::ProfileNotFound { name, path } => {
                write!(f, "Color profile '{}' not found at {}", name, path)
            }
            ConfigError::BadOverride(value) => write!(
                f,
                "Invalid color override '{}' (expected KEY=COLOR, such as ip=cyan)",
                value
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<ThemeError> for ConfigError {
    fn from(error: ThemeError) -> Self {
        ConfigError::Theme(error)
    }
}

impl From<UnknownOutputMode> for ConfigError {
    fn from(error: UnknownOutputMode) -> Self {
        ConfigError::Output(error)
    }
}

/// The settings read from a config file
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub mode: Option<String>,
    pub output: Option<String>,
    pub theme: Option<String>,
    pub jobs: Option<String>,
    pub colors: Vec<(String, String)>,
    pub plugins: BTreeMap<String, BTreeMap<String, String>>,
}

impl Config {
    /// Reads config text, rejecting keys splash does not know
    pub fn parse(text: &str, path: &str) -> Result<Config, ConfigError> {
        let entries = toml::parse(text).map_err(|error| ConfigError::Syntax {
            path: path.to_string(),
            error,
        })?;

        let mut config = Config::default();

        for (key, value) in entries {
            let segments: Vec<&str> = key.split('.').collect();

            match segments.as_slice() {
                ["mode"] => config.mode = Some(value),
                ["output"] => config.output = Some(value),
                ["theme"] => config.theme = Some(value),
                ["jobs"] => config.jobs = Some(value),
                ["colors", name] => config.colors.push((name.to_string(), value)),
                ["plugins", plugin, name] => {
                    config
                        .plugins
                        .entry(plugin.to_string())
                        .or_default()
                        .insert(name.to_string(), value);
                }
                _ => return Err(ConfigError::UnknownKey(key)),
            }
        }

        Ok(config)
    }

    /// Reads the config file at `path`
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = read_file(path)?;

        Config::parse(&text, &path.display().to_string())
    }

    /// The first config file present under `home`, if any
    pub fn find(home: &Path) -> Option<PathBuf> {
        CONFIG_FILES
            .iter()
            .map(|name| home.join(name))
            .find(|path| path.is_file())
    }

    /// Reads the first config file present under `home`, or an empty config
    pub fn load_from_home(home: &Path) -> Result<Config, ConfigError> {
        match Config::find(home) {
            Some(path) => Config::load(&path),
            None => Ok(Config::default()),
        }
    }

    /// Reads the config for the current user, or an empty config when there is
    /// no home directory to search
    pub fn load_default() -> Result<Config, ConfigError> {
        Config::load_for_home_dir(dirs::home_dir())
    }

    /// Reads the config under the given home directory, or an empty config
    /// when the platform reports no home directory
    pub fn load_for_home_dir(home: Option<PathBuf>) -> Result<Config, ConfigError> {
        match home {
            Some(home) => Config::load_from_home(&home),
            None => Ok(Config::default()),
        }
    }

    /// The settings configured for one plugin
    pub fn plugin_settings(&self, name: &str) -> Option<&BTreeMap<String, String>> {
        self.plugins.get(name)
    }

    /// Every plugin with a configuration table, in name order
    pub fn configured_plugins(&self) -> Vec<&str> {
        self.plugins.keys().map(|name| name.as_str()).collect()
    }

    /// Whether a plugin is enabled, which it is unless the config turns it off
    pub fn plugin_enabled(&self, name: &str) -> bool {
        self.plugin_settings(name)
            .and_then(|settings| settings.get("enabled"))
            .map(|value| value != "false")
            .unwrap_or(true)
    }

    /// Plugins the config turns off, in name order
    pub fn disabled_plugins(&self) -> Vec<&str> {
        self.configured_plugins()
            .into_iter()
            .filter(|name| !self.plugin_enabled(name))
            .collect()
    }

    /// The theme named by the config, with its color overrides applied
    pub fn theme(&self) -> Result<Theme, ConfigError> {
        let mut theme = match &self.theme {
            Some(name) => Theme::preset(name)?,
            None => Theme::default(),
        };

        apply_pairs(&mut theme, &self.colors)?;

        Ok(theme)
    }
}

/// Saved color profiles on disk
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profiles {
    root: PathBuf,
}

impl Profiles {
    /// Profiles stored in `root`
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Profiles stored under a home directory
    pub fn in_home(home: &Path) -> Self {
        Profiles::new(home.join(PROFILE_DIR))
    }

    /// Profiles for the current user, falling back to a relative directory
    /// when there is no home directory
    pub fn default_profiles() -> Self {
        Profiles::for_home_dir(dirs::home_dir())
    }

    /// Profiles under the given home directory, falling back to a relative
    /// directory when the platform reports no home directory
    pub fn for_home_dir(home: Option<PathBuf>) -> Self {
        match home {
            Some(home) => Profiles::in_home(&home),
            None => Profiles::new(PathBuf::from(PROFILE_DIR)),
        }
    }

    /// The directory profiles are read from and written to
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the named profile is stored
    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{}.toml", name))
    }

    /// Writes a theme out as a named profile, returning the file written
    pub fn save(&self, name: &str, theme: &Theme) -> Result<PathBuf, ConfigError> {
        fs::create_dir_all(&self.root).map_err(|error| {
            ConfigError::Io(format!(
                "Could not create {}: {}",
                self.root.display(),
                error
            ))
        })?;

        let path = self.path(name);

        fs::write(&path, theme.to_toml()).map_err(|error| {
            ConfigError::Io(format!("Could not write {}: {}", path.display(), error))
        })?;

        Ok(path)
    }

    /// Reads a named profile as a theme
    pub fn load(&self, name: &str) -> Result<Theme, ConfigError> {
        let path = self.path(name);

        if !path.is_file() {
            return Err(ConfigError::ProfileNotFound {
                name: name.to_string(),
                path: path.display().to_string(),
            });
        }

        Config::load(&path)?.theme()
    }

    /// The names of the saved profiles, in name order
    pub fn list(&self) -> Vec<String> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(_) => return Vec::new(),
        };

        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "toml"))
            .filter_map(|entry| {
                entry
                    .path()
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
            })
            .collect();

        names.sort();
        names
    }
}

/// The command line options that configuration can also supply
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    pub mode: Option<String>,
    pub output: Option<String>,
    pub theme: Option<String>,
    pub profile: Option<String>,
    pub colors: Vec<String>,
    pub jobs: Option<usize>,
}

/// The parsing mode, output format, and theme splash will run with
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub mode: String,
    pub output_mode: OutputMode,
    pub theme: Theme,
    pub jobs: usize,
}

/// Mode used when neither the command line nor the config file names one
pub const DEFAULT_MODE: &str = "ad-hoc";

/// Output format used when neither the command line nor the config file names one
pub const DEFAULT_OUTPUT: &str = "ansi";

impl Settings {
    /// Combines command line options with a config file and saved profiles.
    ///
    /// Command line options win over the config file. Colors are layered: the
    /// profile or theme first, then the config file's `[colors]` table, then
    /// any `--color` overrides.
    pub fn resolve(
        options: &Options,
        config: &Config,
        profiles: &Profiles,
    ) -> Result<Settings, ConfigError> {
        let mut theme = match (&options.profile, &options.theme, &config.theme) {
            (Some(profile), _, _) => profiles.load(profile)?,
            (None, Some(name), _) => Theme::preset(name)?,
            (None, None, Some(name)) => Theme::preset(name)?,
            (None, None, None) => Theme::default(),
        };

        apply_pairs(&mut theme, &config.colors)?;

        for value in &options.colors {
            let (key, color) = parse_override(value)?;
            theme.apply_override(&key, &color)?;
        }

        let mode = options
            .mode
            .clone()
            .or_else(|| config.mode.clone())
            .unwrap_or_else(|| DEFAULT_MODE.to_string());

        let output = options
            .output
            .clone()
            .or_else(|| config.output.clone())
            .unwrap_or_else(|| DEFAULT_OUTPUT.to_string());

        Ok(Settings {
            mode,
            output_mode: output.parse()?,
            theme,
            jobs: resolve_jobs(options.jobs, config.jobs.as_deref())?,
        })
    }
}

/// The worker count to render with, which defaults to the number of threads
/// the machine can run at once
fn resolve_jobs(option: Option<usize>, configured: Option<&str>) -> Result<usize, ConfigError> {
    if let Some(jobs) = option {
        return check_jobs(jobs, &jobs.to_string());
    }

    match configured {
        Some(value) => {
            let jobs = value
                .parse::<usize>()
                .map_err(|_| ConfigError::BadJobs(value.to_string()))?;

            check_jobs(jobs, value)
        }
        None => Ok(crate::default_jobs()),
    }
}

fn check_jobs(jobs: usize, value: &str) -> Result<usize, ConfigError> {
    if jobs == 0 {
        return Err(ConfigError::BadJobs(value.to_string()));
    }

    Ok(jobs)
}

/// Splits a `KEY=COLOR` override, as given to `--color`
pub fn parse_override(value: &str) -> Result<(String, String), ConfigError> {
    let (key, color) = value
        .split_once('=')
        .ok_or_else(|| ConfigError::BadOverride(value.to_string()))?;

    let key = key.trim();
    let color = color.trim();

    if key.is_empty() || color.is_empty() {
        return Err(ConfigError::BadOverride(value.to_string()));
    }

    Ok((key.to_string(), color.to_string()))
}

fn apply_pairs(theme: &mut Theme, pairs: &[(String, String)]) -> Result<(), ConfigError> {
    for (key, value) in pairs {
        theme.apply_override(key, value)?;
    }

    Ok(())
}

fn read_file(path: &Path) -> Result<String, ConfigError> {
    fs::read_to_string(path)
        .map_err(|error| ConfigError::Io(format!("Could not read {}: {}", path.display(), error)))
}
