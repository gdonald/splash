use splash::color::{Color, Style, UnknownColor};
use splash::config::{
    parse_override, Config, ConfigError, Options, Profiles, Settings, CONFIG_FILES, PROFILE_DIR,
};
use splash::output::{OutputMode, TokenKind, UnknownOutputMode};
use splash::theme::{Theme, ThemeError};
use splash::toml::TomlError;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const FULL_CONFIG: &str = r#"
# splash configuration
mode = "clf"
output = "html"
theme = "dracula"
jobs = "3"

[colors]
ip = "bright cyan"
status = "white bold"

[plugins.syslog]
enabled = "true"
facility = "cyan"

[plugins.squid]
enabled = "false"
"#;

fn config() -> Config {
    Config::parse(FULL_CONFIG, "test.toml").unwrap()
}

fn write(home: &Path, relative: &str, contents: &str) -> PathBuf {
    let path = home.join(relative);

    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, contents).unwrap();

    path
}

#[test]
fn a_config_reads_the_default_mode_and_output() {
    let config = config();

    assert_eq!(config.mode, Some("clf".to_string()));
    assert_eq!(config.output, Some("html".to_string()));
}

#[test]
fn a_config_reads_the_worker_count() {
    assert_eq!(config().jobs, Some("3".to_string()));
}

#[test]
fn settings_take_the_worker_count_from_the_config_file() {
    let resolved = settings(Options::default(), &config()).unwrap();

    assert_eq!(resolved.jobs, 3);
}

#[test]
fn the_worker_count_option_wins_over_the_config_file() {
    let options = Options {
        jobs: Some(2),
        ..Options::default()
    };

    assert_eq!(settings(options, &config()).unwrap().jobs, 2);
}

#[test]
fn settings_fall_back_to_one_worker_per_core() {
    let resolved = settings(Options::default(), &Config::default()).unwrap();

    assert_eq!(resolved.jobs, splash::default_jobs());
}

#[test]
fn a_worker_count_of_zero_is_rejected() {
    let options = Options {
        jobs: Some(0),
        ..Options::default()
    };

    assert_eq!(
        settings(options, &Config::default()),
        Err(ConfigError::BadJobs("0".to_string()))
    );
}

#[test]
fn a_configured_worker_count_of_zero_is_rejected() {
    let config = Config::parse("jobs = \"0\"\n", "test.toml").unwrap();

    assert_eq!(
        settings(Options::default(), &config),
        Err(ConfigError::BadJobs("0".to_string()))
    );
}

#[test]
fn a_worker_count_that_is_not_a_number_is_rejected() {
    let config = Config::parse("jobs = \"many\"\n", "test.toml").unwrap();
    let error = settings(Options::default(), &config).unwrap_err();

    assert_eq!(error, ConfigError::BadJobs("many".to_string()));
    assert_eq!(
        error.to_string(),
        "Invalid worker count 'many' (expected a whole number of one or more)"
    );
}

#[test]
fn a_config_reads_the_theme_name() {
    assert_eq!(config().theme, Some("dracula".to_string()));
}

#[test]
fn a_config_reads_the_color_overrides_in_order() {
    assert_eq!(
        config().colors,
        vec![
            ("ip".to_string(), "bright cyan".to_string()),
            ("status".to_string(), "white bold".to_string())
        ]
    );
}

#[test]
fn a_config_groups_settings_under_the_plugin_they_belong_to() {
    let config = config();
    let syslog = config.plugin_settings("syslog").unwrap();

    assert_eq!(syslog.get("facility"), Some(&"cyan".to_string()));
}

#[test]
fn a_plugin_without_a_table_has_no_settings() {
    assert!(config().plugin_settings("postfix").is_none());
}

#[test]
fn configured_plugins_are_listed_in_name_order() {
    assert_eq!(config().configured_plugins(), vec!["squid", "syslog"]);
}

