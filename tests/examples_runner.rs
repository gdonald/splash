use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Helper function to get the path to example log files
fn example_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("examples")
        .join(filename)
}

/// Helper function to run splash with specific arguments
fn run_splash(args: &[&str]) -> Result<std::process::Output, std::io::Error> {
    Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .output()
}

/// Helper function to run splash with file input
/// Note: Uses timeout because file mode runs in watch loop
fn run_splash_with_file(mode: &str, filepath: &str) -> Result<String, std::io::Error> {
    // Spawn the command
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .arg("--mode")
        .arg(mode)
        .arg("--path")
        .arg(filepath)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Give it time to process and output (watch loop prints immediately then waits)
    thread::sleep(Duration::from_millis(500));

    // Kill the process (it runs in infinite watch loop)
    let _ = child.kill();

    // Get the output
    let output = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Helper function to run splash against a home directory holding its config
fn run_splash_with_home(
    home: &Path,
    args: &[&str],
    input: &str,
) -> Result<std::process::Output, std::io::Error> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .env("HOME", home)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }

    child.wait_with_output()
}

/// Helper function to read one of the example log files
fn example_contents(filename: &str) -> String {
    std::fs::read_to_string(example_path(filename)).unwrap()
}

/// Helper function to run splash against a file with extra arguments
/// Note: Uses a short wait because file mode runs in a watch loop
fn run_splash_file_args(args: &[&str]) -> Result<String, std::io::Error> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    thread::sleep(Duration::from_millis(500));

    let _ = child.kill();

    let output = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Helper function to run splash against a file, collecting output in a file
/// so a large run is not held up by a full pipe
fn run_splash_file_to_file(args: &[&str], output_path: &Path) -> Result<String, std::io::Error> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .stdout(std::fs::File::create(output_path)?)
        .stderr(std::process::Stdio::null())
        .spawn()?;

    thread::sleep(Duration::from_millis(500));

    let _ = child.kill();
    let _ = child.wait();

    std::fs::read_to_string(output_path)
}

/// Helper function to run splash with stdin
fn run_splash_with_stdin(mode: &str, input: &str) -> Result<String, std::io::Error> {
    run_splash_with_stdin_args(&["--mode", mode], input)
}

/// Helper function to run splash with stdin and arbitrary arguments
fn run_splash_with_stdin_args(args: &[&str], input: &str) -> Result<String, std::io::Error> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ==================== Common Log Format Tests ====================

#[test]
fn test_clf_basic_parsing() {
    let example = example_path("clf_basic.log");
    let result = run_splash_with_file("clf", example.to_str().unwrap());

    assert!(result.is_ok(), "CLF parsing should succeed");
    let output = result.unwrap();

    // Output should contain colorized content (ANSI codes)
    assert!(!output.is_empty(), "Output should not be empty");

    // Should contain the IP address
    assert!(
        output.contains("127.0.0.1") || output.contains("192.168"),
        "Output should contain IP addresses"
    );
}

#[test]
fn test_clf_multiple_entries() {
    let example = example_path("clf_multiple.log");
    let result = run_splash_with_file("clf", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Should process multiple log lines
    let line_count = output.lines().count();
    assert!(line_count >= 3, "Should have at least 3 output lines");
}

#[test]
fn test_clf_http_methods() {
    let example = example_path("clf_http_methods.log");
    let result = run_splash_with_file("clf", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Should contain various HTTP methods
    assert!(
        output.contains("GET")
            || output.contains("POST")
            || output.contains("PUT")
            || output.contains("DELETE"),
        "Output should contain HTTP methods"
    );
}

#[test]
fn test_clf_status_codes() {
    let example = example_path("clf_status_codes.log");
    let result = run_splash_with_file("clf", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Should handle different status codes
    assert!(
        output.contains("200") || output.contains("404") || output.contains("500"),
        "Output should contain status codes"
    );
}

#[test]
fn test_clf_empty_file() {
    let example = example_path("clf_empty.log");
    let result = run_splash_with_file("clf", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Empty file should produce empty output
    assert!(
        output.trim().is_empty() || output.lines().count() == 0,
        "Empty file should produce no output"
    );
}

// ==================== Ad-hoc Mode Tests ====================

#[test]
fn test_adhoc_ip_highlighting() {
    let example = example_path("adhoc_ips.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(!output.is_empty(), "Output should not be empty");
    // IP addresses should be present
    assert!(
        output.contains("192.168") || output.contains("10.0") || output.contains("172.16"),
        "Should contain IP addresses"
    );
}

#[test]
fn test_adhoc_http_verbs() {
    let example = example_path("adhoc_http.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // HTTP verbs should be highlighted
    assert!(
        output.contains("GET") || output.contains("POST") || output.contains("PUT"),
        "Should contain HTTP verbs"
    );
}

#[test]
fn test_adhoc_timestamps() {
    let example = example_path("adhoc_timestamps.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Should handle timestamp patterns
    assert!(!output.is_empty(), "Output should contain timestamp data");
}

#[test]
fn test_adhoc_numbers() {
    let example = example_path("adhoc_numbers.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Numbers should be highlighted
    assert!(!output.is_empty(), "Output should contain numbers");
}

#[test]
fn test_adhoc_mixed_content() {
    let example = example_path("adhoc_mixed.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Should handle various patterns in one file
    assert!(!output.is_empty(), "Output should not be empty");
    let line_count = output.lines().count();
    assert!(line_count > 0, "Should have output lines");
}

// ==================== Edge Cases & Error Handling ====================

#[test]
fn test_nonexistent_file() {
    let result = run_splash_with_file("clf", "/nonexistent/path/file.log");

    // Should handle missing files gracefully
    assert!(
        result.is_err() || result.unwrap().is_empty(),
        "Should handle nonexistent files"
    );
}

#[test]
fn test_malformed_clf_entries() {
    let example = example_path("clf_malformed.log");
    let result = run_splash_with_file("clf", example.to_str().unwrap());

    // Should not crash on malformed entries
    assert!(result.is_ok(), "Should handle malformed entries gracefully");
}

#[test]
fn test_empty_lines() {
    let example = example_path("adhoc_empty_lines.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok());
    // Should skip empty lines without errors
}

#[test]
fn test_very_long_lines() {
    let example = example_path("adhoc_long_lines.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok(), "Should handle very long lines");
}

#[test]
fn test_special_characters() {
    let example = example_path("adhoc_special_chars.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok(), "Should handle special characters");
}

// ==================== CLI Argument Tests ====================

#[test]
fn test_help_flag() {
    let result = run_splash(&["--help"]);

    assert!(result.is_ok());
    let binding = result.unwrap();
    let output = String::from_utf8_lossy(&binding.stdout);

    // Help should mention usage and options
    assert!(
        output.contains("Usage") || output.contains("OPTIONS") || output.contains("help"),
        "Help should show usage information"
    );
}

#[test]
fn test_version_flag() {
    let result = run_splash(&["--version"]);

    assert!(result.is_ok());
    let binding = result.unwrap();
    let output = String::from_utf8_lossy(&binding.stdout);

    // Version should be displayed
    assert!(!output.is_empty(), "Version should be displayed");
}

#[test]
fn test_invalid_mode() {
    let example = example_path("clf_basic.log");
    let result = run_splash_with_file("invalid-mode", example.to_str().unwrap());

    // Should default to ad-hoc mode or handle gracefully
    assert!(result.is_ok(), "Should handle invalid mode");
}

#[test]
fn test_default_mode() {
    let example = example_path("adhoc_mixed.log");
    // Use ad-hoc as default mode
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok(), "Should use default mode");
}

// ==================== Stdin Input Tests ====================

#[test]
fn test_stdin_clf() {
    let input =
        r#"127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326"#;
    let result = run_splash_with_stdin("clf", input);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(!output.is_empty(), "Should process stdin input");
}

#[test]
fn test_stdin_adhoc() {
    let input = "192.168.1.1 GET /api/endpoint HTTP/1.1 200";
    let result = run_splash_with_stdin("ad-hoc", input);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(!output.is_empty(), "Should process stdin input");
}

// ==================== Pattern Matching Tests ====================

#[test]
fn test_ip_pattern_matching() {
    let input = "Connection from 192.168.1.100";
    let result = run_splash_with_stdin("ad-hoc", input);

    assert!(result.is_ok());
    let output = result.unwrap();
    // Should highlight IP address
    assert!(
        output.contains("192.168.1.100"),
        "Should preserve IP address"
    );
}

#[test]
fn test_http_version_matching() {
    let input = "Request: HTTP/1.0";
    let result = run_splash_with_stdin("ad-hoc", input);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("HTTP/1.0"), "Should preserve HTTP version");
}

#[test]
fn test_datetime_matching() {
    let input = "[10/Oct/2000:13:55:36 -0700] Request received";
    let result = run_splash_with_stdin("ad-hoc", input);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(!output.is_empty(), "Should handle datetime patterns");
}

#[test]
fn test_quote_and_bracket_matching() {
    let input = r#"[INFO] "Processing request" from client"#;
    let result = run_splash_with_stdin("ad-hoc", input);

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(!output.is_empty(), "Should handle quotes and brackets");
}

// ==================== Integration Tests ====================

#[test]
fn test_real_world_apache_log() {
    let example = example_path("real_apache.log");
    let result = run_splash_with_file("clf", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Real-world logs should be processed successfully
    assert!(!output.is_empty(), "Should process real Apache logs");
}

#[test]
fn test_real_world_syslog() {
    let example = example_path("real_syslog.log");
    let result = run_splash_with_file("ad-hoc", example.to_str().unwrap());

    assert!(result.is_ok());
    let output = result.unwrap();

    // Syslog format should work in ad-hoc mode
    assert!(!output.is_empty(), "Should process syslog entries");
}

// ==================== Output Mode Tests ====================

fn output_modes_example() -> String {
    std::fs::read_to_string(example_path("output_modes.log"))
        .expect("example log should be readable")
}

#[test]
fn test_plain_output_mode_strips_colors() {
    let result = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "plain"],
        &output_modes_example(),
    );

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(
        !output.contains('\u{1b}'),
        "Plain output should have no escape sequences"
    );
    assert!(output.contains("192.168.1.10 GET /index.html HTTP/1.0 200"));
}

#[test]
fn test_json_output_mode_emits_one_object_per_line() {
    let result = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "json"],
        &output_modes_example(),
    );

    assert!(result.is_ok());
    let output = result.unwrap();

    assert_eq!(
        output.lines().count(),
        3,
        "Should emit one JSON object per log line"
    );
    for line in output.lines() {
        assert!(
            line.starts_with("{\"text\":\""),
            "Each line should be a JSON object"
        );
        assert!(
            line.ends_with("]}"),
            "Each line should close its token list"
        );
    }
    assert!(output.contains("\"kind\":\"http_verb\",\"text\":\"GET\""));
}

#[test]
fn test_json_output_mode_names_clf_fields() {
    let input =
        "127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] \"GET /apache_pb.gif HTTP/1.0\" 200 2326";
    let result = run_splash_with_stdin_args(&["--mode", "clf", "--output", "json"], input);

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(output.contains("\"kind\":\"client\",\"text\":\"127.0.0.1\""));
    assert!(output.contains("\"kind\":\"status\",\"text\":\"200\""));
}

#[test]
fn test_html_output_mode_wraps_the_document() {
    let result = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "html"],
        &output_modes_example(),
    );

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(output.starts_with("<!DOCTYPE html>"));
    assert!(output.contains("<pre class=\"splash\">"));
    assert!(output.contains("<span class=\"splash-ip\">192.168.1.10</span>"));
    assert!(output.trim_end().ends_with("</html>"));
}

#[test]
fn test_ansi_is_the_default_output_mode() {
    let with_flag = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "ansi"],
        &output_modes_example(),
    );
    let without_flag = run_splash_with_stdin_args(&["--mode", "ad-hoc"], &output_modes_example());

    assert!(with_flag.is_ok() && without_flag.is_ok());
    assert_eq!(with_flag.unwrap(), without_flag.unwrap());
}

#[test]
fn test_unknown_output_mode_is_rejected() {
    let result = run_splash(&["--output", "sparkles"]);

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(
        !output.status.success(),
        "Unknown output mode should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Unknown output mode 'sparkles'"),
        "stderr was: {}",
        stderr
    );
}

#[test]
fn test_curses_output_mode_requires_a_terminal() {
    let example = example_path("viewer_scroll.log");
    let result = run_splash(&[
        "--mode",
        "ad-hoc",
        "--output",
        "curses",
        "--path",
        example.to_str().unwrap(),
    ]);

    assert!(result.is_ok());
    let output = result.unwrap();

    assert!(
        !output.status.success(),
        "Redirected curses output should exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("curses output needs a terminal"),
        "stderr was: {}",
        stderr
    );
}

// ==================== Configuration Tests ====================

#[test]
fn test_list_themes_names_every_preset() {
    let home = tempfile::TempDir::new().unwrap();
    let output = run_splash_with_home(home.path(), &["--list-themes"], "").unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    for name in ["dark", "light", "solarized", "dracula"] {
        assert!(stdout.contains(name), "stdout was: {}", stdout);
    }
}

#[test]
fn test_a_theme_changes_the_colors_of_the_output() {
    let home = tempfile::TempDir::new().unwrap();
    let contents = example_contents("config_sample.log");

    let dark = run_splash_with_home(
        home.path(),
        &["--mode", "clf", "--output", "html"],
        &contents,
    )
    .unwrap();
    let light = run_splash_with_home(
        home.path(),
        &["--mode", "clf", "--output", "html", "--theme", "light"],
        &contents,
    )
    .unwrap();

    assert!(light.status.success());
    assert!(String::from_utf8_lossy(&dark.stdout).contains(".splash-client { color: #ff5555; }"));
    assert!(String::from_utf8_lossy(&light.stdout).contains(".splash-client { color: #aa0000; }"));
}

#[test]
fn test_a_color_override_repaints_one_field() {
    let home = tempfile::TempDir::new().unwrap();
    let contents = example_contents("config_sample.log");

    let output = run_splash_with_home(
        home.path(),
        &[
            "--mode",
            "clf",
            "--output",
            "html",
            "--color",
            "client=#00ff00",
        ],
        &contents,
    )
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains(".splash-client { color: #00ff00; }"),
        "stdout was: {}",
        stdout
    );
}

#[test]
fn test_an_invalid_color_override_exits_non_zero() {
    let home = tempfile::TempDir::new().unwrap();

    let output = run_splash_with_home(home.path(), &["--color", "ip=mauve"], "").unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("Unknown color 'mauve'"),
        "stderr was: {}",
        stderr
    );
}

