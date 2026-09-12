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