#[test]
fn a_plugin_is_enabled_unless_the_config_turns_it_off() {
    let config = config();

    assert!(config.plugin_enabled("syslog"));
    assert!(config.plugin_enabled("postfix"));
    assert!(!config.plugin_enabled("squid"));
}

#[test]
fn the_disabled_plugins_are_the_ones_turned_off() {
    assert_eq!(config().disabled_plugins(), vec!["squid"]);
}

#[test]
fn an_empty_config_disables_nothing() {
    assert!(Config::default().disabled_plugins().is_empty());
}

#[test]
fn an_unknown_top_level_key_is_rejected() {
    assert_eq!(
        Config::parse("verbose = true\n", "test.toml"),
        Err(ConfigError::UnknownKey("verbose".to_string()))
    );
}

#[test]
fn an_unknown_table_is_rejected() {
    assert_eq!(
        Config::parse("[styles]\nip = \"cyan\"\n", "test.toml"),
        Err(ConfigError::UnknownKey("styles.ip".to_string()))
    );
}

#[test]
fn a_syntax_error_names_the_file_it_was_found_in() {
    let error = Config::parse("broken\n", "test.toml").unwrap_err();

    assert_eq!(
        error,
        ConfigError::Syntax {
            path: "test.toml".to_string(),
            error: TomlError {
                line: 1,
                message: "expected a key = value pair, found 'broken'".to_string()
            }
        }
    );
    assert_eq!(
        error.to_string(),
        "test.toml: line 1: expected a key = value pair, found 'broken'"
    );
}

#[test]
fn the_config_theme_applies_its_color_overrides() {
    let theme = config().theme().unwrap();

    assert_eq!(theme.name(), "dracula");
    assert_eq!(theme.style(TokenKind::Ip), Style::new(Color::BrightCyan));
    assert_eq!(theme.style(TokenKind::Status), Style::bold(Color::White));
}

#[test]
fn a_config_without_a_theme_starts_from_the_default_theme() {
    let config = Config::parse("[colors]\nip = \"green\"\n", "test.toml").unwrap();
    let theme = config.theme().unwrap();

    assert_eq!(theme.name(), "dark");
    assert_eq!(theme.style(TokenKind::Ip), Style::new(Color::Green));
}

#[test]
fn a_config_naming_an_unknown_color_key_is_rejected() {
    let config = Config::parse("[colors]\nhostname = \"green\"\n", "test.toml").unwrap();

    assert_eq!(
        config.theme(),
        Err(ConfigError::Theme(ThemeError::UnknownTokenKind(
            "hostname".to_string()
        )))
    );
}

#[test]
fn a_config_file_is_read_from_disk() {
    let home = TempDir::new().unwrap();
    let path = write(home.path(), "config.toml", "mode = \"clf\"\n");

    assert_eq!(Config::load(&path).unwrap().mode, Some("clf".to_string()));
}

#[test]
fn a_missing_config_file_is_reported() {
    let home = TempDir::new().unwrap();
    let path = home.path().join("missing.toml");

    let error = Config::load(&path).unwrap_err();

    assert!(
        error
            .to_string()
            .starts_with(&format!("Could not read {}: ", path.display())),
        "{}",
        error
    );
}

#[test]
fn the_config_file_locations_are_searched_in_order() {
    assert_eq!(CONFIG_FILES, [".splash/config.toml", ".splashrc"]);
}

#[test]
fn the_dot_splash_config_file_is_found_first() {
    let home = TempDir::new().unwrap();
    write(home.path(), ".splash/config.toml", "mode = \"clf\"\n");
    write(home.path(), ".splashrc", "mode = \"ad-hoc\"\n");

    assert_eq!(
        Config::find(home.path()),
        Some(home.path().join(".splash/config.toml"))
    );
    assert_eq!(
        Config::load_from_home(home.path()).unwrap().mode,
        Some("clf".to_string())
    );
}