#[test]
fn test_a_config_file_supplies_the_mode_output_and_theme() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".splash")).unwrap();
    std::fs::write(
        home.path().join(".splash/config.toml"),
        "mode = \"clf\"\noutput = \"html\"\ntheme = \"dracula\"\n",
    )
    .unwrap();

    let output =
        run_splash_with_home(home.path(), &[], &example_contents("config_sample.log")).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("background: #282a36;"),
        "stdout was: {}",
        stdout
    );
    assert!(stdout.contains("<span class=\"splash-client\">192.168.10.5</span>"));
}

#[test]
fn test_a_splashrc_file_is_read_when_there_is_no_config_toml() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::write(home.path().join(".splashrc"), "output = \"plain\"\n").unwrap();

    let output =
        run_splash_with_home(home.path(), &[], &example_contents("config_sample.log")).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(!stdout.contains('\u{1b}'), "stdout was: {}", stdout);
}

#[test]
fn test_a_broken_config_file_exits_non_zero() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::write(home.path().join(".splashrc"), "output plain\n").unwrap();

    let output = run_splash_with_home(home.path(), &[], "").unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("expected a key = value pair"),
        "stderr was: {}",
        stderr
    );
}

#[test]
fn test_a_named_config_file_replaces_the_default_locations() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::write(home.path().join(".splashrc"), "output = \"json\"\n").unwrap();
    let named = home.path().join("other.toml");
    std::fs::write(&named, "output = \"plain\"\n").unwrap();

    let output = run_splash_with_home(
        home.path(),
        &["--config", named.to_str().unwrap()],
        "a line\n",
    )
    .unwrap();

    assert_eq!(String::from_utf8_lossy(&output.stdout), "a line\n");
}

#[test]
fn test_a_saved_profile_is_listed_and_reused() {
    let home = tempfile::TempDir::new().unwrap();

    let saved = run_splash_with_home(
        home.path(),
        &[
            "--theme",
            "light",
            "--color",
            "client=#00ff00",
            "--save-profile",
            "day",
        ],
        "",
    )
    .unwrap();

    assert!(saved.status.success());
    assert!(home.path().join(".splash/profiles/day.toml").is_file());

    let listed = run_splash_with_home(home.path(), &["--list-profiles"], "").unwrap();
    assert!(String::from_utf8_lossy(&listed.stdout).contains("  day\n"));

    let used = run_splash_with_home(
        home.path(),
        &["--mode", "clf", "--output", "html", "--profile", "day"],
        &example_contents("config_sample.log"),
    )
    .unwrap();
    let stdout = String::from_utf8_lossy(&used.stdout);

    assert!(used.status.success());
    assert!(
        stdout.contains(".splash-client { color: #00ff00; }"),
        "stdout was: {}",
        stdout
    );
}

#[test]
fn test_a_missing_profile_exits_non_zero() {
    let home = tempfile::TempDir::new().unwrap();

    let output = run_splash_with_home(home.path(), &["--profile", "night"], "").unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("Color profile 'night' not found"),
        "stderr was: {}",
        stderr
    );
}

#[test]
fn test_list_plugins_shows_the_configured_plugin_settings() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::write(
        home.path().join(".splashrc"),
        "[plugins.syslog]\nenabled = \"false\"\n",
    )
    .unwrap();

    let output = run_splash_with_home(home.path(), &["--list-plugins"], "").unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("  syslog (disabled)\n"),
        "stdout was: {}",
        stdout
    );
}

#[test]
fn test_a_profile_that_cannot_be_written_exits_non_zero() {
    let home = tempfile::TempDir::new().unwrap();

    let output = run_splash_with_home(home.path(), &["--save-profile", "nested/day"], "").unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("Could not write"), "stderr was: {}", stderr);
}

#[test]
fn test_disabling_a_plugin_reports_that_it_is_not_yet_supported() {
    let home = tempfile::TempDir::new().unwrap();

    let output = run_splash_with_home(home.path(), &["--disable-plugin", "syslog"], "").unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("Disabling plugin: syslog\n"),
        "stdout was: {}",
        stdout
    );
}

#[test]
fn test_choosing_a_plugin_reports_that_it_is_not_yet_supported() {
    let home = tempfile::TempDir::new().unwrap();

    let output = run_splash_with_home(home.path(), &["--plugin", "syslog"], "a line\n").unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("Using plugin: syslog\n"),
        "stdout was: {}",
        stdout
    );
    assert!(stdout.contains("a line\n"), "stdout was: {}", stdout);
}

// ==================== Performance Tests ====================

#[test]
fn test_worker_threads_render_the_same_output_as_one_thread() {
    let example = example_path("jobs_sample.log");
    let path = example.to_str().unwrap();

    let one = run_splash_file_args(&["--mode", "clf", "--jobs", "1", "--path", path]).unwrap();
    let many = run_splash_file_args(&["--mode", "clf", "--jobs", "4", "--path", path]).unwrap();

    assert!(!one.is_empty());
    assert_eq!(one, many);
}

#[test]
fn test_a_memory_mapped_file_renders_every_line() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("large.log");
    let sample = splash::bench::sample_log(5_000);
    std::fs::write(&path, &sample).unwrap();

    let output = run_splash_file_to_file(
        &[
            "--mode",
            "clf",
            "--output",
            "plain",
            "--jobs",
            "4",
            "--path",
            path.to_str().unwrap(),
        ],
        &directory.path().join("rendered.txt"),
    )
    .unwrap();

    assert!(path.metadata().unwrap().len() >= splash::source::MMAP_THRESHOLD);
    assert_eq!(output.lines().count(), 5_000);
    assert_eq!(output.lines().next(), sample.lines().next());
    assert_eq!(output.lines().last(), sample.lines().last());
}

#[test]
fn test_a_worker_count_of_zero_exits_non_zero() {
    let home = tempfile::TempDir::new().unwrap();

    let output = run_splash_with_home(home.path(), &["--jobs", "0"], "").unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("Invalid worker count '0'"),
        "stderr was: {}",
        stderr
    );
}

// ==================== Apache and nginx Tests ====================

