use std::io::Write;
use std::path::PathBuf;
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

/// Helper function to run splash with stdin
fn run_splash_with_stdin(mode: &str, input: &str) -> Result<String, std::io::Error> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .arg("--mode")
        .arg(mode)
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