#[test]
fn the_splashrc_file_is_used_when_there_is_no_config_toml() {
    let home = TempDir::new().unwrap();
    write(home.path(), ".splashrc", "mode = \"clf\"\n");

    assert_eq!(
        Config::find(home.path()),
        Some(home.path().join(".splashrc"))
    );
}

#[test]
fn a_home_without_a_config_file_yields_an_empty_config() {
    let home = TempDir::new().unwrap();

    assert_eq!(Config::find(home.path()), None);
    assert_eq!(
        Config::load_from_home(home.path()).unwrap(),
        Config::default()
    );
}

#[test]
fn a_platform_without_a_home_directory_yields_an_empty_config() {
    assert_eq!(Config::load_for_home_dir(None).unwrap(), Config::default());
}

#[test]
fn the_home_directory_config_is_read_when_one_is_given() {
    let home = TempDir::new().unwrap();
    write(home.path(), ".splashrc", "mode = \"clf\"\n");

    let config = Config::load_for_home_dir(Some(home.path().to_path_buf())).unwrap();

    assert_eq!(config.mode, Some("clf".to_string()));
}

#[test]
fn the_default_config_is_the_one_in_the_users_home_directory() {
    let home = PathBuf::from(std::env::var("HOME").unwrap());

    assert_eq!(Config::load_default(), Config::load_from_home(&home));
}

#[test]
fn profiles_live_under_the_home_directory() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());

    assert_eq!(profiles.root(), home.path().join(PROFILE_DIR));
    assert_eq!(
        profiles.path("night"),
        home.path().join(PROFILE_DIR).join("night.toml")
    );
}

#[test]
fn a_platform_without_a_home_directory_uses_a_relative_profile_directory() {
    assert_eq!(
        Profiles::for_home_dir(None),
        Profiles::new(PathBuf::from(PROFILE_DIR))
    );
}

#[test]
fn the_home_directory_profiles_are_used_when_one_is_given() {
    let home = TempDir::new().unwrap();

    assert_eq!(
        Profiles::for_home_dir(Some(home.path().to_path_buf())),
        Profiles::in_home(home.path())
    );
}

#[test]
fn the_default_profiles_are_the_ones_in_the_users_home_directory() {
    let home = PathBuf::from(std::env::var("HOME").unwrap());

    assert_eq!(Profiles::default_profiles(), Profiles::in_home(&home));
}

#[test]
fn a_saved_profile_reloads_as_the_same_theme() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());
    let mut theme = Theme::solarized();
    theme.apply_override("ip", "bright cyan").unwrap();

    let path = profiles.save("night", &theme).unwrap();

    assert_eq!(path, profiles.path("night"));
    assert_eq!(profiles.load("night").unwrap(), theme);
}

#[test]
fn saving_a_profile_creates_the_profile_directory() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());

    profiles.save("night", &Theme::dark()).unwrap();

    assert!(profiles.root().is_dir());
}

#[test]
fn a_profile_directory_that_cannot_be_created_is_reported() {
    let home = TempDir::new().unwrap();
    let blocker = write(home.path(), "blocker", "");
    let profiles = Profiles::new(blocker.join("profiles"));

    let error = profiles.save("night", &Theme::dark()).unwrap_err();

    assert!(
        error
            .to_string()
            .starts_with(&format!("Could not create {}: ", profiles.root().display())),
        "{}",
        error
    );
}

#[test]
fn a_profile_that_cannot_be_written_is_reported() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());

    let error = profiles.save("nested/night", &Theme::dark()).unwrap_err();

    assert!(
        error.to_string().starts_with(&format!(
            "Could not write {}: ",
            profiles.path("nested/night").display()
        )),
        "{}",
        error
    );
}