#[test]
fn test_httpd_mode_colors_the_referer_and_user_agent_of_a_combined_log() {
    let example = example_path("httpd_combined.log");
    let output = run_splash_file_args(&[
        "--mode",
        "httpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"referer","text":"http://www.example.com/start.html"}"#),
        "the referer should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"user_agent","text":"curl/8.4.0"}"#),
        "the user agent should be colored as its own field"
    );
}

#[test]
fn test_httpd_mode_reads_every_line_of_a_combined_log() {
    let example = example_path("httpd_combined.log");
    let output = run_splash_with_file("httpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_httpd_mode_colors_the_server_name_of_a_vhost_combined_log() {
    let example = example_path("httpd_vhost_combined.log");
    let output = run_splash_file_args(&[
        "--mode",
        "httpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"vhost","text":"reports.example.com:443"}"#),
        "the server name should be colored as its own field"
    );
}

#[test]
fn test_httpd_mode_colors_the_request_time_of_an_extended_log() {
    let example = example_path("httpd_extended.log");
    let output = run_splash_file_args(&[
        "--mode",
        "httpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"duration","text":"48210"}"#),
        "the trailing request time should be colored as a duration"
    );
}

#[test]
fn test_httpd_mode_separates_the_module_and_level_of_an_apache_error_log() {
    let example = example_path("httpd_apache_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "httpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"proxy_fcgi"}"#),
        "the module should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"level","text":"warn"}"#),
        "the level should be colored as its own field"
    );
}

#[test]
fn test_httpd_mode_colors_the_worker_of_an_nginx_error_log() {
    let example = example_path("httpd_nginx_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "httpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"pid","text":"1200#0"}"#),
        "the worker and thread should be colored as one field"
    );
}

#[test]
fn test_httpd_mode_reads_every_line_of_an_nginx_error_log() {
    let example = example_path("httpd_nginx_error.log");
    let output = run_splash_with_file("httpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_httpd_mode_drops_a_line_that_is_not_a_web_server_log() {
    let example = example_path("httpd_mixed.log");
    let output = run_splash_with_file("httpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_httpd_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("httpd_mixed.log");
    let contents = example_contents("httpd_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "httpd",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_apache_and_nginx_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("httpd v1.0.0"),
        "--list-plugins should name the built-in httpd plugin"
    );
}

// ==================== Squid Tests ====================

#[test]
fn test_squid_mode_colors_a_cache_hit_and_a_cache_miss_differently() {
    let example = example_path("squid_native.log");
    let output = run_splash_file_args(&[
        "--mode",
        "squid",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"cache_miss","text":"TCP_MISS"}"#),
        "a fetched request should be colored as a cache miss"
    );
    assert!(
        output.contains(r#"{"kind":"cache_hit","text":"TCP_MEM_HIT"}"#),
        "a cached request should be colored as a cache hit"
    );
}

#[test]
fn test_squid_mode_gives_a_denied_request_the_neutral_result_style() {
    let example = example_path("squid_native.log");
    let output = run_splash_file_args(&[
        "--mode",
        "squid",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"cache_result","text":"TCP_DENIED"}"#),
        "a denied request is neither a hit nor a miss"
    );
}

#[test]
fn test_squid_mode_colors_the_hierarchy_and_content_type_of_a_native_log() {
    let example = example_path("squid_native.log");
    let output = run_splash_file_args(&[
        "--mode",
        "squid",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"hierarchy","text":"HIER_DIRECT"}"#),
        "the hierarchy code should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"content_type","text":"application/javascript"}"#),
        "the content type should be colored as its own field"
    );
}

#[test]
fn test_squid_mode_reads_every_line_of_a_native_log() {
    let example = example_path("squid_native.log");
    let output = run_splash_with_file("squid", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_squid_mode_reads_the_result_code_appended_to_an_httpd_emulated_log() {
    let example = example_path("squid_emulated.log");
    let output = run_splash_file_args(&[
        "--mode",
        "squid",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"cache_hit","text":"TCP_MEM_HIT"}"#),
        "the appended result code should be colored as a cache hit"
    );
}

#[test]
fn test_squid_mode_reads_every_line_of_an_httpd_emulated_log() {
    let example = example_path("squid_emulated.log");
    let output = run_splash_with_file("squid", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_squid_mode_drops_a_line_that_is_not_an_access_log() {
    let example = example_path("squid_mixed.log");
    let output = run_splash_with_file("squid", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in neither format should be dropped"
    );
}

#[test]
fn test_squid_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("squid_mixed.log");
    let contents = example_contents("squid_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "squid",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_squid_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("squid v1.0.0"),
        "--list-plugins should name the built-in squid plugin"
    );
}

// ==================== Varnish Tests ====================

#[test]
fn test_varnish_mode_colors_the_transaction_a_header_opens() {
    let example = example_path("varnish_transaction.log");
    let output = run_splash_file_args(&[
        "--mode",
        "varnish",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"transaction","text":"Request"}"#),
        "the client transaction should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"transaction","text":"BeReq"}"#),
        "the backend transaction should be colored as its own field"
    );
}

#[test]
fn test_varnish_mode_colors_a_record_tag_apart_from_its_payload() {
    let example = example_path("varnish_transaction.log");
    let output = run_splash_file_args(&[
        "--mode",
        "varnish",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"tag","text":"ReqMethod"}"#),
        "the tag should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"method","text":"GET"}"#),
        "the payload of a method record should be colored as a method"
    );
}

#[test]
fn test_varnish_mode_colors_a_header_name_apart_from_its_value() {
    let example = example_path("varnish_transaction.log");
    let output = run_splash_file_args(&[
        "--mode",
        "varnish",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"header","text":"User-Agent"}"#),
        "the header name should be colored as its own field"
    );
}

#[test]
fn test_varnish_mode_reads_every_line_of_a_transaction_log() {
    let example = example_path("varnish_transaction.log");
    let output = run_splash_with_file("varnish", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 25);
}

#[test]
fn test_varnish_mode_colors_the_side_a_raw_record_came_from() {
    let example = example_path("varnish_raw.log");
    let output = run_splash_file_args(&[
        "--mode",
        "varnish",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"transaction","text":"b"}"#),
        "a backend record should be marked as coming from the backend"
    );
    assert!(
        output.contains(r#"{"kind":"status","text":"503"}"#),
        "the payload of a backend status record should be colored as a status"
    );
}

#[test]
fn test_varnish_mode_reads_every_line_of_a_raw_log() {
    let example = example_path("varnish_raw.log");
    let output = run_splash_with_file("varnish", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_varnish_mode_drops_a_line_that_is_not_a_varnish_log() {
    let example = example_path("varnish_mixed.log");
    let output = run_splash_with_file("varnish", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_varnish_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("varnish_mixed.log");
    let contents = example_contents("varnish_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "varnish",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_varnish_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("varnish v1.0.0"),
        "--list-plugins should name the built-in varnish plugin"
    );
}

// ==================== HAProxy Tests ====================

#[test]
fn test_haproxy_mode_colors_the_frontend_backend_and_server_of_a_connection() {
    let example = example_path("haproxy_http.log");
    let output = run_splash_file_args(&[
        "--mode",
        "haproxy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"frontend","text":"http-in"}"#),
        "the frontend should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"backend","text":"api"}"#),
        "the backend should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"server","text":"srv2"}"#),
        "the server should be colored as its own field"
    );
}

#[test]
fn test_haproxy_mode_colors_the_timers_and_termination_state_of_a_connection() {
    let example = example_path("haproxy_http.log");
    let output = run_splash_file_args(&[
        "--mode",
        "haproxy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"timers","text":"10/0/30/69/109"}"#),
        "the timers should be colored as one field"
    );
    assert!(
        output.contains(r#"{"kind":"termination","text":"SC--"}"#),
        "the termination state should be colored as its own field"
    );
}

#[test]
fn test_haproxy_mode_colors_the_request_an_http_log_ends_with() {
    let example = example_path("haproxy_http.log");
    let output = run_splash_file_args(&[
        "--mode",
        "haproxy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"request","text":"/api/orders"}"#),
        "the requested path should be colored as a request"
    );
}

#[test]
fn test_haproxy_mode_reads_every_line_of_an_http_log() {
    let example = example_path("haproxy_http.log");
    let output = run_splash_with_file("haproxy", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_haproxy_mode_colors_the_shorter_termination_state_of_a_tcp_log() {
    let example = example_path("haproxy_tcp.log");
    let output = run_splash_file_args(&[
        "--mode",
        "haproxy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"termination","text":"cD"}"#),
        "a tcp termination state is two characters"
    );
}

#[test]
fn test_haproxy_mode_reads_every_line_of_a_tcp_log() {
    let example = example_path("haproxy_tcp.log");
    let output = run_splash_with_file("haproxy", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_haproxy_mode_colors_the_listener_that_refused_a_connection() {
    let example = example_path("haproxy_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "haproxy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"frontend","text":"https-in"}"#),
        "the frontend should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"server","text":"ssl"}"#),
        "the listener should be colored as its own field"
    );
}

#[test]
fn test_haproxy_mode_reads_every_line_of_an_error_log() {
    let example = example_path("haproxy_error.log");
    let output = run_splash_with_file("haproxy", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_haproxy_mode_drops_a_line_that_is_not_a_haproxy_log() {
    let example = example_path("haproxy_mixed.log");
    let output = run_splash_with_file("haproxy", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_haproxy_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("haproxy_mixed.log");
    let contents = example_contents("haproxy_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "haproxy",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_haproxy_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("haproxy v1.0.0"),
        "--list-plugins should name the built-in haproxy plugin"
    );
}

// ==================== Caddy Tests ====================

#[test]
fn test_caddy_mode_colors_the_fields_nested_inside_the_request() {
    let example = example_path("caddy_access.log");
    let output = run_splash_file_args(&[
        "--mode",
        "caddy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"ip","text":"10.0.0.42"}"#),
        "the remote address should be colored as an address"
    );
    assert!(
        output.contains(r#"{"kind":"method","text":"POST"}"#),
        "the method should be colored as a method"
    );
    assert!(
        output.contains(r#"{"kind":"request","text":"/orders"}"#),
        "the uri should be colored as a request"
    );
}

#[test]
fn test_caddy_mode_colors_the_status_size_and_duration_of_a_request() {
    let example = example_path("caddy_access.log");
    let output = run_splash_file_args(&[
        "--mode",
        "caddy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"status","text":"404"}"#),
        "the status should be colored as a status"
    );
    assert!(
        output.contains(r#"{"kind":"size","text":"10900"}"#),
        "the response size should be colored as a size"
    );
    assert!(
        output.contains(r#"{"kind":"duration","text":"0.048211"}"#),
        "how long the request took should be colored as a duration"
    );
}

#[test]
fn test_caddy_mode_colors_every_key_of_the_object_as_a_key() {
    let example = example_path("caddy_access.log");
    let output = run_splash_file_args(&[
        "--mode",
        "caddy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"header","text":"Content-Type"}"#),
        "a header name nested in the request should be colored as a key"
    );
}

#[test]
fn test_caddy_mode_reads_every_line_of_an_access_log() {
    let example = example_path("caddy_access.log");
    let output = run_splash_with_file("caddy", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_caddy_mode_colors_the_level_and_the_error_of_an_error_log() {
    let example = example_path("caddy_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "caddy",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"level","text":"warn"}"#),
        "the level should be colored as a level"
    );
    assert!(
        output.contains(r#"{"kind":"message","text":"context canceled"}"#),
        "the error should be colored as message text"
    );
}

#[test]
fn test_caddy_mode_reads_every_line_of_an_error_log() {
    let example = example_path("caddy_error.log");
    let output = run_splash_with_file("caddy", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_caddy_mode_drops_a_line_that_is_not_json() {
    let example = example_path("caddy_mixed.log");
    let output = run_splash_with_file("caddy", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line that is not a json object should be dropped"
    );
}

#[test]
fn test_caddy_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("caddy_mixed.log");
    let contents = example_contents("caddy_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "caddy",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_caddy_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("caddy v1.0.0"),
        "--list-plugins should name the built-in caddy plugin"
    );
}

// ==================== Postfix Tests ====================

#[test]
fn test_postfix_mode_colors_the_queue_id_and_the_recipient_of_a_delivery() {
    let example = example_path("postfix_delivery.log");
    let output = run_splash_file_args(&[
        "--mode",
        "postfix",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"queue_id","text":"4F2A1C0123"}"#),
        "the queue id should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"email","text":"bob@example.org"}"#),
        "the recipient should be colored as an email"
    );
}

#[test]
fn test_postfix_mode_colors_whether_each_mail_was_sent_deferred_or_bounced() {
    let example = example_path("postfix_failures.log");
    let output = run_splash_file_args(&[
        "--mode",
        "postfix",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"warning","text":"deferred"}"#),
        "a deferred mail should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"bounced"}"#),
        "a bounced mail should be colored as a failure"
    );
}

#[test]
fn test_postfix_mode_colors_the_relay_and_its_address() {
    let example = example_path("postfix_delivery.log");
    let output = run_splash_file_args(&[
        "--mode",
        "postfix",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"host","text":"mx.example.org"}"#),
        "the relay should be colored as a host"
    );
    assert!(
        output.contains(r#"{"kind":"ip","text":"93.184.216.34"}"#),
        "the relay address should be colored as an address"
    );
}

#[test]
fn test_postfix_mode_reads_every_line_of_a_delivery_log() {
    let example = example_path("postfix_delivery.log");
    let output = run_splash_with_file("postfix", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_postfix_mode_reads_every_line_of_a_failures_log() {
    let example = example_path("postfix_failures.log");
    let output = run_splash_with_file("postfix", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_postfix_mode_drops_a_line_that_is_not_a_postfix_log() {
    let example = example_path("postfix_mixed.log");
    let output = run_splash_with_file("postfix", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_postfix_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("postfix_mixed.log");
    let contents = example_contents("postfix_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "postfix",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_postfix_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("postfix v1.0.0"),
        "--list-plugins should name the built-in postfix plugin"
    );
}

// ==================== Exim Tests ====================

#[test]
fn test_exim_mode_colors_the_message_id_and_the_arrival_flag() {
    let example = example_path("exim_mainlog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "exim",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"queue_id","text":"1qnXYZ-000ABC-12"}"#),
        "the message id should be colored as its own field"
    );
    assert!(
        output.contains(r#"{"kind":"success","text":"<="}"#),
        "an arrival should be colored as a success"
    );
}

#[test]
fn test_exim_mode_colors_deferred_and_failed_deliveries() {
    let example = example_path("exim_mainlog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "exim",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"warning","text":"=="}"#),
        "a deferral should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"**"}"#),
        "a failed delivery should be colored as a failure"
    );
}

#[test]
fn test_exim_mode_colors_the_router_and_the_transport() {
    let example = example_path("exim_mainlog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "exim",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"dnslookup"}"#),
        "the router should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"module","text":"remote_smtp"}"#),
        "the transport should be colored as a module"
    );
}

#[test]
fn test_exim_mode_reads_every_line_of_a_mainlog_log() {
    let example = example_path("exim_mainlog.log");
    let output = run_splash_with_file("exim", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_exim_mode_drops_a_line_that_is_not_a_exim_log() {
    let example = example_path("exim_mixed.log");
    let output = run_splash_with_file("exim", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_exim_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("exim_mixed.log");
    let contents = example_contents("exim_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "exim",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_exim_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("exim v1.0.0"),
        "--list-plugins should name the built-in exim plugin"
    );
}

// ==================== Fetchmail Tests ====================

#[test]
fn test_fetchmail_mode_colors_the_user_and_server_a_poll_was_for() {
    let example = example_path("fetchmail_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "fetchmail",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"userid","text":"alice"}"#),
        "the user should be colored as a user"
    );
    assert!(
        output.contains(r#"{"kind":"host","text":"mail.example.com"}"#),
        "the server should be colored as a host"
    );
}

#[test]
fn test_fetchmail_mode_colors_whether_each_message_was_flushed() {
    let example = example_path("fetchmail_file.log");
    let output = run_splash_file_args(&[
        "--mode",
        "fetchmail",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"success","text":"flushed"}"#),
        "a flushed message should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"warning","text":"not flushed"}"#),
        "a message left on the server should be colored as a warning"
    );
}

#[test]
fn test_fetchmail_mode_colors_an_authentication_failure() {
    let example = example_path("fetchmail_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "fetchmail",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"failure","text":"AUTHFAIL"}"#),
        "an authentication failure should be colored as a failure"
    );
}

#[test]
fn test_fetchmail_mode_reads_every_line_of_a_syslog_log() {
    let example = example_path("fetchmail_syslog.log");
    let output = run_splash_with_file("fetchmail", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_fetchmail_mode_reads_every_line_of_a_file_log() {
    let example = example_path("fetchmail_file.log");
    let output = run_splash_with_file("fetchmail", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_fetchmail_mode_drops_a_line_that_is_not_a_fetchmail_log() {
    let example = example_path("fetchmail_mixed.log");
    let output = run_splash_with_file("fetchmail", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_fetchmail_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("fetchmail_mixed.log");
    let contents = example_contents("fetchmail_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "fetchmail",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_fetchmail_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("fetchmail v1.0.0"),
        "--list-plugins should name the built-in fetchmail plugin"
    );
}

// ==================== Dovecot Tests ====================

#[test]
fn test_dovecot_mode_colors_the_service_user_and_session_of_a_login() {
    let example = example_path("dovecot_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "dovecot",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"imap-login"}"#),
        "the service should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"userid","text":"alice"}"#),
        "the user should be colored as a user"
    );
    assert!(
        output.contains(r#"{"kind":"transaction","text":"Xy7AbC"}"#),
        "the session id should be colored as its own field"
    );
}

#[test]
fn test_dovecot_mode_colors_a_failed_login() {
    let example = example_path("dovecot_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "dovecot",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"failure","text":"auth failed"}"#),
        "a failed login should be colored as a failure"
    );
}

#[test]
fn test_dovecot_mode_colors_the_level_of_a_log_file_line() {
    let example = example_path("dovecot_file.log");
    let output = run_splash_file_args(&[
        "--mode",
        "dovecot",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"level","text":"Info"}"#),
        "the level should be colored as a level"
    );
    assert!(
        output.contains(r#"{"kind":"level","text":"Warning"}"#),
        "the level should be colored as a level"
    );
}

#[test]
fn test_dovecot_mode_reads_every_line_of_a_syslog_log() {
    let example = example_path("dovecot_syslog.log");
    let output = run_splash_with_file("dovecot", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_dovecot_mode_reads_every_line_of_a_file_log() {
    let example = example_path("dovecot_file.log");
    let output = run_splash_with_file("dovecot", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_dovecot_mode_drops_a_line_that_is_not_a_dovecot_log() {
    let example = example_path("dovecot_mixed.log");
    let output = run_splash_with_file("dovecot", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_dovecot_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("dovecot_mixed.log");
    let contents = example_contents("dovecot_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "dovecot",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_dovecot_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("dovecot v1.0.0"),
        "--list-plugins should name the built-in dovecot plugin"
    );
}

// ==================== Procmail Tests ====================

#[test]
fn test_procmail_mode_colors_the_sender_folder_and_size_of_a_delivered_mail() {
    let example = example_path("procmail_abstract.log");
    let output = run_splash_file_args(&[
        "--mode",
        "procmail",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"email","text":"alice@example.com"}"#),
        "the sender should be colored as an email"
    );
    assert!(
        output.contains(r#"{"kind":"path","text":"/home/bob/Mail/inbox"}"#),
        "the folder should be colored as a path"
    );
    assert!(
        output.contains(r#"{"kind":"size","text":"4512"}"#),
        "the size should be colored as a size"
    );
}

#[test]
fn test_procmail_mode_colors_matched_and_unmatched_recipes() {
    let example = example_path("procmail_verbose.log");
    let output = run_splash_file_args(&[
        "--mode",
        "procmail",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"success","text":"Match on"}"#),
        "a matched recipe should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"warning","text":"No match on"}"#),
        "an unmatched recipe should be colored as a warning"
    );
}

#[test]
fn test_procmail_mode_colors_a_failure_to_deliver() {
    let example = example_path("procmail_verbose.log");
    let output = run_splash_file_args(&[
        "--mode",
        "procmail",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"failure","text":"Couldn't"}"#),
        "a failure should be colored as a failure"
    );
}

#[test]
fn test_procmail_mode_reads_every_line_of_a_abstract_log() {
    let example = example_path("procmail_abstract.log");
    let output = run_splash_with_file("procmail", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_procmail_mode_reads_every_line_of_a_verbose_log() {
    let example = example_path("procmail_verbose.log");
    let output = run_splash_with_file("procmail", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_procmail_mode_drops_a_line_that_is_not_a_procmail_log() {
    let example = example_path("procmail_mixed.log");
    let output = run_splash_with_file("procmail", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_procmail_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("procmail_mixed.log");
    let contents = example_contents("procmail_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "procmail",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_procmail_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("procmail v1.0.0"),
        "--list-plugins should name the built-in procmail plugin"
    );
}

// ==================== vsftpd Tests ====================

#[test]
fn test_vsftpd_mode_colors_the_user_file_and_size_of_a_download() {
    let example = example_path("vsftpd_file.log");
    let output = run_splash_file_args(&[
        "--mode",
        "vsftpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"userid","text":"alice"}"#),
        "the user should be colored as a user id"
    );
    assert!(
        output.contains(r#"{"kind":"path","text":"/home/alice/report.pdf"}"#),
        "the file should be colored as a path"
    );
    assert!(
        output.contains(r#"{"kind":"size","text":"4096"}"#),
        "the bytes should be colored as a size"
    );
}

#[test]
fn test_vsftpd_mode_colors_whether_each_event_succeeded() {
    let example = example_path("vsftpd_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "vsftpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"success","text":"OK"}"#),
        "a succeeded event should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"FAIL"}"#),
        "a failed event should be colored as a failure"
    );
}

#[test]
fn test_vsftpd_mode_reads_every_line_of_a_log_file() {
    let example = example_path("vsftpd_file.log");
    let output = run_splash_with_file("vsftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_vsftpd_mode_reads_every_line_of_a_syslog_log() {
    let example = example_path("vsftpd_syslog.log");
    let output = run_splash_with_file("vsftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_vsftpd_mode_drops_a_line_that_is_not_a_vsftpd_log() {
    let example = example_path("vsftpd_mixed.log");
    let output = run_splash_with_file("vsftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_vsftpd_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("vsftpd_mixed.log");
    let contents = example_contents("vsftpd_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "vsftpd",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_vsftpd_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("vsftpd v1.0.0"),
        "--list-plugins should name the built-in vsftpd plugin"
    );
}

// ==================== ProFTPD Tests ====================

#[test]
fn test_proftpd_mode_colors_the_server_client_and_login_result() {
    let example = example_path("proftpd_system.log");
    let output = run_splash_file_args(&[
        "--mode",
        "proftpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"vhost","text":"ftp.example.com"}"#),
        "the server should be colored as a virtual host"
    );
    assert!(
        output.contains(r#"{"kind":"host","text":"client.example.org"}"#),
        "the client name should be colored as a host"
    );
    assert!(
        output.contains(r#"{"kind":"success","text":"Login successful"}"#),
        "a successful login should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"Incorrect password"}"#),
        "a failed login should be colored as a failure"
    );
}

#[test]
fn test_proftpd_mode_colors_the_commands_of_an_extended_log() {
    let example = example_path("proftpd_extended.log");
    let output = run_splash_file_args(&[
        "--mode",
        "proftpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"method","text":"RETR"}"#),
        "the command should be colored as a method"
    );
    assert!(
        output.contains(r#"{"kind":"status","text":"226"}"#),
        "the reply code should be colored as a status"
    );
}

#[test]
fn test_proftpd_mode_reads_every_line_of_a_system_log() {
    let example = example_path("proftpd_system.log");
    let output = run_splash_with_file("proftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_proftpd_mode_reads_every_line_of_a_syslog_log() {
    let example = example_path("proftpd_syslog.log");
    let output = run_splash_with_file("proftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_proftpd_mode_reads_every_line_of_an_extended_log() {
    let example = example_path("proftpd_extended.log");
    let output = run_splash_with_file("proftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_proftpd_mode_drops_a_line_that_is_not_a_proftpd_log() {
    let example = example_path("proftpd_mixed.log");
    let output = run_splash_with_file("proftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_proftpd_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("proftpd_mixed.log");
    let contents = example_contents("proftpd_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "proftpd",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_proftpd_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("proftpd v1.0.0"),
        "--list-plugins should name the built-in proftpd plugin"
    );
}

// ==================== pure-ftpd Tests ====================

#[test]
fn test_pureftpd_mode_colors_a_download_and_a_partial_upload() {
    let example = example_path("pureftpd_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "pure-ftpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"success","text":"downloaded"}"#),
        "a download should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"warning","text":"partially uploaded"}"#),
        "a partial upload should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"level","text":"NOTICE"}"#),
        "the level should be colored as a level"
    );
}

#[test]
fn test_pureftpd_mode_colors_the_records_of_a_w3c_transfer_log() {
    let example = example_path("pureftpd_altlog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "pure-ftpd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"header","text":"Fields"}"#),
        "a header name should be colored as a header"
    );
    assert!(
        output.contains(r#"{"kind":"method","text":"sent"}"#),
        "the action should be colored as a method"
    );
    assert!(
        output.contains(r#"{"kind":"path","text":"/home/alice/report.pdf"}"#),
        "the file should be colored as a path"
    );
}

#[test]
fn test_pureftpd_mode_reads_every_line_of_a_syslog_log() {
    let example = example_path("pureftpd_syslog.log");
    let output = run_splash_with_file("pure-ftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_pureftpd_mode_reads_every_line_of_a_transfer_log() {
    let example = example_path("pureftpd_altlog.log");
    let output = run_splash_with_file("pure-ftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_pureftpd_mode_drops_a_line_that_is_not_a_pureftpd_log() {
    let example = example_path("pureftpd_mixed.log");
    let output = run_splash_with_file("pure-ftpd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_pureftpd_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("pureftpd_mixed.log");
    let contents = example_contents("pureftpd_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "pure-ftpd",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_pureftpd_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("pure-ftpd v1.0.0"),
        "--list-plugins should name the built-in pure-ftpd plugin"
    );
}

// ==================== xferlog Tests ====================

#[test]
fn test_xferlog_mode_colors_the_file_and_whether_each_transfer_completed() {
    let example = example_path("xferlog_transfers.log");
    let output = run_splash_file_args(&[
        "--mode",
        "xferlog",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"path","text":"/pub/release notes.txt"}"#),
        "a file name holding a space should be one path"
    );
    assert!(
        output.contains(r#"{"kind":"success","text":"c"}"#),
        "a completed transfer should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"i"}"#),
        "an incomplete transfer should be colored as a failure"
    );
}

#[test]
fn test_xferlog_mode_reads_every_line_of_a_transfer_log() {
    let example = example_path("xferlog_transfers.log");
    let output = run_splash_with_file("xferlog", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_xferlog_mode_drops_a_line_that_is_not_a_xferlog_log() {
    let example = example_path("xferlog_mixed.log");
    let output = run_splash_with_file("xferlog", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_xferlog_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("xferlog_mixed.log");
    let contents = example_contents("xferlog_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "xferlog",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_xferlog_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("xferlog v1.0.0"),
        "--list-plugins should name the built-in xferlog plugin"
    );
}

// ==================== ftpstats Tests ====================

#[test]
fn test_ftpstats_mode_colors_the_session_direction_and_file() {
    let example = example_path("ftpstats_transfers.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ftpstats",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"transaction","text":"651c0b41.10e1"}"#),
        "the session should be colored as a transaction"
    );
    assert!(
        output.contains(r#"{"kind":"method","text":"U"}"#),
        "the direction should be colored as a method"
    );
    assert!(
        output.contains(r#"{"kind":"path","text":"/pub/release.tar"}"#),
        "the file should be colored as a path"
    );
}

#[test]
fn test_ftpstats_mode_reads_every_line_of_a_transfer_log() {
    let example = example_path("ftpstats_transfers.log");
    let output = run_splash_with_file("ftpstats", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_ftpstats_mode_drops_a_line_that_is_not_a_ftpstats_log() {
    let example = example_path("ftpstats_mixed.log");
    let output = run_splash_with_file("ftpstats", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_ftpstats_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("ftpstats_mixed.log");
    let contents = example_contents("ftpstats_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ftpstats",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_ftpstats_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("ftpstats v1.0.0"),
        "--list-plugins should name the built-in ftpstats plugin"
    );
}

// ==================== Syslog Tests ====================

#[test]
fn test_syslog_mode_colors_the_header_of_any_program() {
    let example = example_path("syslog_traditional.log");
    let output = run_splash_file_args(&[
        "--mode",
        "syslog",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"tag","text":"nginx"}"#),
        "the program should be colored as a tag"
    );
    assert!(
        output.contains(r#"{"kind":"pid","text":"2200"}"#),
        "the process id should be colored as a pid"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"error"}"#),
        "a problem word should be colored as a failure"
    );
}

#[test]
fn test_syslog_mode_colors_rfc_5424_structured_data() {
    let example = example_path("syslog_rfc5424.log");
    let output = run_splash_file_args(&[
        "--mode",
        "syslog",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"exampleSDID@32473"}"#),
        "a structured data id should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"header","text":"eventSource"}"#),
        "a parameter name should be colored as a header"
    );
    assert!(
        output.contains(r#"{"kind":"transaction","text":"ID47"}"#),
        "the message id should be colored as a transaction"
    );
}

#[test]
fn test_syslog_mode_reads_every_line_of_a_traditional_log() {
    let example = example_path("syslog_traditional.log");
    let output = run_splash_with_file("syslog", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_syslog_mode_reads_every_line_of_an_rfc_5424_log() {
    let example = example_path("syslog_rfc5424.log");
    let output = run_splash_with_file("syslog", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_syslog_mode_drops_a_line_that_is_not_a_syslog_log() {
    let example = example_path("syslog_mixed.log");
    let output = run_splash_with_file("syslog", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_syslog_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("syslog_mixed.log");
    let contents = example_contents("syslog_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "syslog",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_syslog_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("syslog v1.0.0"),
        "--list-plugins should name the built-in syslog plugin"
    );
}

// ==================== journalctl Tests ====================

#[test]
fn test_journalctl_mode_colors_units_and_what_happened_to_them() {
    let example = example_path("journalctl_short.log");
    let output = run_splash_file_args(&[
        "--mode",
        "journalctl",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"nginx.service"}"#),
        "a unit should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"success","text":"Started"}"#),
        "a started unit should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"Failed to start"}"#),
        "a unit that failed to start should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"transaction","text":"6f1d2c3b4a5968778695a4b3c2d1e0f9"}"#),
        "the boot id should be colored as a transaction"
    );
}

#[test]
fn test_journalctl_mode_reads_each_timestamp_form() {
    let example = example_path("journalctl_precise.log");
    let output = run_splash_file_args(&[
        "--mode",
        "journalctl",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"timestamp","text":"2023-10-03T12:00:01+0000"}"#),
        "a short-iso timestamp should be read"
    );
    assert!(
        output.contains(r#"{"kind":"timestamp","text":"Tue 2023-10-03 12:00:04 UTC"}"#),
        "a short-full timestamp should be read"
    );
    assert!(
        output.contains(r#"{"kind":"timestamp","text":"[    5.123456]"}"#),
        "a short-monotonic timestamp should be read"
    );
    assert!(
        output.contains(r#"{"kind":"timestamp","text":"1696334406.000123"}"#),
        "a short-unix timestamp should be read"
    );
}

#[test]
fn test_journalctl_mode_reads_every_line_of_short_output() {
    let example = example_path("journalctl_short.log");
    let output = run_splash_with_file("journalctl", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_journalctl_mode_reads_every_line_of_precise_output() {
    let example = example_path("journalctl_precise.log");
    let output = run_splash_with_file("journalctl", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_journalctl_mode_drops_a_line_that_is_not_a_journalctl_log() {
    let example = example_path("journalctl_mixed.log");
    let output = run_splash_with_file("journalctl", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_journalctl_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("journalctl_mixed.log");
    let contents = example_contents("journalctl_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "journalctl",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_journalctl_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("journalctl v1.0.0"),
        "--list-plugins should name the built-in journalctl plugin"
    );
}

// ==================== dmesg Tests ====================

#[test]
fn test_dmesg_mode_colors_subsystems_and_problems() {
    let example = example_path("dmesg_default.log");
    let output = run_splash_file_args(&[
        "--mode",
        "dmesg",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"usb 1-1"}"#),
        "the subsystem should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"I/O error"}"#),
        "an I/O error should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"success","text":"Link is Up"}"#),
        "a link coming up should be colored as a success"
    );
}

#[test]
fn test_dmesg_mode_colors_decoded_levels() {
    let example = example_path("dmesg_decoded.log");
    let output = run_splash_file_args(&[
        "--mode",
        "dmesg",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"level","text":"info"}"#),
        "an informational level should be colored as a level"
    );
    assert!(
        output.contains(r#"{"kind":"warning","text":"warn"}"#),
        "a warning level should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"err"}"#),
        "an error level should be colored as a failure"
    );
}

#[test]
fn test_dmesg_mode_reads_every_line_of_default_output() {
    let example = example_path("dmesg_default.log");
    let output = run_splash_with_file("dmesg", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_dmesg_mode_reads_every_line_of_decoded_output() {
    let example = example_path("dmesg_decoded.log");
    let output = run_splash_with_file("dmesg", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_dmesg_mode_drops_a_line_that_is_not_a_dmesg_log() {
    let example = example_path("dmesg_mixed.log");
    let output = run_splash_with_file("dmesg", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_dmesg_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("dmesg_mixed.log");
    let contents = example_contents("dmesg_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "dmesg",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_dmesg_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("dmesg v1.0.0"),
        "--list-plugins should name the built-in dmesg plugin"
    );
}

// ==================== auth Tests ====================

#[test]
fn test_auth_mode_colors_accepted_and_failed_logins() {
    let example = example_path("auth_sshd.log");
    let output = run_splash_file_args(&[
        "--mode",
        "auth",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"success","text":"Accepted"}"#),
        "an accepted login should be colored as a success"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"Failed password"}"#),
        "a failed password should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"userid","text":"admin"}"#),
        "the user should be colored as a user id"
    );
}

#[test]
fn test_auth_mode_colors_the_command_sudo_ran() {
    let example = example_path("auth_sudo.log");
    let output = run_splash_file_args(&[
        "--mode",
        "auth",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"request","text":"/usr/bin/apt update"}"#),
        "the command should be colored as a request"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"NOT in sudoers"}"#),
        "a user not in sudoers should be colored as a failure"
    );
}

#[test]
fn test_auth_mode_reads_every_line_of_an_sshd_log() {
    let example = example_path("auth_sshd.log");
    let output = run_splash_with_file("auth", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_auth_mode_reads_every_line_of_a_sudo_log() {
    let example = example_path("auth_sudo.log");
    let output = run_splash_with_file("auth", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_auth_mode_drops_a_line_that_is_not_an_auth_log() {
    let example = example_path("auth_mixed.log");
    let output = run_splash_with_file("auth", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_auth_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("auth_mixed.log");
    let contents = example_contents("auth_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "auth",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_auth_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("auth v1.0.0"),
        "--list-plugins should name the built-in auth plugin"
    );
}

// ==================== cron Tests ====================

#[test]
fn test_cron_mode_colors_the_commands_cron_ran() {
    let example = example_path("cron_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "cron",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"method","text":"CMD"}"#),
        "the event should be colored as a method"
    );
    assert!(
        output
            .contains(r#"{"kind":"request","text":"cd / && run-parts --report /etc/cron.hourly"}"#),
        "the command should be colored as a request"
    );
    assert!(
        output.contains(r#"{"kind":"module","text":"cron.daily"}"#),
        "an anacron job should be colored as a module"
    );
}

#[test]
fn test_cron_mode_colors_cronie_log_file_events() {
    let example = example_path("cron_file.log");
    let output = run_splash_file_args(&[
        "--mode",
        "cron",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"timestamp","text":"10/03-12:00:01"}"#),
        "the date should be colored as a timestamp"
    );
    assert!(
        output.contains(r#"{"kind":"method","text":"BEGIN EDIT"}"#),
        "a two word event should be one method"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"ERROR"}"#),
        "an error should be colored as a failure"
    );
}

#[test]
fn test_cron_mode_reads_every_line_of_a_syslog_log() {
    let example = example_path("cron_syslog.log");
    let output = run_splash_with_file("cron", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_cron_mode_reads_every_line_of_a_log_file() {
    let example = example_path("cron_file.log");
    let output = run_splash_with_file("cron", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_cron_mode_drops_a_line_that_is_not_a_cron_log() {
    let example = example_path("cron_mixed.log");
    let output = run_splash_with_file("cron", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_cron_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("cron_mixed.log");
    let contents = example_contents("cron_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "cron",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_cron_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("cron v1.0.0"),
        "--list-plugins should name the built-in cron plugin"
    );
}

// ==================== ulogd Tests ====================

#[test]
fn test_ulogd_mode_colors_the_packet_fields() {
    let example = example_path("ulogd_logemu.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ulogd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"failure","text":"BLOCK"}"#),
        "a blocked packet should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"ip","text":"203.0.113.7"}"#),
        "the source should be colored as an address"
    );
    assert!(
        output.contains(r#"{"kind":"protocol","text":"TCP"}"#),
        "the protocol should be colored as a protocol"
    );
    assert!(
        output.contains(r#"{"kind":"tag","text":"SYN"}"#),
        "a flag should be colored as a tag"
    );
}

#[test]
fn test_ulogd_mode_colors_kernel_log_target_lines() {
    let example = example_path("ulogd_kernel.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ulogd",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"timestamp","text":"[12345.678901]"}"#),
        "the seconds since boot should be colored as a timestamp"
    );
    assert!(
        output.contains(r#"{"kind":"ip","text":"2001:db8::7"}"#),
        "an IPv6 source should be colored as an address"
    );
}

#[test]
fn test_ulogd_mode_reads_every_line_of_a_logemu_log() {
    let example = example_path("ulogd_logemu.log");
    let output = run_splash_with_file("ulogd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_ulogd_mode_reads_every_line_of_a_kernel_log() {
    let example = example_path("ulogd_kernel.log");
    let output = run_splash_with_file("ulogd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
}

#[test]
fn test_ulogd_mode_drops_a_line_that_is_not_a_ulogd_log() {
    let example = example_path("ulogd_mixed.log");
    let output = run_splash_with_file("ulogd", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_ulogd_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("ulogd_mixed.log");
    let contents = example_contents("ulogd_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ulogd",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_ulogd_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("ulogd v1.0.0"),
        "--list-plugins should name the built-in ulogd plugin"
    );
}

// ==================== PHP Tests ====================

#[test]
fn test_php_mode_colors_error_types_files_and_lines() {
    let example = example_path("php_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "php",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"warning","text":"Warning"}"#),
        "a warning should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"Fatal error"}"#),
        "a fatal error should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"path","text":"/var/www/html/cart.php"}"#),
        "the file should be colored as a path"
    );
}

#[test]
fn test_php_mode_colors_php_fpm_pools_and_children() {
    let example = example_path("php_fpm.log");
    let output = run_splash_file_args(&[
        "--mode",
        "php",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"www"}"#),
        "the pool should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"pid","text":"1234"}"#),
        "the child should be colored as a pid"
    );
}

#[test]
fn test_php_mode_reads_every_line_of_an_error_log() {
    let example = example_path("php_error.log");
    let output = run_splash_with_file("php", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_php_mode_reads_every_line_of_a_php_fpm_log() {
    let example = example_path("php_fpm.log");
    let output = run_splash_with_file("php", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_php_mode_drops_a_line_that_is_not_a_php_log() {
    let example = example_path("php_mixed.log");
    let output = run_splash_with_file("php", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_php_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("php_mixed.log");
    let contents = example_contents("php_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "php",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_php_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("php v1.0.0"),
        "--list-plugins should name the built-in php plugin"
    );
}

// ==================== Apache Error Tests ====================

#[test]
fn test_apache_error_mode_colors_each_field() {
    let example = example_path("apache_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "apache-error",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"failure","text":"error"}"#),
        "an error level should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"pid","text":"35708"}"#),
        "the process should be colored as a pid"
    );
    assert!(
        output.contains(r#"{"kind":"ip","text":"72.15.99.187"}"#),
        "the client address should be colored as an address"
    );
    assert!(
        output.contains(r#"{"kind":"transaction","text":"AH00128"}"#),
        "the error code should be colored as a transaction"
    );
}

#[test]
fn test_apache_error_mode_reads_every_line_of_an_error_log() {
    let example = example_path("apache_error.log");
    let output = run_splash_with_file("apache-error", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_apache_error_mode_drops_a_line_that_is_not_an_apache_error_line() {
    let example = example_path("apache_error_mixed.log");
    let output = run_splash_with_file("apache-error", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_apache_error_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("apache_error_mixed.log");
    let contents = example_contents("apache_error_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "apache-error",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_apache_error_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("apache-error v1.0.0"),
        "--list-plugins should name the built-in apache-error plugin"
    );
}

// ==================== MySQL Tests ====================

#[test]
fn test_mysql_mode_colors_error_log_labels() {
    let example = example_path("mysql_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "mysql",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"warning","text":"Warning"}"#),
        "a warning should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"ERROR"}"#),
        "an error should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"transaction","text":"MY-010068"}"#),
        "the error code should be colored as a transaction"
    );
}

#[test]
fn test_mysql_mode_colors_slow_query_times() {
    let example = example_path("mysql_slow.log");
    let output = run_splash_file_args(&[
        "--mode",
        "mysql",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"duration","text":"2.345678"}"#),
        "the query time should be colored as a duration"
    );
    assert!(
        output.contains(r#"{"kind":"userid","text":"app"}"#),
        "the user should be colored as a user id"
    );
    assert!(
        output.contains(r#"{"kind":"request","text":"use appdb;"}"#),
        "a statement should be colored as a request"
    );
}

#[test]
fn test_mysql_mode_reads_every_line_of_an_error_log() {
    let example = example_path("mysql_error.log");
    let output = run_splash_with_file("mysql", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_mysql_mode_reads_every_line_of_a_slow_query_log() {
    let example = example_path("mysql_slow.log");
    let output = run_splash_with_file("mysql", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_mysql_mode_drops_a_line_that_is_not_a_mysql_log() {
    let example = example_path("mysql_mixed.log");
    let output = run_splash_with_file("mysql", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_mysql_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("mysql_mixed.log");
    let contents = example_contents("mysql_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "mysql",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_mysql_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("mysql v1.0.0"),
        "--list-plugins should name the built-in mysql plugin"
    );
}

// ==================== PostgreSQL Tests ====================

#[test]
fn test_postgresql_mode_colors_severities_and_statements() {
    let example = example_path("postgresql_default.log");
    let output = run_splash_file_args(&[
        "--mode",
        "postgresql",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"failure","text":"FATAL"}"#),
        "a fatal message should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"duration","text":"2345.678 ms"}"#),
        "a slow statement's duration should be colored as a duration"
    );
    assert!(
        output.contains(r#"{"kind":"request","text":"SELECT * FROM users;"}"#),
        "a statement should be colored as a request"
    );
}

#[test]
fn test_postgresql_mode_colors_the_user_and_database_of_the_prefix() {
    let example = example_path("postgresql_debian.log");
    let output = run_splash_file_args(&[
        "--mode",
        "postgresql",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"userid","text":"app"}"#),
        "the user should be colored as a user id"
    );
    assert!(
        output.contains(r#"{"kind":"module","text":"appdb"}"#),
        "the database should be colored as a module"
    );
}

#[test]
fn test_postgresql_mode_reads_every_line_of_a_default_log() {
    let example = example_path("postgresql_default.log");
    let output = run_splash_with_file("postgresql", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_postgresql_mode_reads_every_line_of_a_debian_log() {
    let example = example_path("postgresql_debian.log");
    let output = run_splash_with_file("postgresql", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_postgresql_mode_drops_a_line_that_is_not_a_postgresql_log() {
    let example = example_path("postgresql_mixed.log");
    let output = run_splash_with_file("postgresql", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_postgresql_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("postgresql_mixed.log");
    let contents = example_contents("postgresql_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "postgresql",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_postgresql_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("postgresql v1.0.0"),
        "--list-plugins should name the built-in postgresql plugin"
    );
}

// ==================== Redis Tests ====================

#[test]
fn test_redis_mode_colors_roles_marks_and_readiness() {
    let example = example_path("redis_server.log");
    let output = run_splash_file_args(&[
        "--mode",
        "redis",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"M"}"#),
        "the role should be colored as a module"
    );
    assert!(
        output.contains(r##"{"kind":"warning","text":"#"}"##),
        "a warning mark should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"success","text":"Ready to accept connections"}"#),
        "readiness should be colored as a success"
    );
}

#[test]
fn test_redis_mode_reads_every_line_of_a_server_log() {
    let example = example_path("redis_server.log");
    let output = run_splash_with_file("redis", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 8);
}

#[test]
fn test_redis_mode_drops_a_line_that_is_not_a_redis_log() {
    let example = example_path("redis_mixed.log");
    let output = run_splash_with_file("redis", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_redis_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("redis_mixed.log");
    let contents = example_contents("redis_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "redis",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_redis_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("redis v1.0.0"),
        "--list-plugins should name the built-in redis plugin"
    );
}

// ==================== MongoDB Tests ====================

#[test]
fn test_mongodb_mode_colors_structured_fields() {
    let example = example_path("mongodb_json.log");
    let output = run_splash_file_args(&[
        "--mode",
        "mongodb",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"NETWORK"}"#),
        "the component should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"E"}"#),
        "an error severity should be colored as a failure"
    );
    assert!(
        output.contains(r#"{"kind":"duration","text":"2345"}"#),
        "a duration should be colored as a duration"
    );
}

#[test]
fn test_mongodb_mode_reads_every_line_of_a_structured_log() {
    let example = example_path("mongodb_json.log");
    let output = run_splash_with_file("mongodb", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_mongodb_mode_reads_every_line_of_a_text_log() {
    let example = example_path("mongodb_text.log");
    let output = run_splash_with_file("mongodb", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_mongodb_mode_drops_a_line_that_is_not_a_mongodb_log() {
    let example = example_path("mongodb_mixed.log");
    let output = run_splash_with_file("mongodb", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_mongodb_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("mongodb_mixed.log");
    let contents = example_contents("mongodb_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "mongodb",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_mongodb_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("mongodb v1.0.0"),
        "--list-plugins should name the built-in mongodb plugin"
    );
}

// ==================== Elasticsearch Tests ====================

#[test]
fn test_elasticsearch_mode_colors_text_and_slow_logs() {
    let example = example_path("elasticsearch_server.log");
    let output = run_splash_file_args(&[
        "--mode",
        "elasticsearch",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"module","text":"o.e.n.Node"}"#),
        "the logger should be colored as a module"
    );
    assert!(
        output.contains(r#"{"kind":"warning","text":"WARN"}"#),
        "a warning should be colored as a warning"
    );
    assert!(
        output.contains(r#"{"kind":"duration","text":"2.3s"}"#),
        "a slow log time should be colored as a duration"
    );
}

#[test]
fn test_elasticsearch_mode_colors_json_logs() {
    let example = example_path("elasticsearch_json.log");
    let output = run_splash_file_args(&[
        "--mode",
        "elasticsearch",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r#"{"kind":"host","text":"node-1"}"#),
        "the node should be colored as a host"
    );
    assert!(
        output.contains(r#"{"kind":"failure","text":"ERROR"}"#),
        "an error should be colored as a failure"
    );
}

#[test]
fn test_elasticsearch_mode_reads_every_line_of_a_text_log() {
    let example = example_path("elasticsearch_server.log");
    let output = run_splash_with_file("elasticsearch", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_elasticsearch_mode_reads_every_line_of_a_json_log() {
    let example = example_path("elasticsearch_json.log");
    let output = run_splash_with_file("elasticsearch", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_elasticsearch_mode_drops_a_line_that_is_not_an_elasticsearch_log() {
    let example = example_path("elasticsearch_mixed.log");
    let output = run_splash_with_file("elasticsearch", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_elasticsearch_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("elasticsearch_mixed.log");
    let contents = example_contents("elasticsearch_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "elasticsearch",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_elasticsearch_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("elasticsearch v1.0.0"),
        "--list-plugins should name the built-in elasticsearch plugin"
    );
}

// ==================== SSH Tests ====================

#[test]
fn test_ssh_mode_colors_login_outcomes_methods_and_keys() {
    let example = example_path("ssh_sshd.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ssh",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"success","text":"Accepted"}"##),
        "an accepted login should be colored as a success"
    );
    assert!(
        output.contains(r##"{"kind":"protocol","text":"publickey"}"##),
        "the method should be colored as a protocol"
    );
    assert!(
        output.contains(r##"{"kind":"module","text":"ED25519"}"##),
        "the key type should be colored as a module"
    );
    assert!(
        output.contains(r##"{"kind":"tag","text":"[preauth]"}"##),
        "the preauth tag should be colored as a tag"
    );
}

#[test]
fn test_ssh_mode_reads_every_line_of_an_sshd_log() {
    let example = example_path("ssh_sshd.log");
    let output = run_splash_with_file("ssh", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_ssh_mode_drops_a_line_that_is_not_an_sshd_line() {
    let example = example_path("ssh_mixed.log");
    let output = run_splash_with_file("ssh", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_ssh_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("ssh_mixed.log");
    let contents = example_contents("ssh_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ssh",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_ssh_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("ssh v1.0.0"),
        "--list-plugins should name the built-in ssh plugin"
    );
}

// ==================== sudo Tests ====================

#[test]
fn test_sudo_mode_colors_commands_and_switches() {
    let example = example_path("sudo_syslog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "sudo",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"request","text":"/usr/bin/apt update"}"##),
        "the command should be colored as a request"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"command not allowed"}"##),
        "a refused command should be colored as a failure"
    );
    assert!(
        output.contains(r##"{"kind":"path","text":"/dev/pts/1"}"##),
        "the terminal of a switch should be colored as a path"
    );
}

#[test]
fn test_sudo_mode_reads_every_line_of_a_syslog_log() {
    let example = example_path("sudo_syslog.log");
    let output = run_splash_with_file("sudo", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_sudo_mode_reads_every_line_of_a_log_file() {
    let example = example_path("sudo_logfile.log");
    let output = run_splash_with_file("sudo", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
}

#[test]
fn test_sudo_mode_drops_a_line_that_is_not_a_sudo_or_su_line() {
    let example = example_path("sudo_mixed.log");
    let output = run_splash_with_file("sudo", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_sudo_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("sudo_mixed.log");
    let contents = example_contents("sudo_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "sudo",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_sudo_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("sudo v1.0.0"),
        "--list-plugins should name the built-in sudo plugin"
    );
}

// ==================== super Tests ====================

#[test]
fn test_super_mode_colors_the_user_and_command() {
    let example = example_path("super_logfile.log");
    let output = run_splash_file_args(&[
        "--mode",
        "super",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"userid","text":"alice"}"##),
        "the user should be colored as a user id"
    );
    assert!(
        output.contains(r##"{"kind":"method","text":"shutdown"}"##),
        "the command should be colored as a method"
    );
    assert!(
        output.contains(r##"{"kind":"request","text":"-h now"}"##),
        "the arguments should be colored as a request"
    );
}

#[test]
fn test_super_mode_reads_every_line_of_a_log_file() {
    let example = example_path("super_logfile.log");
    let output = run_splash_with_file("super", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_super_mode_drops_a_line_that_is_not_a_super_line() {
    let example = example_path("super_mixed.log");
    let output = run_splash_with_file("super", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_super_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("super_mixed.log");
    let contents = example_contents("super_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "super",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_super_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("super v1.0.0"),
        "--list-plugins should name the built-in super plugin"
    );
}

// ==================== sulog Tests ====================

#[test]
fn test_sulog_mode_colors_each_switch() {
    let example = example_path("sulog_solaris.log");
    let output = run_splash_file_args(&[
        "--mode",
        "sulog",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"success","text":"+"}"##),
        "a successful switch should be colored as a success"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"-"}"##),
        "a failed switch should be colored as a failure"
    );
    assert!(
        output.contains(r##"{"kind":"userid","text":"oracle"}"##),
        "the target user should be colored as a user id"
    );
}

#[test]
fn test_sulog_mode_reads_every_line_of_a_sulog() {
    let example = example_path("sulog_solaris.log");
    let output = run_splash_with_file("sulog", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_sulog_mode_drops_a_line_that_is_not_a_sulog_line() {
    let example = example_path("sulog_mixed.log");
    let output = run_splash_with_file("sulog", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_sulog_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("sulog_mixed.log");
    let contents = example_contents("sulog_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "sulog",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_sulog_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("sulog v1.0.0"),
        "--list-plugins should name the built-in sulog plugin"
    );
}

// ==================== distcc Tests ====================

#[test]
fn test_distcc_mode_colors_job_summaries() {
    let example = example_path("distcc_logfile.log");
    let output = run_splash_file_args(&[
        "--mode",
        "distcc",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"success","text":"COMPILE_OK"}"##),
        "a finished compile should be colored as a success"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"COMPILE_ERROR"}"##),
        "a failed compile should be colored as a failure"
    );
    assert!(
        output.contains(r##"{"kind":"module","text":"dcc_job_summary"}"##),
        "the function should be colored as a module"
    );
}

#[test]
fn test_distcc_mode_reads_every_line_of_a_log_file() {
    let example = example_path("distcc_logfile.log");
    let output = run_splash_with_file("distcc", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_distcc_mode_drops_a_line_that_is_not_a_distcc_line() {
    let example = example_path("distcc_mixed.log");
    let output = run_splash_with_file("distcc", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_distcc_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("distcc_mixed.log");
    let contents = example_contents("distcc_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "distcc",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_distcc_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("distcc v1.0.0"),
        "--list-plugins should name the built-in distcc plugin"
    );
}

// ==================== icecast Tests ====================

#[test]
fn test_icecast_mode_colors_error_log_levels_and_modules() {
    let example = example_path("icecast_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "icecast",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"failure","text":"EROR"}"##),
        "an error should be colored as a failure"
    );
    assert!(
        output.contains(r##"{"kind":"module","text":"main/main"}"##),
        "the module should be colored as a module"
    );
}

#[test]
fn test_icecast_mode_colors_icecast_1_usage() {
    let example = example_path("icecast_v1.log");
    let output = run_splash_file_args(&[
        "--mode",
        "icecast",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"header","text":"Clients"}"##),
        "a count name should be colored as a header"
    );
    assert!(
        output.contains(r##"{"kind":"number","text":"42"}"##),
        "a count should be colored as a number"
    );
}

#[test]
fn test_icecast_mode_reads_every_line_of_an_error_log() {
    let example = example_path("icecast_error.log");
    let output = run_splash_with_file("icecast", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_icecast_mode_reads_every_line_of_an_access_log() {
    let example = example_path("icecast_access.log");
    let output = run_splash_with_file("icecast", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
}

#[test]
fn test_icecast_mode_reads_every_line_of_an_icecast_1_log() {
    let example = example_path("icecast_v1.log");
    let output = run_splash_with_file("icecast", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_icecast_mode_drops_a_line_that_is_not_an_icecast_line() {
    let example = example_path("icecast_mixed.log");
    let output = run_splash_with_file("icecast", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_icecast_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("icecast_mixed.log");
    let contents = example_contents("icecast_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "icecast",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_icecast_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("icecast v1.0.0"),
        "--list-plugins should name the built-in icecast plugin"
    );
}

// ==================== APM Tests ====================

#[test]
fn test_apm_mode_colors_battery_state_and_times() {
    let example = example_path("apm_apmd.log");
    let output = run_splash_file_args(&[
        "--mode",
        "apm",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"size","text":"87%"}"##),
        "the charge should be colored as a size"
    );
    assert!(
        output.contains(r##"{"kind":"warning","text":"discharging"}"##),
        "a discharging battery should be colored as a warning"
    );
    assert!(
        output.contains(r##"{"kind":"duration","text":"1:23:45"}"##),
        "a time should be colored as a duration"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"BATTERY IS LOW"}"##),
        "a low battery should be colored as a failure"
    );
}

#[test]
fn test_apm_mode_reads_every_line_of_an_apmd_log() {
    let example = example_path("apm_apmd.log");
    let output = run_splash_with_file("apm", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 9);
}

#[test]
fn test_apm_mode_drops_a_line_that_is_not_an_apmd_line() {
    let example = example_path("apm_mixed.log");
    let output = run_splash_with_file("apm", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 1);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_apm_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("apm_mixed.log");
    let contents = example_contents("apm_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "apm",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_apm_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("apm v1.0.0"),
        "--list-plugins should name the built-in apm plugin"
    );
}

// ==================== Oops Tests ====================

#[test]
fn test_oops_mode_colors_the_parts_of_an_oops() {
    let example = example_path("oops_kernel.log");
    let output = run_splash_file_args(&[
        "--mode",
        "oops",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"failure","text":"Oops"}"##),
        "the oops headline should be colored as a failure"
    );
    assert!(
        output.contains(r##"{"kind":"module","text":"ext4_do_writepages+0x1a/0x40"}"##),
        "the faulting function should be colored as a module"
    );
    assert!(
        output.contains(r##"{"kind":"tag","text":"ext4"}"##),
        "the module should be colored as a tag"
    );
    assert!(
        output.contains(r##"{"kind":"header","text":"Call Trace"}"##),
        "the call trace heading should be colored as a header"
    );
}

#[test]
fn test_oops_mode_reads_every_line_of_an_oops() {
    let example = example_path("oops_kernel.log");
    let output = run_splash_with_file("oops", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 20);
}

#[test]
fn test_oops_mode_drops_a_line_that_is_not_part_of_an_oops() {
    let example = example_path("oops_mixed.log");
    let output = run_splash_with_file("oops", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_oops_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("oops_mixed.log");
    let contents = example_contents("oops_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "oops",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_oops_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("oops v1.0.0"),
        "--list-plugins should name the built-in oops plugin"
    );
}

// ==================== Docker Tests ====================

#[test]
fn test_docker_mode_colors_container_and_daemon_logs() {
    let example = example_path("docker_daemon.log");
    let output = run_splash_file_args(&[
        "--mode",
        "docker",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"warning","text":"warning"}"##),
        "a warning level should be colored as a warning"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"error"}"##),
        "an error level should be colored as a failure"
    );
    assert!(
        output.contains(r##"{"kind":"transaction","text":"4f2a1c0123ab"}"##),
        "a container id should be colored as a transaction"
    );
}

#[test]
fn test_docker_mode_colors_json_file_streams() {
    let example = example_path("docker_json.log");
    let output = run_splash_file_args(&[
        "--mode",
        "docker",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"warning","text":"stderr"}"##),
        "standard error should be colored as a warning"
    );
}

#[test]
fn test_docker_mode_reads_every_line_of_a_json_file_log() {
    let example = example_path("docker_json.log");
    let output = run_splash_with_file("docker", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_docker_mode_reads_every_line_of_a_daemon_log() {
    let example = example_path("docker_daemon.log");
    let output = run_splash_with_file("docker", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_docker_mode_reads_every_line_of_timestamped_output() {
    let example = example_path("docker_timestamps.log");
    let output = run_splash_with_file("docker", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
}

#[test]
fn test_docker_mode_drops_a_line_that_is_not_a_docker_line() {
    let example = example_path("docker_mixed.log");
    let output = run_splash_with_file("docker", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_docker_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("docker_mixed.log");
    let contents = example_contents("docker_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "docker",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_docker_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("docker v1.0.0"),
        "--list-plugins should name the built-in docker plugin"
    );
}

// ==================== Kubernetes Tests ====================

#[test]
fn test_kubernetes_mode_colors_klog_lines() {
    let example = example_path("kubernetes_klog.log");
    let output = run_splash_file_args(&[
        "--mode",
        "kubernetes",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"failure","text":"E"}"##),
        "an error severity should be colored as a failure"
    );
    assert!(
        output.contains(r##"{"kind":"path","text":"controller.go"}"##),
        "the file should be colored as a path"
    );
    assert!(
        output.contains(r##"{"kind":"module","text":"default/web-0"}"##),
        "a pod should be colored as a module"
    );
}

#[test]
fn test_kubernetes_mode_colors_cri_lines() {
    let example = example_path("kubernetes_cri.log");
    let output = run_splash_file_args(&[
        "--mode",
        "kubernetes",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"warning","text":"stderr"}"##),
        "standard error should be colored as a warning"
    );
    assert!(
        output.contains(r##"{"kind":"tag","text":"P"}"##),
        "a partial line tag should be colored as a tag"
    );
}

#[test]
fn test_kubernetes_mode_reads_every_line_of_a_klog_log() {
    let example = example_path("kubernetes_klog.log");
    let output = run_splash_with_file("kubernetes", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_kubernetes_mode_reads_every_line_of_a_cri_log() {
    let example = example_path("kubernetes_cri.log");
    let output = run_splash_with_file("kubernetes", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_kubernetes_mode_drops_a_line_that_is_not_a_kubernetes_line() {
    let example = example_path("kubernetes_mixed.log");
    let output = run_splash_with_file("kubernetes", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_kubernetes_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("kubernetes_mixed.log");
    let contents = example_contents("kubernetes_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "kubernetes",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_kubernetes_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("kubernetes v1.0.0"),
        "--list-plugins should name the built-in kubernetes plugin"
    );
}

// ==================== systemd-resolved Tests ====================

#[test]
fn test_resolved_mode_colors_feature_levels_and_servers() {
    let example = example_path("resolved_journal.log");
    let output = run_splash_file_args(&[
        "--mode",
        "systemd-resolved",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"warning","text":"Using degraded feature set"}"##),
        "a degraded feature set should be colored as a warning"
    );
    assert!(
        output.contains(r##"{"kind":"protocol","text":"UDP+EDNS0"}"##),
        "a feature level should be colored as a protocol"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"NXDOMAIN"}"##),
        "a failed lookup should be colored as a failure"
    );
}

#[test]
fn test_resolved_mode_reads_every_line_of_a_journal() {
    let example = example_path("resolved_journal.log");
    let output = run_splash_with_file("systemd-resolved", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 8);
}

#[test]
fn test_resolved_mode_drops_a_line_that_is_not_a_systemd_resolved_line() {
    let example = example_path("resolved_mixed.log");
    let output = run_splash_with_file("systemd-resolved", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 1);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_resolved_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("resolved_mixed.log");
    let contents = example_contents("resolved_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "systemd-resolved",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_resolved_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("systemd-resolved v1.0.0"),
        "--list-plugins should name the built-in systemd-resolved plugin"
    );
}

// ==================== nginx Error Tests ====================

#[test]
fn test_nginx_error_mode_colors_calls_files_and_context() {
    let example = example_path("nginx_error.log");
    let output = run_splash_file_args(&[
        "--mode",
        "nginx-error",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"module","text":"open()"}"##),
        "the system call should be colored as a module"
    );
    assert!(
        output.contains(r##"{"kind":"path","text":"/usr/share/nginx/html/favicon.ico"}"##),
        "the file should be colored as a path"
    );
    assert!(
        output.contains(r##"{"kind":"request","text":"GET /favicon.ico HTTP/1.1"}"##),
        "the request should be colored as a request"
    );
}

#[test]
fn test_nginx_error_mode_reads_every_line_of_an_error_log() {
    let example = example_path("nginx_error.log");
    let output = run_splash_with_file("nginx-error", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_nginx_error_mode_drops_a_line_that_is_not_an_nginx_error_line() {
    let example = example_path("nginx_error_mixed.log");
    let output = run_splash_with_file("nginx-error", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 1);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_nginx_error_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("nginx_error_mixed.log");
    let contents = example_contents("nginx_error_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "nginx-error",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_nginx_error_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("nginx-error v1.0.0"),
        "--list-plugins should name the built-in nginx-error plugin"
    );
}

// ==================== Git Tests ====================

#[test]
fn test_git_mode_colors_daemon_requests() {
    let example = example_path("git_daemon.log");
    let output = run_splash_file_args(&[
        "--mode",
        "git",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"method","text":"upload-pack"}"##),
        "the service should be colored as a method"
    );
    assert!(
        output.contains(r##"{"kind":"path","text":"/srv/git/app.git"}"##),
        "the repository should be colored as a path"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"repository not exported"}"##),
        "a refused repository should be colored as a failure"
    );
}

#[test]
fn test_git_mode_colors_trace_output() {
    let example = example_path("git_trace.log");
    let output = run_splash_file_args(&[
        "--mode",
        "git",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"module","text":"built-in"}"##),
        "what Git ran should be colored as a module"
    );
    assert!(
        output.contains(r##"{"kind":"request","text":"git fetch origin"}"##),
        "the command should be colored as a request"
    );
}

#[test]
fn test_git_mode_reads_every_line_of_a_daemon_log() {
    let example = example_path("git_daemon.log");
    let output = run_splash_with_file("git", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 7);
}

#[test]
fn test_git_mode_reads_every_line_of_trace_output() {
    let example = example_path("git_trace.log");
    let output = run_splash_with_file("git", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_git_mode_drops_a_line_that_is_not_a_git_line() {
    let example = example_path("git_mixed.log");
    let output = run_splash_with_file("git", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_git_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("git_mixed.log");
    let contents = example_contents("git_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "git",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_git_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("git v1.0.0"),
        "--list-plugins should name the built-in git plugin"
    );
}

// ==================== CI Tests ====================

#[test]
fn test_ci_mode_colors_jenkins_console_lines() {
    let example = example_path("ci_jenkins.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ci",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"tag","text":"Pipeline"}"##),
        "a pipeline step should be colored as a tag"
    );
    assert!(
        output.contains(r##"{"kind":"request","text":"make test"}"##),
        "a shell command should be colored as a request"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"FAILURE"}"##),
        "a failed build should be colored as a failure"
    );
}

#[test]
fn test_ci_mode_colors_github_actions_markers() {
    let example = example_path("ci_github.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ci",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"header","text":"group"}"##),
        "a group marker should be colored as a header"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"error"}"##),
        "an error marker should be colored as a failure"
    );
}

#[test]
fn test_ci_mode_keeps_every_line_of_a_jenkins_console_log() {
    let example = example_path("ci_jenkins.log");
    let output = run_splash_with_file("ci", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 10);
}

#[test]
fn test_ci_mode_reads_every_line_of_a_jenkins_server_log() {
    let example = example_path("ci_jenkins_server.log");
    let output = run_splash_with_file("ci", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_ci_mode_keeps_every_line_of_a_github_actions_log() {
    let example = example_path("ci_github.log");
    let output = run_splash_with_file("ci", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_ci_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("ci_jenkins.log");
    let contents = example_contents("ci_jenkins.log");
    let output = run_splash_file_args(&[
        "--mode",
        "ci",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_ci_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("ci v1.0.0"),
        "--list-plugins should name the built-in ci plugin"
    );
}

// ==================== cloud-init Tests ====================

#[test]
fn test_cloud_init_mode_colors_stages_and_results() {
    let example = example_path("cloud_init.log");
    let output = run_splash_file_args(&[
        "--mode",
        "cloud-init",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"module","text":"init-local"}"##),
        "the stage should be colored as a module"
    );
    assert!(
        output.contains(r##"{"kind":"success","text":"SUCCESS"}"##),
        "a successful module should be colored as a success"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"ERROR"}"##),
        "an error level should be colored as a failure"
    );
}

#[test]
fn test_cloud_init_mode_reads_every_line_of_a_log() {
    let example = example_path("cloud_init.log");
    let output = run_splash_with_file("cloud-init", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 6);
}

#[test]
fn test_cloud_init_mode_reads_every_line_of_an_output_log() {
    let example = example_path("cloud_init_output.log");
    let output = run_splash_with_file("cloud-init", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
}

#[test]
fn test_cloud_init_mode_drops_a_line_that_is_not_a_cloud_init_line() {
    let example = example_path("cloud_init_mixed.log");
    let output = run_splash_with_file("cloud-init", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_cloud_init_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("cloud_init_mixed.log");
    let contents = example_contents("cloud_init_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "cloud-init",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_cloud_init_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("cloud-init v1.0.0"),
        "--list-plugins should name the built-in cloud-init plugin"
    );
}

// ==================== Terraform Tests ====================

#[test]
fn test_terraform_mode_colors_operations_and_summaries() {
    let example = example_path("terraform_cli.log");
    let output = run_splash_file_args(&[
        "--mode",
        "terraform",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"success","text":"Creation complete"}"##),
        "a finished operation should be colored as a success"
    );
    assert!(
        output.contains(r##"{"kind":"duration","text":"32s"}"##),
        "how long an operation took should be colored as a duration"
    );
    assert!(
        output.contains(r##"{"kind":"failure","text":"Error"}"##),
        "an error should be colored as a failure"
    );
}

#[test]
fn test_terraform_mode_colors_logs_and_json() {
    let example = example_path("terraform_log.log");
    let output = run_splash_file_args(&[
        "--mode",
        "terraform",
        "--output",
        "json",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert!(
        output.contains(r##"{"kind":"module","text":"provider.terraform-provider-aws_v5.0.0"}"##),
        "the logger should be colored as a module"
    );
    assert!(
        output.contains(r##"{"kind":"tag","text":"apply_complete"}"##),
        "a JSON message type should be colored as a tag"
    );
}

#[test]
fn test_terraform_mode_reads_every_line_of_cli_output() {
    let example = example_path("terraform_cli.log");
    let output = run_splash_with_file("terraform", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 14);
}

#[test]
fn test_terraform_mode_reads_every_line_of_a_log() {
    let example = example_path("terraform_log.log");
    let output = run_splash_with_file("terraform", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_terraform_mode_drops_a_line_that_is_not_terraform_output() {
    let example = example_path("terraform_mixed.log");
    let output = run_splash_with_file("terraform", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 2);
    assert!(
        !output.contains("maintenance window"),
        "a line in none of the formats should be dropped"
    );
}

#[test]
fn test_terraform_mode_reproduces_each_line_in_plain_output() {
    let example = example_path("terraform_mixed.log");
    let contents = example_contents("terraform_mixed.log");
    let output = run_splash_file_args(&[
        "--mode",
        "terraform",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    for line in output.lines() {
        assert!(
            contents.contains(line),
            "plain output changed the line '{}'",
            line
        );
    }
}

#[test]
fn test_list_plugins_names_the_terraform_plugin() {
    let output = run_splash(&["--list-plugins"]).unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);

    assert!(
        listing.contains("terraform v1.0.0"),
        "--list-plugins should name the built-in terraform plugin"
    );
}