#[test]
fn a_missing_profile_names_the_file_it_looked_for() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());

    let error = profiles.load("night").unwrap_err();

    assert_eq!(
        error,
        ConfigError::ProfileNotFound {
            name: "night".to_string(),
            path: profiles.path("night").display().to_string()
        }
    );
    assert_eq!(
        error.to_string(),
        format!(
            "Color profile 'night' not found at {}",
            profiles.path("night").display()
        )
    );
}

#[test]
fn saved_profiles_are_listed_in_name_order() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());
    profiles.save("night", &Theme::dark()).unwrap();
    profiles.save("day", &Theme::light()).unwrap();
    fs::write(profiles.root().join("notes.txt"), "ignored").unwrap();
    fs::create_dir(profiles.root().join("nested")).unwrap();

    assert_eq!(
        profiles.list(),
        vec!["day".to_string(), "night".to_string()]
    );
}

#[test]
fn a_missing_profile_directory_lists_nothing() {
    let home = TempDir::new().unwrap();

    assert!(Profiles::in_home(home.path()).list().is_empty());
}

fn settings(options: Options, config: &Config) -> Result<Settings, ConfigError> {
    Settings::resolve(&options, config, &Profiles::new(PathBuf::from("unused")))
}

#[test]
fn settings_fall_back_to_the_ad_hoc_mode_and_ansi_output() {
    let resolved = settings(Options::default(), &Config::default()).unwrap();

    assert_eq!(resolved.mode, "ad-hoc");
    assert_eq!(resolved.output_mode, OutputMode::Ansi);
    assert_eq!(resolved.theme, Theme::dark());
}

#[test]
fn settings_take_the_mode_and_output_from_the_config_file() {
    let resolved = settings(Options::default(), &config()).unwrap();

    assert_eq!(resolved.mode, "clf");
    assert_eq!(resolved.output_mode, OutputMode::Html);
    assert_eq!(resolved.theme.name(), "dracula");
}

#[test]
fn command_line_options_win_over_the_config_file() {
    let options = Options {
        mode: Some("ad-hoc".to_string()),
        output: Some("json".to_string()),
        theme: Some("light".to_string()),
        ..Options::default()
    };

    let resolved = settings(options, &config()).unwrap();

    assert_eq!(resolved.mode, "ad-hoc");
    assert_eq!(resolved.output_mode, OutputMode::Json);
    assert_eq!(resolved.theme.name(), "light");
}

#[test]
fn a_color_override_wins_over_the_config_file() {
    let options = Options {
        colors: vec!["ip=green".to_string()],
        ..Options::default()
    };

    let resolved = settings(options, &config()).unwrap();

    assert_eq!(
        resolved.theme.style(TokenKind::Ip),
        Style::new(Color::Green)
    );
}

#[test]
fn a_saved_profile_supplies_the_colors_to_start_from() {
    let home = TempDir::new().unwrap();
    let profiles = Profiles::in_home(home.path());
    let mut saved = Theme::light();
    saved.apply_override("ip", "magenta").unwrap();
    profiles.save("day", &saved).unwrap();

    let options = Options {
        profile: Some("day".to_string()),
        ..Options::default()
    };

    let resolved = Settings::resolve(&options, &Config::default(), &profiles).unwrap();

    assert_eq!(resolved.theme, saved);
}

#[test]
fn a_missing_profile_stops_the_run() {
    let home = TempDir::new().unwrap();
    let options = Options {
        profile: Some("day".to_string()),
        ..Options::default()
    };

    let error = Settings::resolve(
        &options,
        &Config::default(),
        &Profiles::in_home(home.path()),
    )
    .unwrap_err();

    assert!(matches!(error, ConfigError::ProfileNotFound { .. }));
}

#[test]
fn an_unknown_theme_stops_the_run() {
    let options = Options {
        theme: Some("neon".to_string()),
        ..Options::default()
    };

    assert_eq!(
        settings(options, &Config::default()),
        Err(ConfigError::Theme(ThemeError::UnknownTheme(
            "neon".to_string()
        )))
    );
}

#[test]
fn an_unknown_theme_in_the_config_file_stops_the_run() {
    let config = Config::parse("theme = \"neon\"\n", "test.toml").unwrap();

    assert!(matches!(
        settings(Options::default(), &config),
        Err(ConfigError::Theme(ThemeError::UnknownTheme(_)))
    ));
}

#[test]
fn an_unknown_output_format_stops_the_run() {
    let options = Options {
        output: Some("braille".to_string()),
        ..Options::default()
    };

    let error = settings(options, &Config::default()).unwrap_err();

    assert_eq!(
        error,
        ConfigError::Output(UnknownOutputMode("braille".to_string()))
    );
    assert_eq!(
        error.to_string(),
        "Unknown output mode 'braille' (expected ansi, curses, html, json, or plain)"
    );
}

#[test]
fn a_color_override_naming_an_unknown_key_stops_the_run() {
    let options = Options {
        colors: vec!["hostname=green".to_string()],
        ..Options::default()
    };

    assert_eq!(
        settings(options, &Config::default()),
        Err(ConfigError::Theme(ThemeError::UnknownTokenKind(
            "hostname".to_string()
        )))
    );
}

#[test]
fn a_color_override_naming_an_unknown_color_stops_the_run() {
    let options = Options {
        colors: vec!["ip=mauve".to_string()],
        ..Options::default()
    };

    assert_eq!(
        settings(options, &Config::default()),
        Err(ConfigError::Theme(ThemeError::UnknownColor(UnknownColor(
            "mauve".to_string()
        ))))
    );
}

#[test]
fn a_color_override_splits_on_its_first_equals_sign() {
    assert_eq!(
        parse_override(" ip = bright cyan "),
        Ok(("ip".to_string(), "bright cyan".to_string()))
    );
}

#[test]
fn a_color_override_without_an_equals_sign_is_rejected() {
    let error = parse_override("ip cyan").unwrap_err();

    assert_eq!(error, ConfigError::BadOverride("ip cyan".to_string()));
    assert_eq!(
        error.to_string(),
        "Invalid color override 'ip cyan' (expected KEY=COLOR, such as ip=cyan)"
    );
}

#[test]
fn a_color_override_missing_a_key_is_rejected() {
    assert_eq!(
        parse_override("=cyan"),
        Err(ConfigError::BadOverride("=cyan".to_string()))
    );
}

#[test]
fn a_color_override_missing_a_color_is_rejected() {
    assert_eq!(
        parse_override("ip="),
        Err(ConfigError::BadOverride("ip=".to_string()))
    );
}

#[test]
fn a_bad_override_from_the_command_line_stops_the_run() {
    let options = Options {
        colors: vec!["ip cyan".to_string()],
        ..Options::default()
    };

    assert_eq!(
        settings(options, &Config::default()),
        Err(ConfigError::BadOverride("ip cyan".to_string()))
    );
}

#[test]
fn a_config_error_is_an_error() {
    let error: Box<dyn Error> = Box::new(ConfigError::Io("no file".to_string()));

    assert_eq!(error.to_string(), "no file");
    assert!(error.source().is_none());
}

#[test]
fn an_unknown_key_says_which_key_it_does_not_know() {
    assert_eq!(
        ConfigError::UnknownKey("verbose".to_string()).to_string(),
        "Unknown configuration key 'verbose'"
    );
}

#[test]
fn a_theme_error_reaches_the_config_error_unchanged() {
    let error = ConfigError::from(ThemeError::UnknownTheme("neon".to_string()));

    assert_eq!(
        error.to_string(),
        ThemeError::UnknownTheme("neon".to_string()).to_string()
    );
}

#[test]
fn an_output_error_reaches_the_config_error_unchanged() {
    let error = ConfigError::from(UnknownOutputMode("braille".to_string()));

    assert_eq!(
        error.to_string(),
        UnknownOutputMode("braille".to_string()).to_string()
    );
}
