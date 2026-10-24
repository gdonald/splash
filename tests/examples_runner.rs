use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// How long a followed file may take to produce its first output, which is
/// slow when the binary runs under a coverage tracer
const FIRST_OUTPUT_DEADLINE: Duration = Duration::from_secs(60);

/// A followed file has finished its initial render once its output has been
/// idle this long
const OUTPUT_QUIET_PERIOD: Duration = Duration::from_millis(500);

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
fn run_splash_with_file(mode: &str, filepath: &str) -> Result<String, std::io::Error> {
    let output = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--mode", mode, "--path", filepath])
        .output()?;

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
fn run_splash_file_args(args: &[&str]) -> Result<String, std::io::Error> {
    let output = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()?;

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

    child.wait()?;

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

/// A real-world sample rendered in `args`, as each line's token kinds
fn sample_kinds(example: &str, args: &[&str]) -> Vec<Vec<String>> {
    let mut args = args.to_vec();
    args.extend(["--output", "json"]);

    json_kinds(&run_splash_with_stdin_args(&args, &example_contents(example)).unwrap())
}

/// A real-world sample rendered as plain text, which must hold every line as
/// the log wrote it
fn assert_every_line_kept(example: &str, args: &[&str]) {
    let mut args = args.to_vec();
    args.extend(["--output", "plain"]);

    let output = run_splash_with_stdin_args(&args, &example_contents(example)).unwrap();

    assert_eq!(output, example_contents(example), "{}", example);
}

#[test]
fn test_nginx_production_traffic_keeps_every_request() {
    assert_every_line_kept("nginx_production.log", &["--mode", "httpd"]);
}

#[test]
fn test_nginx_production_traffic_reads_ipv6_clients_and_malformed_requests() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "httpd",
            "--output",
            "csv",
            "--fields",
            "client,request,status",
        ],
        &example_contents("nginx_production.log"),
    )
    .unwrap();
    let rows: Vec<&str> = output.lines().collect();

    assert_eq!(rows[2], "2001:db8::1f,/static/app.3f9a2c.js,200");
    assert_eq!(rows[4], "198.51.100.23,/api/cart,499");
    assert_eq!(
        rows[9],
        r"45.33.32.156,\x16\x03\x01\x02\x00\x01\x00\x01\xFC\x03\x03,400"
    );
    assert_eq!(rows[10], "45.33.32.156,-,400");
}

#[test]
fn test_nginx_production_traffic_keeps_an_escaped_quote_in_the_user_agent() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "httpd",
            "--output",
            "jsonl",
            "--fields",
            "user_agent",
        ],
        &example_contents("nginx_production.log"),
    )
    .unwrap();

    assert_eq!(
        output.lines().nth(7).unwrap(),
        r#"{"user_agent":"Mozilla/5.0 \\\"quoted\\\" agent"}"#
    );
}

#[test]
fn test_nginx_production_traffic_statistics_count_every_status() {
    let output = run_splash_with_stdin_args(
        &["--mode", "httpd", "--stats", "--stats-format", "csv"],
        &example_contents("nginx_production.log"),
    )
    .unwrap();

    for row in [
        "lines,,10\n",
        "status,200,4\n",
        "status,400,2\n",
        "status,499,1\n",
        "status,504,1\n",
    ] {
        assert!(output.contains(row), "{}", row);
    }
}

#[test]
fn test_a_multi_service_syslog_colors_each_program_by_its_own_mode() {
    let kinds = sample_kinds("multi_service_syslog.log", &["--auto"]);

    assert_every_line_kept("multi_service_syslog.log", &["--auto"]);
    assert!(kinds[2].contains(&"success".to_string()));
    assert!(kinds[3].contains(&"failure".to_string()));
    assert!(kinds[7].contains(&"userid".to_string()));
    assert!(kinds[8].contains(&"ip".to_string()));
}

#[test]
fn test_docker_compose_stack_logs_color_each_service_in_its_own_format() {
    let kinds = sample_kinds("compose_stack.log", &["--mode", "docker"]);

    assert_every_line_kept("compose_stack.log", &["--mode", "docker"]);
    assert_eq!(kinds[0][..3], ["module", "punctuation", "timestamp"]);
    assert_eq!(kinds[3][..3], ["module", "punctuation", "client"]);
    assert!(kinds[4].contains(&"failure".to_string()));
    assert!(kinds[5].contains(&"status".to_string()));
    assert_eq!(kinds[6], ["module", "punctuation", "message"]);
}

#[test]
fn test_kubernetes_cluster_logs_color_pods_components_and_containers() {
    let kinds = sample_kinds("k8s_cluster.log", &["--mode", "k8s"]);

    assert_every_line_kept("k8s_cluster.log", &["--mode", "k8s"]);
    assert_eq!(kinds[0][..2], ["module", "client"]);
    assert_eq!(kinds[2][..2], ["module", "failure"]);
    assert_eq!(kinds[3][..3], ["timestamp", "host", "tag"]);
    assert_eq!(kinds[4][..2], ["timestamp", "warning"]);
    assert!(kinds[5].contains(&"failure".to_string()));
}

#[test]
fn test_mysql_slow_query_logs_color_each_entrys_times_and_statement() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "mysql",
            "--output",
            "jsonl",
            "--fields",
            "duration,request",
        ],
        &example_contents("mysql_slow_production.log"),
    )
    .unwrap();
    let lines: Vec<&str> = output.lines().collect();

    assert_every_line_kept("mysql_slow_production.log", &["--mode", "mysql"]);
    assert_eq!(lines[7], r#"{"duration":["12.003210","0.000201"]}"#);
    assert!(lines[9].starts_with(r#"{"request":"SELECT COUNT(*) FROM order_items"#));
}

#[test]
fn test_postgresql_slow_statement_logs_color_durations_and_failures() {
    let kinds = sample_kinds("postgresql_slow.log", &["--mode", "postgresql"]);
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "postgresql",
            "--output",
            "jsonl",
            "--fields",
            "duration",
        ],
        &example_contents("postgresql_slow.log"),
    )
    .unwrap();

    assert_every_line_kept("postgresql_slow.log", &["--mode", "postgresql"]);
    assert_eq!(
        output.lines().nth(1).unwrap(),
        r#"{"duration":"12003.210 ms"}"#
    );
    assert!(kinds[3].contains(&"failure".to_string()));
    assert!(kinds[4].contains(&"request".to_string()));
}

#[test]
fn test_application_error_logs_keep_stack_traces_as_written() {
    assert_every_line_kept("app_errors.log", &["--auto"]);
}

#[test]
fn test_application_error_logs_color_timestamps_and_levels() {
    let kinds = sample_kinds("app_errors.log", &["--auto"]);

    assert_eq!(kinds[0][..2], ["timestamp", "level"]);
    assert_eq!(kinds[1][..2], ["timestamp", "failure"]);
    assert_eq!(kinds[8][..2], ["timestamp", "warning"]);
}

#[test]
fn test_application_errors_keep_their_stack_traces_under_a_level_filter() {
    let output = run_splash_with_stdin_args(
        &["--auto", "--level", "error", "--output", "plain"],
        &example_contents("app_errors.log"),
    )
    .unwrap();
    let example = example_contents("app_errors.log");
    let lines: Vec<&str> = example.lines().collect();

    assert_eq!(
        output,
        format!("{}\n{}\n", lines[1..8].join("\n"), lines[9..13].join("\n"))
    );
}

#[test]
fn test_security_audit_logs_mark_failed_and_successful_logins() {
    let kinds = sample_kinds("auth_security.log", &["--mode", "auth"]);
    let failures: Vec<usize> = (0..kinds.len())
        .filter(|index| kinds[*index].contains(&"failure".to_string()))
        .collect();
    let successes: Vec<usize> = (0..kinds.len())
        .filter(|index| kinds[*index].contains(&"success".to_string()))
        .collect();

    assert_every_line_kept("auth_security.log", &["--mode", "auth"]);
    assert_eq!(failures, [0, 1, 2, 7, 8]);
    assert_eq!(successes, [4, 5]);
}

#[test]
fn test_security_audit_logs_count_the_attacking_address() {
    let output = run_splash_with_stdin_args(
        &["--mode", "auth", "--output", "csv", "--fields", "ip"],
        &example_contents("auth_security.log"),
    )
    .unwrap();

    assert_eq!(output.matches("203.0.113.50").count(), 4);
}

#[cfg(unix)]
#[test]
fn test_tail_follow_output_is_shown_as_it_arrives() {
    assert_eq!(
        followed_pipe_output("httpd", "nginx_production.log"),
        example_contents("nginx_production.log")
    );
}

#[cfg(unix)]
#[test]
fn test_a_followed_log_is_read_across_rotation() {
    let output = follow_file(
        &["--mode", "ad-hoc", "--output", "plain"],
        b"service started\n",
        Some(&|log: &Path| {
            append_to(log, "written before rotation\n");
            std::fs::rename(log, log.with_extension("log.1")).unwrap();
            std::fs::write(log, "written after rotation\n").unwrap();
        }),
    );

    assert_eq!(
        output,
        "service started\nwritten before rotation\nwritten after rotation\n"
    );
}

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
fn test_jsonl_output_mode_writes_one_object_of_fields_per_line() {
    let output = run_splash_with_stdin_args(
        &["--mode", "httpd", "--output", "jsonl"],
        &example_contents("stats_access.log"),
    )
    .unwrap();

    assert_eq!(output.lines().count(), 6);
    assert_eq!(
        output.lines().next().unwrap(),
        r#"{"text":"10.0.0.5 - alice [03/Oct/2023:12:00:01 +0000] \"GET /index.html HTTP/1.1\" 200 5120 \"-\" \"curl/8.4.0\"","client":"10.0.0.5","user_identifier":"-","userid":"alice","timestamp":"[03/Oct/2023:12:00:01 +0000]","method":"GET","request":"/index.html","protocol":"HTTP/1.1","status":"200","size":"5120","referer":"-","user_agent":"curl/8.4.0"}"#
    );
}

#[test]
fn test_csv_output_mode_writes_the_named_fields_under_a_header_row() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "httpd",
            "--output",
            "csv",
            "--fields",
            "client,status",
            "--fields",
            "request",
        ],
        &example_contents("stats_access.log"),
    )
    .unwrap();

    assert_eq!(
        output,
        "client,status,request\n10.0.0.5,200,/index.html\n10.0.0.5,200,/about.html\n10.0.0.7,302,/login\n10.0.0.9,404,/index.html\n10.0.0.5,500,/index.html\n10.0.0.7,503,/api/queue/91\n"
    );
}

#[test]
fn test_jsonl_output_mode_writes_the_named_fields() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "httpd",
            "--output",
            "jsonl",
            "--fields",
            "status,request",
        ],
        &example_contents("stats_access.log"),
    )
    .unwrap();

    assert_eq!(
        output.lines().next().unwrap(),
        r#"{"status":"200","request":"/index.html"}"#
    );
}

#[test]
fn test_jsonl_output_keeps_the_structure_of_json_log_lines() {
    let input = example_contents("caddy_access.log");

    let output = run_splash_with_stdin_args(
        &["--mode", "caddy", "--output", "jsonl", "--fields", "record"],
        &input,
    )
    .unwrap();

    let expected: String = input
        .lines()
        .map(|line| format!("{{\"record\":{}}}\n", line.trim()))
        .collect();

    assert_eq!(output, expected);
}

#[test]
fn test_an_unknown_field_is_reported() {
    let output = run_splash(&["--output", "csv", "--fields", "colour"]).unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: Unknown field 'colour' (expected text or a color key, such as status)\n"
    );
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

fn redis_fields(home: &Path, args: &[&str]) -> std::process::Output {
    let example = example_path("redis_server.log");
    let mut all_args = vec![
        "--output",
        "jsonl",
        "--fields",
        "pid,module",
        "--path",
        example.to_str().unwrap(),
    ];
    all_args.extend_from_slice(args);

    run_splash_with_home(home, &all_args, "").unwrap()
}

fn first_line(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn test_auto_mode_detects_a_plugin_that_is_not_disabled() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_fields(home.path(), &["--auto"]);

    assert_eq!(first_line(&output), r#"{"pid":"1234","module":"C"}"#);
}

#[test]
fn test_auto_mode_leaves_out_a_plugin_disabled_on_the_command_line() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_fields(home.path(), &["--auto", "--disable-plugin", "redis"]);

    assert_eq!(first_line(&output), "{}");
}

#[test]
fn test_auto_mode_leaves_out_a_plugin_the_config_turns_off() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::write(
        home.path().join(".splashrc"),
        "[plugins.redis]\nenabled = \"false\"\n",
    )
    .unwrap();

    let output = redis_fields(home.path(), &["--auto"]);

    assert_eq!(first_line(&output), "{}");
}

#[test]
fn test_the_mode_of_a_disabled_plugin_is_refused() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_fields(
        home.path(),
        &["--mode", "redis", "--disable-plugin", "redis"],
    );

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: Plugin 'redis' is disabled\n"
    );
}

#[test]
fn test_disabling_a_mode_alias_disables_its_mode() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_fields(
        home.path(),
        &["--mode", "kubernetes", "--disable-plugin", "k8s"],
    );

    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: Plugin 'kubernetes' is disabled\n"
    );
}

#[test]
fn test_disabling_a_plugin_that_is_not_loaded_fails() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_fields(home.path(), &["--disable-plugin", "absent"]);

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: Plugin 'absent' not found\n"
    );
}

#[test]
fn test_choosing_a_plugin_reads_the_input_in_its_mode() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_fields(home.path(), &["--plugin", "redis"]);

    assert_eq!(first_line(&output), r#"{"pid":"1234","module":"C"}"#);
}

#[test]
fn test_a_plugin_and_a_mode_cannot_both_be_chosen() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_fields(home.path(), &["--plugin", "redis", "--mode", "syslog"]);

    assert!(!output.status.success());
}

fn redis_with_colors(home: &Path, args: &[&str]) -> String {
    std::fs::write(home.join(".splashrc"), "[plugins.redis]\npid = \"green\"\n").unwrap();

    let example = example_path("redis_server.log");
    let output = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .args(["--path", example.to_str().unwrap()])
        .env("HOME", home)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap();

    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn test_a_plugin_mode_renders_with_the_colors_configured_for_the_plugin() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_with_colors(home.path(), &["--mode", "redis"]);

    assert!(
        output.starts_with("\u{1b}[32m1234\u{1b}[0m"),
        "{:?}",
        output
    );
}

#[test]
fn test_a_detected_plugin_renders_with_the_colors_configured_for_the_plugin() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_with_colors(home.path(), &["--auto"]);

    assert!(
        output.starts_with("\u{1b}[32m1234\u{1b}[0m"),
        "{:?}",
        output
    );
}

#[test]
fn test_html_output_styles_a_plugin_mode_with_the_colors_configured_for_it() {
    let home = tempfile::TempDir::new().unwrap();

    let output = redis_with_colors(home.path(), &["--mode", "redis", "--output", "html"]);

    assert!(
        output.contains(".splash-pid { color: #00aa00; }"),
        "{}",
        output
    );
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
fn test_docker_mode_keeps_a_line_a_container_printed() {
    let example = example_path("docker_mixed.log");
    let output = run_splash_with_file("docker", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
    assert!(output.contains("the maintenance window moves to 02:00"));
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
fn test_kubernetes_mode_keeps_a_line_a_container_printed() {
    let example = example_path("kubernetes_mixed.log");
    let output = run_splash_with_file("kubernetes", example.to_str().unwrap()).unwrap();

    assert_eq!(output.lines().count(), 4);
    assert!(output.contains("the maintenance window moves to 02:00"));
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

// ==================== Auto Detection Tests ====================

#[test]
fn test_auto_flag_renders_every_line_of_a_mixed_log_from_stdin() {
    let contents = example_contents("auto_mixed.log");
    let output = run_splash_with_stdin_args(&["--auto", "--output", "plain"], &contents).unwrap();

    let expected: Vec<&str> = contents.lines().filter(|line| !line.is_empty()).collect();

    assert_eq!(output.lines().collect::<Vec<&str>>(), expected);
}

#[test]
fn test_auto_mode_colors_postfix_queue_ids_in_a_mixed_log() {
    let contents = example_contents("auto_mixed.log");
    let output =
        run_splash_with_stdin_args(&["--mode", "auto", "--output", "json"], &contents).unwrap();

    assert!(
        output.contains(r#"{"kind":"queue_id","text":"4F2A1C0123"}"#),
        "auto mode should render the postfix lines with the postfix plugin: {}",
        output
    );
}

#[test]
fn test_auto_mode_renders_a_short_stdin_stream() {
    let output = run_splash_with_stdin_args(
        &["--auto", "--output", "plain"],
        "maintenance window moves to 02:00\n",
    )
    .unwrap();

    assert_eq!(output, "maintenance window moves to 02:00\n");
}

#[test]
fn test_auto_mode_streams_stdin_past_the_detection_sample() {
    let line =
        "127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] \"GET /apache_pb.gif HTTP/1.0\" 200 2326\n";
    let input = line.repeat(120);
    let output = run_splash_with_stdin_args(&["--auto", "--output", "plain"], &input).unwrap();

    assert_eq!(output, input);
}

#[test]
fn test_auto_flag_renders_a_log_file() {
    let example = example_path("auto_mixed.log");
    let output = run_splash_file_args(&[
        "--auto",
        "--output",
        "plain",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output.lines().count(), 5);
}

#[test]
fn test_auto_flag_cannot_be_combined_with_a_mode() {
    let output = run_splash(&["--auto", "--mode", "syslog"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}

#[test]
fn test_a_config_file_can_turn_on_auto_detection() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".splash")).unwrap();
    std::fs::write(
        home.path().join(".splash/config.toml"),
        "mode = \"auto\"\noutput = \"json\"\n",
    )
    .unwrap();

    let output =
        run_splash_with_home(home.path(), &[], &example_contents("auto_mixed.log")).unwrap();

    assert!(String::from_utf8_lossy(&output.stdout)
        .contains(r#"{"kind":"queue_id","text":"4F2A1C0123"}"#));
}

// ==================== Filtering Tests ====================

/// Runs splash in auto mode over the filter example with extra arguments,
/// collecting plain output
fn filter_example(args: &[&str]) -> String {
    let mut all = vec!["--auto", "--output", "plain"];
    all.extend_from_slice(args);

    run_splash_with_stdin_args(&all, &example_contents("filter_app.log")).unwrap()
}

#[test]
fn test_level_filter_keeps_an_error_entry_and_its_stack_trace() {
    let output = filter_example(&["--level", "ERROR"]);

    assert_eq!(output.lines().count(), 3);
    assert!(output.contains("uncaught exception"));
    assert!(output.contains("NodeEnvironment.java:298"));
}

#[test]
fn test_level_filter_reads_a_comma_separated_list() {
    let output = filter_example(&["--level", "warn,error"]);

    assert_eq!(output.lines().count(), 4);
    assert!(!output.contains("starting"));
}

#[test]
fn test_since_and_until_keep_the_entries_written_between_them() {
    let output = filter_example(&[
        "--since",
        "2023-10-03 12:00:03",
        "--until",
        "2023-10-03 12:00:04",
    ]);

    assert_eq!(output.lines().count(), 4);
    assert!(output.contains("high disk watermark"));
    assert!(!output.contains("stopping"));
}

#[test]
fn test_since_reads_an_age() {
    let output = filter_example(&["--since", "2h ago"]);

    assert_eq!(output, "");
}

#[test]
fn test_grep_keeps_only_matching_lines() {
    let output = filter_example(&["--grep", "node locks"]);

    assert_eq!(
        output,
        "java.lang.IllegalStateException: failed to obtain node locks\n"
    );
}

#[test]
fn test_grep_highlights_capture_groups() {
    let output = run_splash_with_stdin_args(
        &["--auto", "--output", "json", "--grep", r"thread \[(\w+)\]"],
        &example_contents("filter_app.log"),
    )
    .unwrap();

    assert!(output.contains(r#"{"kind":"capture","text":"main"}"#));
    assert!(output.contains(r#"{"kind":"match","text":"thread "}"#));
}

#[test]
fn test_exclude_drops_matching_lines() {
    let output = filter_example(&["--exclude", "INFO", "--exclude", r"^\s+at "]);

    assert_eq!(output.lines().count(), 3);
    assert!(!output.contains("starting"));
}

#[test]
fn test_field_filter_keeps_lines_holding_the_field() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "clf",
            "--output",
            "plain",
            "--filter",
            "ip=10.0.0.50",
        ],
        &example_contents("clf_multiple.log"),
    )
    .unwrap();

    assert_eq!(
        output,
        "10.0.0.50 - bob [15/Jan/2024:10:31:12 +0000] \"POST /api/login HTTP/1.1\" 200 512\n"
    );
}

#[test]
fn test_filters_apply_to_a_log_file() {
    let example = example_path("filter_app.log");
    let output = run_splash_file_args(&[
        "--auto",
        "--output",
        "plain",
        "--level",
        "error",
        "--path",
        example.to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_an_invalid_filter_is_reported() {
    for args in [
        ["--level", "loud"],
        ["--since", "soon"],
        ["--grep", "("],
        ["--filter", "colour=red"],
    ] {
        let output = run_splash(&args).unwrap();

        assert!(!output.status.success(), "{:?} should fail", args);
        assert!(
            String::from_utf8_lossy(&output.stderr).starts_with("Error: "),
            "{:?}",
            args
        );
    }
}

// ==================== Timestamp Tests ====================

#[test]
fn test_convert_date_rewrites_epoch_timestamps() {
    let output = run_splash_with_stdin_args(
        &[
            "--auto",
            "--output",
            "plain",
            "--convert-date",
            "--tz",
            "UTC",
        ],
        &example_contents("dates_mixed.log"),
    )
    .unwrap();

    assert!(output.starts_with("2021-03-03T00:00:00.123Z    123 10.0.0.5"));
}

#[test]
fn test_tz_rewrites_every_timestamp_in_that_zone() {
    let output = run_splash_with_stdin_args(
        &["--auto", "--output", "plain", "--tz", "UTC"],
        &example_contents("dates_mixed.log"),
    )
    .unwrap();

    assert!(output.contains("frank [2000-10-10T20:55:36Z] \"GET"));
}

#[test]
fn test_normalize_dates_rewrites_every_timestamp() {
    let output = run_splash_with_stdin_args(
        &["--auto", "--output", "plain", "--normalize-dates"],
        &example_contents("dates_mixed.log"),
    )
    .unwrap();

    assert!(!output.contains("10/Oct/2000"));
    assert!(!output.contains("1614729600.123"));
}

#[test]
fn test_relative_rewrites_every_timestamp_as_an_age() {
    let output = run_splash_with_stdin_args(
        &["--auto", "--output", "plain", "--relative"],
        &example_contents("dates_mixed.log"),
    )
    .unwrap();

    assert!(output.lines().all(|line| line.contains("years ago")));
}

#[test]
fn test_an_unknown_time_zone_is_reported() {
    let output = run_splash(&["--tz", "Mars/Base"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown time zone 'Mars/Base'"));
}

// ==================== Keyword Tests ====================

#[test]
fn test_keywords_are_styled_in_ad_hoc_text() {
    let output = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "json"],
        &example_contents("keywords.log"),
    )
    .unwrap();

    for expected in [
        r#"{"kind":"failure","text":"ERROR"}"#,
        r#"{"kind":"protocol","text":"tcp"}"#,
        r#"{"kind":"success","text":"OK"}"#,
        r#"{"kind":"keyword","text":"restart"}"#,
    ] {
        assert!(output.contains(expected), "missing {}", expected);
    }
}

#[test]
fn test_no_wordcolor_leaves_keywords_unstyled() {
    let output = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "json", "--no-wordcolor"],
        &example_contents("keywords.log"),
    )
    .unwrap();

    assert!(!output.contains(r#""kind":"failure""#));
}

#[test]
fn test_a_config_file_adds_keywords_and_patterns() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".splash")).unwrap();
    std::fs::write(
        home.path().join(".splash/config.toml"),
        "output = \"json\"\n\n[keywords]\nfailure = \"timed out\"\n\n[patterns]\ntransaction = \"req-[0-9a-f]{8}\"\n",
    )
    .unwrap();

    let output = run_splash_with_home(home.path(), &[], &example_contents("keywords.log")).unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains(
        r#"{"kind":"failure","text":"timed"},{"kind":"failure","text":" "},{"kind":"failure","text":"out"}"#
    ));
    assert!(stdout.contains(r#"{"kind":"transaction","text":"req-4f2a1c01"}"#));
}

// ==================== Lookup Tests ====================

/// A home directory whose config file points every lookup at the example
/// account, service, and network files
fn lookups_home(enabled: bool) -> tempfile::TempDir {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".splash")).unwrap();
    std::fs::write(
        home.path().join(".splash/config.toml"),
        format!(
            "mode = \"auth\"\noutput = \"json\"\n\n[lookups]\nenabled = {}\npasswd = \"{}\"\nservices = \"{}\"\ngeoip = \"{}\"\n",
            enabled,
            example_path("lookups_passwd.txt").display(),
            example_path("lookups_services.txt").display(),
            example_path("lookups_networks.csv").display(),
        ),
    )
    .unwrap();

    home
}

fn lookups_example(enabled: bool, args: &[&str]) -> String {
    let home = lookups_home(enabled);
    let output = run_splash_with_home(home.path(), args, &example_contents("lookups.log")).unwrap();

    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_lookups_mark_unknown_users_and_name_services_and_locations() {
    let output = lookups_example(false, &["--lookups"]);

    for expected in [
        r#"{"kind":"userid","text":"alice"}"#,
        r#"{"kind":"unknown_user","text":"admin"}"#,
        r#"{"kind":"number","text":"22"},{"kind":"service","text":"(ssh)"}"#,
        r#"{"kind":"ip","text":"203.0.113.7"},{"kind":"location","text":" (US, Boston)"}"#,
    ] {
        assert!(output.contains(expected), "missing {}", expected);
    }
}

#[test]
fn test_a_config_file_can_turn_lookups_on() {
    let output = lookups_example(true, &[]);

    assert!(output.contains(r#"{"kind":"unknown_user","text":"admin"}"#));
}

#[test]
fn test_no_lookups_wins_over_the_config_file() {
    let output = lookups_example(true, &["--no-lookups"]);

    assert!(output.contains(r#"{"kind":"userid","text":"admin"}"#));
    assert!(!output.contains(r#""kind":"service""#));
}

#[test]
fn test_lookups_and_no_lookups_cannot_be_combined() {
    let output = run_splash(&["--lookups", "--no-lookups"]).unwrap();

    assert!(!output.status.success());
}

// ==================== Statistics Tests ====================

fn stats_example(args: &[&str]) -> String {
    let mut args = args.to_vec();
    args.extend(["--mode", "httpd", "--tz", "UTC"]);

    run_splash_with_stdin_args(&args, &example_contents("stats_access.log")).unwrap()
}

#[cfg(unix)]
fn append_to(log: &Path, text: &str) {
    std::fs::OpenOptions::new()
        .append(true)
        .open(log)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

/// Follows a copy of the example access log, appends `appended` to it once
/// the first output has arrived, and stops splash with SIGINT once the
/// output for the appended lines has arrived, returning everything written
#[cfg(unix)]
fn follow_stats_example(args: &[&str], appended: &str) -> String {
    let mut args = args.to_vec();
    args.extend(["--mode", "httpd", "--tz", "UTC"]);

    let append = |log: &Path| append_to(log, appended);

    follow_file(
        &args,
        example_contents("stats_access.log").as_bytes(),
        (!appended.is_empty()).then_some(&append as &dyn Fn(&Path)),
    )
}

/// Follows a file holding `initial`, makes `change` to it once the first
/// output has arrived, and stops splash with SIGINT once the output for the
/// change has arrived, returning everything written
#[cfg(unix)]
fn follow_file(args: &[&str], initial: &[u8], change: Option<&dyn Fn(&Path)>) -> String {
    follow_file_with_errors(args, initial, change).0
}

/// Follows a file as `follow_file` does, returning what splash wrote to
/// stdout and to stderr
#[cfg(unix)]
fn follow_file_with_errors(
    args: &[&str],
    initial: &[u8],
    change: Option<&dyn Fn(&Path)>,
) -> (String, String) {
    let directory = tempfile::TempDir::new().unwrap();
    let log = directory.path().join("followed.log");
    let errors = directory.path().join("errors.txt");
    std::fs::write(&log, initial).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .args(["--follow", "--path"])
        .arg(&log)
        .stdout(std::process::Stdio::piped())
        .stderr(std::fs::File::create(&errors).unwrap())
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        while let Ok(count) = stdout.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let collect = |output: &mut Vec<u8>| {
        if let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
            output.extend(chunk);

            while let Ok(chunk) = receiver.recv_timeout(OUTPUT_QUIET_PERIOD) {
                output.extend(chunk);
            }
        }
    };

    let mut output = Vec::new();
    collect(&mut output);

    if let Some(change) = change {
        change(&log);
        collect(&mut output);
    }

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }

    assert!(child.wait().unwrap().success());

    while let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
        output.extend(chunk);
    }

    (
        String::from_utf8_lossy(&output).to_string(),
        std::fs::read_to_string(&errors).unwrap(),
    )
}

#[test]
fn test_stats_write_a_report_in_place_of_the_lines() {
    let output = stats_example(&["--stats"]);

    assert!(output.starts_with("Statistics\n==========\nLines:         6\nErrors:        2\n"));
    assert!(output.contains("\nTop URLs:\n  /index.html    3\n"));
    assert!(!output.contains("HTTP/1.1"));
}

#[test]
fn test_stats_are_written_as_json() {
    let output = stats_example(&["--stats", "--stats-format", "json"]);

    assert!(output.starts_with(r#"{"lines":6,"errors":2,"error_percent":33.3,"requests":6,"#));
}

#[test]
fn test_summary_follows_the_lines() {
    let output = stats_example(&["--summary", "--output", "plain"]);

    assert!(output.starts_with("10.0.0.5 - alice [2023-10-03T12:00:01Z]"));
    assert!(output.ends_with(
        "Summary\n=======\nLines:       6\nErrors:      2\nError rate:  33.3%\nFrom:        2023-10-03T12:00:01Z\nTo:          2023-10-03T14:10:01Z\n"
    ));
}

#[test]
fn test_histogram_counts_lines_per_hour_by_default() {
    let output = stats_example(&["--histogram"]);

    assert!(output.starts_with("Lines per 1h\n"));
    assert!(output.contains("2023-10-03T13:00:00Z"));
}

#[test]
fn test_histogram_takes_a_period_and_a_format() {
    let output = stats_example(&["--histogram", "1d", "--stats-format", "csv"]);

    assert_eq!(output, "start,count\n2023-10-03T00:00:00Z,6\n");
}

#[test]
fn test_stats_and_histogram_cannot_be_combined() {
    let output = run_splash(&["--stats", "--histogram", "1h"]).unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_an_unknown_histogram_period_is_reported() {
    let output = run_splash(&["--histogram", "1w"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid histogram period '1w'"));
}

#[test]
fn test_stats_cannot_be_shown_in_the_curses_viewer() {
    let output = run_splash(&["--stats", "--output", "curses"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("The curses viewer cannot show --stats, --summary, or --histogram"));
}

#[cfg(unix)]
#[test]
fn test_stats_are_written_again_as_a_followed_file_grows() {
    let output = follow_stats_example(
        &["--stats", "--stats-format", "csv"],
        "10.0.0.9 - - [03/Oct/2023:14:20:00 +0000] \"GET /index.html HTTP/1.1\" 200 5120 \"-\" \"curl/8.4.0\"\n",
    );

    let line_counts: Vec<&str> = output
        .lines()
        .filter(|line| line.starts_with("lines,,"))
        .collect();

    assert_eq!(line_counts, vec!["lines,,6", "lines,,7"]);
}

#[cfg(unix)]
#[test]
fn test_summary_is_written_when_following_a_file_is_interrupted() {
    let output = follow_stats_example(&["--summary", "--output", "plain"], "");

    assert!(output.contains("/api/queue/91"));
    assert!(output.ends_with("To:          2023-10-03T14:10:01Z\n"));
}

#[cfg(unix)]
#[test]
fn test_html_output_is_closed_when_following_a_file_is_interrupted() {
    let output = follow_stats_example(&["--output", "html"], "");

    assert!(output.ends_with("</html>\n"));
}

// ==================== Follow Mode Tests ====================

#[test]
fn test_a_file_is_rendered_once_without_follow() {
    let output = run_splash(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--path",
        example_path("adhoc_ips.log").to_str().unwrap(),
    ])
    .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        example_contents("adhoc_ips.log")
    );
}

#[test]
fn test_follow_needs_a_path() {
    let output = run_splash(&["--follow"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--path <PATH>"));
}

#[cfg(unix)]
#[test]
fn test_a_followed_file_writes_the_lines_it_gains() {
    let output = follow_file(
        &["--mode", "ad-hoc", "--output", "plain"],
        b"service started\n",
        Some(&|log: &Path| append_to(log, "request served\n")),
    );

    assert_eq!(output, "service started\nrequest served\n");
}

#[cfg(unix)]
#[test]
fn test_a_followed_line_is_written_once_it_is_complete() {
    let output = follow_file(
        &["--mode", "ad-hoc", "--output", "plain"],
        b"service started\nrequest",
        Some(&|log: &Path| append_to(log, " served\n")),
    );

    assert_eq!(output, "service started\nrequest served\n");
}

#[cfg(unix)]
#[test]
fn test_an_unfinished_followed_line_is_written_when_splash_is_interrupted() {
    let output = follow_file(
        &["--mode", "ad-hoc", "--output", "plain"],
        b"service started\nrequest",
        None,
    );

    assert_eq!(output, "service started\nrequest\n");
}

#[cfg(unix)]
#[test]
fn test_a_truncated_followed_file_is_read_again_from_its_beginning() {
    let output = follow_file(
        &["--mode", "ad-hoc", "--output", "plain"],
        b"service started\nrequest served\n",
        Some(&|log: &Path| std::fs::write(log, "restarted\n").unwrap()),
    );

    assert_eq!(output, "service started\nrequest served\nrestarted\n");
}

#[cfg(unix)]
#[test]
fn test_invalid_bytes_a_followed_file_gains_are_replaced() {
    let output = follow_file(
        &["--mode", "ad-hoc", "--output", "plain"],
        b"service started\n",
        Some(&|log: &Path| {
            std::fs::OpenOptions::new()
                .append(true)
                .open(log)
                .unwrap()
                .write_all(b"request \xff served\n")
                .unwrap()
        }),
    );

    assert_eq!(output, "service started\nrequest \u{fffd} served\n");
}

// ==================== Network Listening Tests ====================

/// Splash listening on a port the system chose
#[cfg(unix)]
struct Listening {
    child: std::process::Child,
    address: String,
    output: mpsc::Receiver<Vec<u8>>,
    written: Vec<u8>,
}

#[cfg(unix)]
impl Listening {
    fn start(args: &[&str]) -> Listening {
        Listening::receive(args, "--listen", "Listening on ")
    }

    /// Starts splash with `source` naming port 0, reading the address it
    /// chose from the stderr line that opens with `announcement`
    fn receive(args: &[&str], source: &str, announcement_start: &str) -> Listening {
        use std::io::BufRead;

        let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
            .args(args)
            .args([source, "127.0.0.1:0"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let mut announcement = String::new();
        std::io::BufReader::new(child.stderr.take().unwrap())
            .read_line(&mut announcement)
            .unwrap();

        let address = announcement
            .trim_end()
            .strip_prefix(announcement_start)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .to_string();

        let mut stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();

        thread::spawn(move || {
            let mut buffer = [0u8; 8192];

            while let Ok(count) = stdout.read(&mut buffer) {
                if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                    break;
                }
            }
        });

        Listening {
            child,
            address,
            output,
            written: Vec::new(),
        }
    }

    /// Waits for the output that what was just sent produces
    fn collect(&mut self) {
        if let Ok(chunk) = self.output.recv_timeout(FIRST_OUTPUT_DEADLINE) {
            self.written.extend(chunk);

            while let Ok(chunk) = self.output.recv_timeout(OUTPUT_QUIET_PERIOD) {
                self.written.extend(chunk);
            }
        }
    }

    fn send_raw(&mut self, text: &str) {
        let mut connection = std::net::TcpStream::connect(&self.address).unwrap();
        connection.write_all(text.as_bytes()).unwrap();
        drop(connection);
        self.collect();
    }

    fn send_datagram(&mut self, text: &str) {
        std::net::UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .send_to(text.as_bytes(), &self.address)
            .unwrap();
        self.collect();
    }

    /// Posts `body` and returns the response
    fn post(&mut self, body: &str) -> String {
        let mut connection = std::net::TcpStream::connect(&self.address).unwrap();
        write!(
            connection,
            "POST /logs HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\n\r\n{}",
            self.address,
            body.len(),
            body
        )
        .unwrap();

        let mut response = String::new();
        connection.read_to_string(&mut response).unwrap();
        self.collect();

        response
    }

    /// Stops splash with SIGINT, returning whether it succeeded and
    /// everything it wrote
    fn interrupt(mut self) -> (bool, String) {
        unsafe {
            libc::kill(self.child.id() as libc::pid_t, libc::SIGINT);
        }

        let success = self.child.wait().unwrap().success();

        while let Ok(chunk) = self.output.recv_timeout(FIRST_OUTPUT_DEADLINE) {
            self.written.extend(chunk);
        }

        (success, String::from_utf8_lossy(&self.written).to_string())
    }
}

#[cfg(unix)]
#[test]
fn test_lines_sent_over_tcp_and_http_are_rendered() {
    let mut listening = Listening::start(&["--mode", "ad-hoc", "--output", "plain"]);

    listening.send_raw("service started\r\nworker ready\n");
    let response = listening.post("request served\n");
    let (success, output) = listening.interrupt();

    assert!(response.starts_with("HTTP/1.1 204 No Content\r\n"));
    assert!(success);
    assert_eq!(output, "service started\nworker ready\nrequest served\n");
}

#[cfg(unix)]
#[test]
fn test_summary_is_written_when_listening_is_interrupted() {
    let mut listening = Listening::start(&[
        "--mode",
        "httpd",
        "--tz",
        "UTC",
        "--output",
        "plain",
        "--summary",
    ]);

    listening.send_raw(&example_contents("stats_access.log"));
    let (success, output) = listening.interrupt();

    assert!(success);
    assert!(output.contains("/api/queue/91"));
    assert!(output.ends_with("To:          2023-10-03T14:10:01Z\n"));
}

#[cfg(unix)]
#[test]
fn test_auto_mode_detects_the_format_of_lines_received() {
    let mut listening = Listening::start(&["--auto", "--output", "json"]);

    listening.send_raw(&example_contents("stats_access.log"));
    let (_, output) = listening.interrupt();

    assert!(output.contains(r#"{"kind":"userid","text":"alice"}"#));
}

#[test]
fn test_an_address_that_cannot_be_bound_is_reported() {
    let output = run_splash(&["--listen", "not an address"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .starts_with("Error: Cannot listen on not an address: "));
}

#[test]
fn test_listen_cannot_be_combined_with_a_path() {
    let output = run_splash(&["--listen", ":0", "--path", "access.log"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}

// ==================== WebSocket Output Tests ====================

/// Reads one short text message from a WebSocket connection
fn read_websocket_message(connection: &mut std::net::TcpStream) -> String {
    let mut header = [0u8; 2];
    connection.read_exact(&mut header).unwrap();
    assert_eq!(header[0], 0x81);

    let mut payload = vec![0u8; (header[1] & 0x7f) as usize];
    connection.read_exact(&mut payload).unwrap();

    String::from_utf8(payload).unwrap()
}

#[test]
fn test_output_is_sent_to_websocket_clients() {
    use std::io::BufRead;

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--mode", "ad-hoc", "--output", "plain"])
        .args(["--websocket", "127.0.0.1:0"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let mut announcement = String::new();
    std::io::BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut announcement)
        .unwrap();
    let address = announcement
        .trim_end()
        .strip_prefix("Sending WebSocket output on ")
        .unwrap()
        .to_string();

    let mut browser = std::net::TcpStream::connect(address).unwrap();
    browser
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n")
        .unwrap();

    let mut response = Vec::new();
    while !response.ends_with(b"\r\n\r\n") {
        let mut byte = [0u8; 1];
        browser.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
    }

    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(b"service started\nrequest served\n")
        .unwrap();
    let messages = [
        read_websocket_message(&mut browser),
        read_websocket_message(&mut browser),
    ];
    drop(stdin);
    let output = child.wait_with_output().unwrap();

    assert!(String::from_utf8_lossy(&response).starts_with("HTTP/1.1 101 Switching Protocols\r\n"));
    assert_eq!(messages, ["service started", "request served"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "service started\nrequest served\n"
    );
}

#[test]
fn test_websocket_output_cannot_be_combined_with_the_curses_viewer() {
    let output = run_splash(&["--websocket", ":0", "--output", "curses"]).unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: --websocket sends the output splash writes, and the curses viewer writes none\n"
    );
}

#[test]
fn test_a_websocket_address_that_cannot_be_bound_is_reported() {
    let output = run_splash(&["--websocket", "not an address"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .starts_with("Error: Cannot listen on not an address: "));
}

// ==================== Syslog Receiver Tests ====================

const SYSLOG_UDP: &str =
    "<38>Oct  3 12:00:01 web01 sshd[4101]: Accepted publickey for alice from 10.0.0.5 port 52144 ssh2";

const SYSLOG_TCP: &str = "<165>1 2023-10-03T12:00:02.003Z web01 app 4102 ID47 - backup finished";

#[cfg(unix)]
#[test]
fn test_syslog_messages_over_udp_and_tcp_are_rendered_in_syslog_mode() {
    let mut receiving = Listening::receive(
        &["--output", "json"],
        "--syslog",
        "Receiving syslog messages on ",
    );

    receiving.send_datagram(SYSLOG_UDP);
    receiving.send_raw(&format!("{} {}", SYSLOG_TCP.len(), SYSLOG_TCP));
    let (success, output) = receiving.interrupt();

    let expected = run_splash_with_stdin_args(
        &["--mode", "syslog", "--output", "json"],
        &format!("{}\n{}\n", SYSLOG_UDP, SYSLOG_TCP),
    )
    .unwrap();

    assert!(success);
    assert_eq!(output, expected);
}

#[cfg(unix)]
#[test]
fn test_a_mode_given_with_syslog_is_used_in_place_of_syslog_mode() {
    let mut receiving = Listening::receive(
        &["--mode", "ad-hoc", "--output", "json"],
        "--syslog",
        "Receiving syslog messages on ",
    );

    receiving.send_datagram(SYSLOG_UDP);
    let (_, output) = receiving.interrupt();

    let expected =
        run_splash_with_stdin_args(&["--mode", "ad-hoc", "--output", "json"], SYSLOG_UDP).unwrap();

    assert_eq!(output, expected);
}

#[test]
fn test_syslog_cannot_be_combined_with_listen() {
    let output = run_splash(&["--syslog", ":0", "--listen", ":0"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}

#[test]
fn test_a_syslog_address_that_cannot_be_bound_is_reported() {
    let output = run_splash(&["--syslog", "not an address"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .starts_with("Error: Cannot listen on not an address: "));
}

// ==================== Anonymization Tests ====================

#[test]
fn test_anonymized_output_holds_no_address_or_user_name() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "httpd",
            "--output",
            "csv",
            "--fields",
            "client,userid,text",
            "--anonymize",
        ],
        &example_contents("stats_access.log"),
    )
    .unwrap();

    let rows: Vec<Vec<&str>> = output
        .lines()
        .skip(1)
        .map(|row| row.splitn(3, ',').collect())
        .collect();

    assert_eq!(rows.len(), 6);
    assert!(!output.contains("10.0.0."));
    assert!(!output.contains("alice"));
    assert_eq!(rows[0][0], rows[1][0]);
    assert_ne!(rows[0][0], rows[2][0]);
    assert_eq!(rows[2][1], "-");
    assert!(rows[0][2].starts_with(&format!("\"{} - user-", rows[0][0])));
}

#[test]
fn test_statistics_on_anonymized_lines_hold_no_address() {
    let output = run_splash_with_stdin_args(
        &["--mode", "httpd", "--stats", "--anonymize"],
        &example_contents("stats_access.log"),
    )
    .unwrap();

    assert!(output.contains("Top addresses:\n  ip-"));
    assert!(!output.contains("10.0.0."));
}

// ==================== Multiple File Tests ====================

/// A directory holding log files with the given names and contents
fn log_directory(files: &[(&str, &str)]) -> tempfile::TempDir {
    let directory = tempfile::TempDir::new().unwrap();

    for (name, contents) in files {
        std::fs::write(directory.path().join(name), contents).unwrap();
    }

    directory
}

fn header(path: &Path) -> String {
    format!("==> {} <==\n", path.display())
}

#[test]
fn test_several_files_are_rendered_in_turn_under_headers() {
    let directory = log_directory(&[("a.log", "alpha\n"), ("b.log", "beta\n")]);
    let first = directory.path().join("a.log");
    let second = directory.path().join("b.log");

    let output = run_splash_file_args(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--path",
        first.to_str().unwrap(),
        second.to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(
        output,
        format!("{}alpha\n{}beta\n", header(&first), header(&second))
    );
}

#[test]
fn test_a_path_pattern_renders_every_matching_file() {
    let directory = log_directory(&[
        ("web.log", "web line\n"),
        ("app.log", "app line\n"),
        ("notes.txt", "not a log\n"),
    ]);
    let pattern = directory.path().join("*.log");

    let output = run_splash_file_args(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--path",
        pattern.to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(
        output,
        format!(
            "{}app line\n{}web line\n",
            header(&directory.path().join("app.log")),
            header(&directory.path().join("web.log"))
        )
    );
}

#[test]
fn test_a_path_pattern_matching_nothing_is_reported() {
    let directory = log_directory(&[]);
    let pattern = directory.path().join("*.log");

    let output = run_splash(&["--path", pattern.to_str().unwrap()]).unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!("Error: No files match '{}'\n", pattern.display())
    );
}

#[test]
fn test_each_file_is_detected_in_its_own_format() {
    let access = example_path("stats_access.log");
    let journal = example_path("journalctl_follow.log");

    let output = run_splash_file_args(&[
        "--auto",
        "--output",
        "jsonl",
        "--path",
        access.to_str().unwrap(),
        journal.to_str().unwrap(),
    ])
    .unwrap();

    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 11);
    assert!(lines[1].contains(r#""client":"10.0.0.5""#));
    assert!(lines[8].contains(r#""host":"web01""#));
}

#[cfg(unix)]
#[test]
fn test_followed_files_show_a_header_when_the_output_switches_files() {
    let directory = log_directory(&[("a.log", "alpha\n"), ("b.log", "beta\n")]);
    let first = directory.path().join("a.log");
    let second = directory.path().join("b.log");

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args([
            "--mode", "ad-hoc", "--output", "plain", "--follow", "--path",
        ])
        .args([&first, &second])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        while let Ok(count) = stdout.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let collect = |output: &mut Vec<u8>| {
        if let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
            output.extend(chunk);

            while let Ok(chunk) = receiver.recv_timeout(OUTPUT_QUIET_PERIOD) {
                output.extend(chunk);
            }
        }
    };

    let mut output = Vec::new();
    collect(&mut output);
    append_to(&second, "second beta\n");
    collect(&mut output);
    append_to(&first, "second alpha\n");
    collect(&mut output);

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }

    assert!(child.wait().unwrap().success());

    assert_eq!(
        String::from_utf8_lossy(&output),
        format!(
            "{}alpha\n{}beta\nsecond beta\n{}second alpha\n",
            header(&first),
            header(&second),
            header(&first)
        )
    );
}

#[test]
fn test_a_directory_renders_each_of_its_files() {
    let directory = log_directory(&[("api.log", "api line\n"), ("web.log", "web line\n")]);

    let output = run_splash_file_args(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--path",
        directory.path().to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(
        output,
        format!(
            "{}api line\n{}web line\n",
            header(&directory.path().join("api.log")),
            header(&directory.path().join("web.log"))
        )
    );
}

#[test]
fn test_a_directory_read_recursively_renders_the_files_of_its_subdirectories() {
    let directory = log_directory(&[("web.log", "web line\n")]);
    std::fs::create_dir(directory.path().join("api")).unwrap();
    std::fs::write(directory.path().join("api/api.log"), "api line\n").unwrap();

    let output = run_splash_file_args(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--recursive",
        "--path",
        directory.path().to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(
        output,
        format!(
            "{}api line\n{}web line\n",
            header(&directory.path().join("api/api.log")),
            header(&directory.path().join("web.log"))
        )
    );
}

#[test]
fn test_a_file_named_twice_is_rendered_once() {
    let directory = log_directory(&[("web.log", "web line\n")]);
    let file = directory.path().join("web.log");

    let output = run_splash_file_args(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--path",
        file.to_str().unwrap(),
        file.to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output, "web line\n");
}

#[test]
fn test_recursive_needs_a_path() {
    let output = run_splash(&["--recursive"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--path <PATH>"));
}

/// Follows `path` with `args`, makes `change` once the first output has
/// arrived, and returns everything written once the output for the change
/// has arrived and splash was interrupted
#[cfg(unix)]
fn follow_path(args: &[&str], path: &Path, change: &dyn Fn()) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .args(["--follow", "--path"])
        .arg(path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        while let Ok(count) = stdout.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let collect = |output: &mut Vec<u8>| {
        if let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
            output.extend(chunk);

            while let Ok(chunk) = receiver.recv_timeout(OUTPUT_QUIET_PERIOD) {
                output.extend(chunk);
            }
        }
    };

    let mut output = Vec::new();
    collect(&mut output);
    change();
    collect(&mut output);

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }

    assert!(child.wait().unwrap().success());

    String::from_utf8_lossy(&output).to_string()
}

#[cfg(unix)]
#[test]
fn test_a_file_created_in_a_followed_directory_is_followed() {
    let directory = log_directory(&[("web.log", "web line\n")]);
    let created = directory.path().join("api.log");

    let output = follow_path(
        &["--mode", "ad-hoc", "--output", "plain"],
        directory.path(),
        &|| std::fs::write(&created, "api line\n").unwrap(),
    );

    assert_eq!(
        output,
        format!(
            "{}web line\n{}api line\n",
            header(&directory.path().join("web.log")),
            header(&created)
        )
    );
}

#[cfg(unix)]
#[test]
fn test_a_file_created_in_a_subdirectory_of_a_recursively_followed_directory_is_followed() {
    let directory = log_directory(&[("web.log", "web line\n")]);
    std::fs::create_dir(directory.path().join("api")).unwrap();
    let created = directory.path().join("api/api.log");

    let output = follow_path(
        &["--mode", "ad-hoc", "--output", "plain", "--recursive"],
        directory.path(),
        &|| std::fs::write(&created, "api line\n").unwrap(),
    );

    assert_eq!(
        output,
        format!(
            "{}web line\n{}api line\n",
            header(&directory.path().join("web.log")),
            header(&created)
        )
    );
}

// ==================== Merge Tests ====================

const MERGE_API: &str =
    "2023-10-03T12:00:01Z api start\n2023-10-03T12:00:05Z panic: boom\n  at main.go:12\n";

const MERGE_WORKER: &str = "2023-10-03T12:00:03Z job 1\n2023-10-03T12:00:07Z job 2\n";

const MERGED: &str = "api.log 2023-10-03T12:00:01Z api start\nworker.log 2023-10-03T12:00:03Z job 1\napi.log 2023-10-03T12:00:05Z panic: boom\napi.log   at main.go:12\nworker.log 2023-10-03T12:00:07Z job 2\n";

#[test]
fn test_merged_files_are_interleaved_in_time_order_with_labels() {
    let directory = log_directory(&[("api.log", MERGE_API), ("worker.log", MERGE_WORKER)]);
    let pattern = directory.path().join("*.log");

    let output = run_splash_file_args(&[
        "--mode",
        "docker",
        "--output",
        "plain",
        "--merge",
        "--path",
        pattern.to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output, MERGED);
}

#[test]
fn test_statistics_on_merged_files_are_written_once() {
    let directory = log_directory(&[("api.log", MERGE_API), ("worker.log", MERGE_WORKER)]);
    let pattern = directory.path().join("*.log");

    let output = run_splash_file_args(&[
        "--mode",
        "docker",
        "--stats",
        "--stats-format",
        "csv",
        "--merge",
        "--path",
        pattern.to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output.matches("lines,,").collect::<Vec<_>>(), ["lines,,"]);
    assert!(output.contains("lines,,5\n"));
}

#[cfg(unix)]
#[test]
fn test_lines_merged_files_gain_are_labeled_without_headers() {
    let directory = log_directory(&[("api.log", MERGE_API), ("worker.log", MERGE_WORKER)]);
    let worker = directory.path().join("worker.log");
    let api = directory.path().join("api.log");

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args([
            "--mode", "docker", "--output", "plain", "--merge", "--follow", "--path",
        ])
        .args([&api, &worker])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        while let Ok(count) = stdout.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let collect = |output: &mut Vec<u8>| {
        if let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
            output.extend(chunk);

            while let Ok(chunk) = receiver.recv_timeout(OUTPUT_QUIET_PERIOD) {
                output.extend(chunk);
            }
        }
    };

    let mut output = Vec::new();
    collect(&mut output);
    append_to(&worker, "2023-10-03T12:00:11Z job 3\n");
    collect(&mut output);

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }

    assert!(child.wait().unwrap().success());
    assert_eq!(
        String::from_utf8_lossy(&output),
        format!("{}worker.log 2023-10-03T12:00:11Z job 3\n", MERGED)
    );
}

#[test]
fn test_merge_needs_a_path() {
    let output = run_splash(&["--merge"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--path <PATH>"));
}

// ==================== Aggregation Tests ====================

#[cfg(unix)]
#[test]
fn test_an_aggregator_labels_the_lines_each_host_forwards() {
    let mut aggregator = Listening::start(&["--mode", "docker", "--output", "plain", "--label"]);
    let directory = log_directory(&[("app.log", MERGE_WORKER)]);
    let url = format!("tcp://{}", aggregator.address);

    let edge = run_splash(&[
        "--mode",
        "docker",
        "--output",
        "plain",
        "--forward",
        &url,
        "--path",
        directory.path().join("app.log").to_str().unwrap(),
    ])
    .unwrap();
    aggregator.collect();
    let (success, output) = aggregator.interrupt();

    assert!(edge.status.success());
    assert!(success);
    assert_eq!(
        output,
        "127.0.0.1 2023-10-03T12:00:03Z job 1\n127.0.0.1 2023-10-03T12:00:07Z job 2\n"
    );
}

#[cfg(unix)]
#[test]
fn test_syslog_messages_are_labeled_with_the_host_that_sent_them() {
    let mut receiving = Listening::receive(
        &["--mode", "syslog", "--output", "plain", "--label"],
        "--syslog",
        "Receiving syslog messages on ",
    );

    receiving.send_datagram(SYSLOG_UDP);
    let (_, output) = receiving.interrupt();

    assert_eq!(output, format!("127.0.0.1 {}\n", SYSLOG_UDP));
}

#[test]
fn test_a_labeled_file_starts_each_line_with_its_name() {
    let directory = log_directory(&[("app.log", "alpha\nbeta\n")]);

    let output = run_splash_file_args(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--label",
        "--path",
        directory.path().join("app.log").to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output, "app.log alpha\napp.log beta\n");
}

// ==================== Plugin File Tests ====================

fn example_plugins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("plugins")
}

fn plugin_output(mode: &str, example: &str) -> String {
    run_splash_file_args(&[
        "--plugin-dir",
        example_plugins().to_str().unwrap(),
        "--mode",
        mode,
        "--output",
        "jsonl",
        "--fields",
        "level,pid,module,request,transaction,success,failure,client,method,status,duration",
        "--path",
        example_path(example).to_str().unwrap(),
    ])
    .unwrap()
}

#[test]
fn test_the_gunicorn_example_plugin_reads_gunicorn_logs() {
    let output = plugin_output("gunicorn", "gunicorn.log");
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], r#"{"level":"INFO","pid":"4100"}"#);
    assert_eq!(lines[3], r#"{"level":"CRITICAL","pid":"4100"}"#);
}

#[test]
fn test_the_celery_example_plugin_reads_celery_logs() {
    let output = plugin_output("celery", "celery.log");

    assert_eq!(
        output,
        concat!(
            r#"{"level":"INFO","module":"MainProcess"}"#,
            "\n",
            r#"{"level":"INFO","module":"ForkPoolWorker-1","request":"app.tasks.send_email","transaction":"8f3c2a1b-0d4e-4f5a-9b6c-7d8e9f0a1b2c","success":"succeeded","duration":"0.42s"}"#,
            "\n",
            r#"{"level":"ERROR","module":"ForkPoolWorker-2","request":"app.tasks.build_report","transaction":"1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d","failure":"raised unexpected"}"#,
            "\n"
        )
    );
}

#[test]
fn test_the_uwsgi_example_plugin_reads_uwsgi_logs() {
    let output = plugin_output("uwsgi", "uwsgi.log");

    assert_eq!(
        output.lines().next().unwrap(),
        r#"{"pid":"4101","request":"/index.html","client":"10.0.0.5","method":"GET","status":"200","duration":"3 msecs"}"#
    );
}

#[test]
fn test_the_django_example_plugin_reads_gunicorn_lines_with_the_plugin_it_requires() {
    let output = plugin_output("django", "django.log");

    assert_eq!(
        output,
        concat!(
            r#"{"level":"INFO","pid":"4100"}"#,
            "\n",
            r#"{"level":"INFO","pid":"4101"}"#,
            "\n",
            r#"{"level":"INFO","module":"django.utils.autoreload"}"#,
            "\n",
            r#"{"level":"ERROR","module":"django.request","request":"/reports/export","failure":"Error"}"#,
            "\n",
            r#"{"level":"WARNING","module":"django.security.csrf","request":"/orders/"}"#,
            "\n",
            r#"{"level":"CRITICAL","pid":"4100"}"#,
            "\n"
        )
    );
}

#[test]
fn test_a_plugin_file_whose_required_plugin_is_missing_is_reported() {
    let directory = log_directory(&[(
        "needs.toml",
        "name = \"needs\"\nrequires = \"absent\"\n[patterns]\nall = '.+'\n",
    )]);

    let output = run_splash(&[
        "--plugin-dir",
        directory.path().to_str().unwrap(),
        "--list-plugins",
    ])
    .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Cannot load plugin {}: requires plugin 'absent', which is not loaded\n",
            directory.path().join("needs.toml").display()
        )
    );
}

#[test]
fn test_regex_testing_a_plugin_whose_required_plugin_is_missing_fails() {
    let directory = log_directory(&[(
        "needs.toml",
        "name = \"needs\"\nrequires = \"absent\"\n[patterns]\nall = '.+'\n",
    )]);
    let plugin = directory.path().join("needs.toml");

    let output = run_splash(&["--test-regex", plugin.to_str().unwrap()]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(&format!(
        "Cannot load plugin {}: requires plugin 'absent', which is not loaded",
        plugin.display()
    )));
}

#[test]
fn test_every_example_plugin_reads_every_line_of_its_example_log() {
    for name in ["gunicorn", "celery", "uwsgi", "django"] {
        let output = run_splash(&[
            "--plugin-dir",
            example_plugins().to_str().unwrap(),
            "--test-regex",
            example_plugins()
                .join(format!("{}.toml", name))
                .to_str()
                .unwrap(),
            "--path",
            example_path(&format!("{}.log", name)).to_str().unwrap(),
        ])
        .unwrap();

        assert!(output.status.success(), "{}", name);
    }
}

#[test]
fn test_auto_mode_detects_a_plugin_files_format() {
    let output = run_splash_file_args(&[
        "--plugin-dir",
        example_plugins().to_str().unwrap(),
        "--auto",
        "--output",
        "jsonl",
        "--fields",
        "level",
        "--path",
        example_path("gunicorn.log").to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output.lines().next().unwrap(), r#"{"level":"INFO"}"#);
}

#[test]
fn test_list_plugins_names_loaded_plugin_files_and_their_directory() {
    let output = run_splash(&[
        "--plugin-dir",
        example_plugins().to_str().unwrap(),
        "--list-plugins",
    ])
    .unwrap();
    let listed = String::from_utf8_lossy(&output.stdout);

    assert!(listed.contains("celery"));
    assert!(listed.contains(&format!("  {}\n", example_plugins().display())));
}

#[test]
fn test_a_plugin_file_that_cannot_be_loaded_is_reported_and_splash_goes_on() {
    let directory = log_directory(&[("broken.toml", "name = \"broken\"\n")]);

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--plugin-dir", directory.path().to_str().unwrap()])
        .args(["--mode", "ad-hoc", "--output", "plain"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"service started\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "service started\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "Cannot load plugin {}: the plugin has no [patterns]\n",
            directory.path().join("broken.toml").display()
        )
    );
}

const ORDERS_PLUGIN: &str = "name = \"orders\"\n[patterns]\nplaced = '^(?P<level>[A-Z]+) order (?P<transaction>\\d+) placed$'\n";

#[test]
fn test_test_regex_reports_each_line_and_succeeds_when_all_are_read() {
    let directory = log_directory(&[("orders.toml", ORDERS_PLUGIN)]);
    let plugin = directory.path().join("orders.toml");

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--test-regex", plugin.to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"INFO order 42 placed\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "1: placed\n   level            INFO\n   transaction      42\n1 of 1 lines matched\n"
    );
}

#[test]
fn test_test_regex_fails_when_a_line_is_not_read() {
    let directory = log_directory(&[
        ("orders.toml", ORDERS_PLUGIN),
        ("a.log", "INFO order 1 placed\n"),
        ("b.log", "order lost\n"),
    ]);

    let output = run_splash(&[
        "--test-regex",
        directory.path().join("orders.toml").to_str().unwrap(),
        "--path",
        directory.path().join("*.log").to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "{}1: placed\n   level            INFO\n   transaction      1\n1 of 1 lines matched\n{}1: no pattern matched\n   order lost\n0 of 1 lines matched\n",
            header(&directory.path().join("a.log")),
            header(&directory.path().join("b.log"))
        )
    );
}

#[test]
fn test_test_regex_reports_a_plugin_file_that_cannot_be_loaded() {
    let output = run_splash(&["--test-regex", "missing.toml"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .starts_with("Error: Cannot load plugin missing.toml: missing.toml: "));
}

#[test]
fn test_test_regex_reports_a_path_that_cannot_be_read() {
    let directory = log_directory(&[("orders.toml", ORDERS_PLUGIN)]);
    let plugin = directory.path().join("orders.toml");

    let missing_pattern = run_splash(&[
        "--test-regex",
        plugin.to_str().unwrap(),
        "--path",
        directory.path().join("*.log").to_str().unwrap(),
    ])
    .unwrap();
    let missing_file = run_splash(&[
        "--test-regex",
        plugin.to_str().unwrap(),
        "--path",
        directory.path().join("missing.log").to_str().unwrap(),
    ])
    .unwrap();

    assert!(!missing_pattern.status.success());
    assert!(String::from_utf8_lossy(&missing_pattern.stderr).starts_with("Error: No files match"));
    assert!(!missing_file.status.success());
}

#[cfg(unix)]
#[test]
fn test_a_plugin_directory_that_cannot_be_read_does_not_stop_reloading() {
    use std::os::unix::fs::PermissionsExt;

    let directory = log_directory(&[]);
    let locked = directory.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    let output = run_splash_with_stdin_args(
        &[
            "--plugin-dir",
            locked.to_str().unwrap(),
            "--reload-plugins",
            "--mode",
            "ad-hoc",
            "--output",
            "plain",
        ],
        "service started\n",
    );

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(output.unwrap(), "service started\n");
}

#[cfg(unix)]
#[test]
fn test_a_changed_plugin_file_is_used_for_the_lines_that_follow() {
    use std::io::BufRead;

    let directory = log_directory(&[
        ("orders.toml", ORDERS_PLUGIN),
        ("orders.log", "INFO order 1 placed\n"),
    ]);
    let plugin = directory.path().join("orders.toml");
    let log = directory.path().join("orders.log");

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--plugin-dir", directory.path().to_str().unwrap()])
        .args(["--reload-plugins", "--mode", "orders", "--output", "jsonl"])
        .args([
            "--fields",
            "level,message,transaction",
            "--follow",
            "--path",
        ])
        .arg(&log)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        while let Ok(count) = stdout.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    let (report_sender, reports) = mpsc::channel();
    let stderr = child.stderr.take().unwrap();

    thread::spawn(move || {
        for line in std::io::BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
        {
            let _ = report_sender.send(line);
        }
    });

    let collect = |output: &mut Vec<u8>| {
        if let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
            output.extend(chunk);

            while let Ok(chunk) = receiver.recv_timeout(OUTPUT_QUIET_PERIOD) {
                output.extend(chunk);
            }
        }
    };

    let mut output = Vec::new();
    collect(&mut output);

    let replacement = directory.path().join("orders.toml.new");
    std::fs::write(
        &replacement,
        "name = \"orders\"\nversion = \"2.0.0\"\n[patterns]\nplaced = '^(?P<level>[A-Z]+) (?P<message>.*)$'\n",
    )
    .unwrap();
    std::fs::rename(&replacement, &plugin).unwrap();

    while reports.recv_timeout(FIRST_OUTPUT_DEADLINE).unwrap() != "Reloaded plugins: orders 2.0.0" {
    }

    append_to(&log, "INFO order 2 placed\n");
    collect(&mut output);

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }

    assert!(child.wait().unwrap().success());
    assert_eq!(
        String::from_utf8_lossy(&output),
        "{\"level\":\"INFO\",\"transaction\":\"1\"}\n{\"level\":\"INFO\",\"message\":\"order 2 placed\"}\n"
    );
}

// ==================== Rate Limit Tests ====================

#[test]
fn test_lines_over_the_rate_limit_are_dropped_and_counted_on_stderr() {
    let directory = tempfile::TempDir::new().unwrap();
    let log = directory.path().join("burst.log");
    std::fs::write(&log, "one\ntwo\nthree\nfour\nfive\n").unwrap();

    let output = run_splash(&[
        "--mode",
        "ad-hoc",
        "--output",
        "plain",
        "--rate-limit",
        "2",
        "--path",
        log.to_str().unwrap(),
    ])
    .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "one\ntwo\n");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Dropped 3 lines over the limit of 2 per second\n"
    );
}

#[test]
fn test_stdin_under_the_rate_limit_is_rendered_whole() {
    let input = example_contents("stats_access.log");

    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "httpd",
            "--output",
            "plain",
            "--rate-limit",
            "1000",
        ],
        &input,
    )
    .unwrap();

    assert_eq!(output, input);
}

#[test]
fn test_auto_detected_stdin_under_the_rate_limit_is_rendered_whole() {
    let input = example_contents("stats_access.log");

    let output = run_splash_with_stdin_args(
        &["--auto", "--output", "plain", "--rate-limit", "1000"],
        &input,
    )
    .unwrap();

    assert_eq!(output, input);
}

#[test]
fn test_a_rate_limit_of_zero_is_rejected() {
    let output = run_splash(&["--rate-limit", "0"]).unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--rate-limit <LINES>"));
}

#[test]
fn test_a_rate_limit_cannot_be_combined_with_the_curses_viewer() {
    let output = run_splash(&["--rate-limit", "10", "--output", "curses"]).unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: --rate-limit cannot be combined with the curses viewer\n"
    );
}

#[cfg(unix)]
#[test]
fn test_a_new_rate_limit_window_keeps_lines_again() {
    let (output, errors) = follow_file_with_errors(
        &["--mode", "ad-hoc", "--output", "plain", "--rate-limit", "1"],
        b"one\ntwo\nthree\n",
        Some(&|log: &Path| {
            thread::sleep(splash::rate::WINDOW + Duration::from_millis(100));
            append_to(log, "four\n");
        }),
    );

    assert_eq!(output, "one\nfour\n");
    assert_eq!(errors, "Dropped 2 lines over the limit of 1 per second\n");
}

// ==================== Container and Journal Pipe Tests ====================

/// The JSON token kinds of each line splash writes, leaving out plain text
fn json_kinds(output: &str) -> Vec<Vec<String>> {
    let kind = regex::Regex::new(r#""kind":"([a-z_]+)""#).unwrap();

    output
        .lines()
        .map(|line| {
            kind.captures_iter(line)
                .map(|found| found[1].to_string())
                .filter(|found| found != "plain")
                .collect()
        })
        .collect()
}

#[test]
fn test_docker_logs_output_is_shown_in_each_containers_format() {
    let output = run_splash_with_stdin_args(
        &["--mode", "docker", "--output", "json"],
        &example_contents("docker_logs.log"),
    )
    .unwrap();

    let kinds = json_kinds(&output);

    assert_eq!(kinds.len(), 4);
    assert_eq!(kinds[0][0], "client");
    assert_eq!(kinds[1][..3], ["timestamp", "punctuation", "failure"]);
    assert_eq!(kinds[2], ["message"]);
    assert_eq!(kinds[3][..2], ["timestamp", "client"]);
}

#[test]
fn test_docker_logs_output_keeps_every_line() {
    let input = example_contents("docker_logs.log");

    let output =
        run_splash_with_stdin_args(&["--mode", "docker", "--output", "plain"], &input).unwrap();

    assert_eq!(output, input);
}

#[test]
fn test_kubectl_logs_output_is_shown_with_the_k8s_mode() {
    let output = run_splash_with_stdin_args(
        &["--mode", "k8s", "--output", "json"],
        &example_contents("kubectl_logs.log"),
    )
    .unwrap();

    let kinds = json_kinds(&output);

    assert_eq!(kinds.len(), 5);
    assert_eq!(kinds[0][..2], ["level", "timestamp"]);
    assert_eq!(kinds[1], ["message"]);
    assert_eq!(kinds[2][..2], ["timestamp", "client"]);
    assert_eq!(kinds[3][..2], ["module", "client"]);
    assert_eq!(kinds[4], ["module", "timestamp", "message"]);
}

#[test]
fn test_the_systemd_mode_is_the_journalctl_mode() {
    let input = example_contents("journalctl_follow.log");

    let systemd =
        run_splash_with_stdin_args(&["--mode", "systemd", "--output", "json"], &input).unwrap();
    let journalctl =
        run_splash_with_stdin_args(&["--mode", "journalctl", "--output", "json"], &input).unwrap();

    assert_eq!(systemd, journalctl);
    assert_eq!(json_kinds(&systemd)[1][..3], ["timestamp", "host", "tag"]);
}

/// Streams `example` into splash through a pipe that stays open, as
/// `docker logs -f`, `kubectl logs -f`, and `journalctl -f` do, and returns
/// what splash wrote before the pipe closed
#[cfg(unix)]
fn followed_pipe_output(mode: &str, example: &str) -> String {
    let (mut child, stdin, receiver, shown_before_input_ended) = stream_stdin_example(
        &["--mode", mode, "--output", "plain"],
        &example_contents(example),
    );

    assert!(!shown_before_input_ended.is_empty());

    drop(stdin);
    child.wait().unwrap();

    let mut output = shown_before_input_ended;
    output.extend(receiver.iter().flatten());

    String::from_utf8_lossy(&output).to_string()
}

#[cfg(unix)]
#[test]
fn test_docker_logs_follow_is_shown_as_it_arrives() {
    assert_eq!(
        followed_pipe_output("docker", "docker_logs.log"),
        example_contents("docker_logs.log")
    );
}

#[cfg(unix)]
#[test]
fn test_kubectl_logs_follow_is_shown_as_it_arrives() {
    assert_eq!(
        followed_pipe_output("k8s", "kubectl_logs.log"),
        example_contents("kubectl_logs.log")
    );
}

#[cfg(unix)]
#[test]
fn test_journalctl_follow_is_shown_as_it_arrives() {
    assert_eq!(
        followed_pipe_output("systemd", "journalctl_follow.log"),
        example_contents("journalctl_follow.log")
    );
}

// ==================== Forwarding Tests ====================

#[test]
fn test_output_is_forwarded_before_splash_exits() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("tcp://{}", listener.local_addr().unwrap());
    let input = example_contents("stats_access.log");

    let output = run_splash_with_stdin_args(
        &["--mode", "httpd", "--output", "plain", "--forward", &url],
        &input,
    )
    .unwrap();

    let mut forwarded = String::new();
    listener
        .accept()
        .unwrap()
        .0
        .read_to_string(&mut forwarded)
        .unwrap();

    assert_eq!(output, input);
    assert_eq!(forwarded, input);
}

#[test]
fn test_an_invalid_forward_url_is_reported() {
    let output = run_splash(&["--forward", "https://logs.example.com:443/"]).unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: Cannot forward over https (expected tcp, udp, or http)\n"
    );
}

#[test]
fn test_forwarding_cannot_be_combined_with_the_curses_viewer() {
    let output = run_splash(&["--forward", "tcp://127.0.0.1:9", "--output", "curses"]).unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: --forward sends the output splash writes, and the curses viewer writes none\n"
    );
}

#[test]
fn test_a_forward_failure_is_reported_on_stderr() {
    let address = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let url = format!("tcp://{}", address);

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--mode", "ad-hoc", "--output", "plain", "--forward", &url])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"service started\n")
        .unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .starts_with(&format!("Cannot forward to {}: ", url)));
}

// ==================== Stdin Streaming Tests ====================

/// Starts splash reading a pipe that stays open, writes `input` to it, and
/// waits for the output those lines produce
#[cfg(unix)]
fn stream_stdin_example(
    args: &[&str],
    input: &str,
) -> (
    std::process::Child,
    std::process::ChildStdin,
    mpsc::Receiver<Vec<u8>>,
    Vec<u8>,
) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        while let Ok(count) = stdout.read(&mut buffer) {
            if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });

    stdin.write_all(input.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let mut output = Vec::new();

    if let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
        output.extend(chunk);

        while let Ok(chunk) = receiver.recv_timeout(OUTPUT_QUIET_PERIOD) {
            output.extend(chunk);
        }
    }

    (child, stdin, receiver, output)
}

#[cfg(unix)]
fn interrupt(child: &std::process::Child) {
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
}

#[test]
fn test_invalid_bytes_on_stdin_are_replaced_and_later_lines_are_rendered() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--mode", "ad-hoc", "--output", "plain"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(&std::fs::read(example_path("stdin_invalid_bytes.log")).unwrap())
        .unwrap();

    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "connection from 10.0.0.5 \u{fffd}\u{fffd} accepted\nconnection from 10.0.0.7 closed\n"
    );
}

#[test]
fn test_a_path_of_dash_reads_stdin() {
    let output = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "plain", "--path", "-"],
        &example_contents("adhoc_ips.log"),
    )
    .unwrap();

    assert_eq!(output, example_contents("adhoc_ips.log"));
}

#[test]
fn test_crlf_line_endings_on_stdin_are_dropped() {
    let output = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "plain"],
        "first line\r\nsecond line\r\n",
    )
    .unwrap();

    assert_eq!(output, "first line\nsecond line\n");
}

#[cfg(unix)]
#[test]
fn test_splash_stops_quietly_when_the_reader_of_its_output_exits() {
    use std::os::unix::process::ExitStatusExt;

    let mut child = Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(["--mode", "httpd"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let input = example_contents("stats_access.log").repeat(5000);

    let writer = thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });

    let mut first_output = [0u8; 1024];
    child
        .stdout
        .take()
        .unwrap()
        .read_exact(&mut first_output)
        .unwrap();

    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();

    assert_eq!(output.status.signal(), Some(libc::SIGPIPE));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[cfg(unix)]
#[test]
fn test_summary_is_written_when_reading_stdin_is_interrupted() {
    let (mut child, stdin, receiver, mut output) = stream_stdin_example(
        &[
            "--summary",
            "--output",
            "plain",
            "--mode",
            "httpd",
            "--tz",
            "UTC",
        ],
        &example_contents("stats_access.log"),
    );

    interrupt(&child);
    drop(stdin);

    assert!(child.wait().unwrap().success());

    while let Ok(chunk) = receiver.recv_timeout(FIRST_OUTPUT_DEADLINE) {
        output.extend(chunk);
    }

    let output = String::from_utf8_lossy(&output);

    assert!(output.contains("/api/queue/91"));
    assert!(output.ends_with("To:          2023-10-03T14:10:01Z\n"));
}

#[cfg(unix)]
#[test]
fn test_a_second_interrupt_stops_reading_stdin_at_once() {
    use std::os::unix::process::ExitStatusExt;

    let (mut child, _stdin, _receiver, _output) = stream_stdin_example(
        &[
            "--summary",
            "--output",
            "plain",
            "--mode",
            "httpd",
            "--tz",
            "UTC",
        ],
        &example_contents("stats_access.log"),
    );

    let status = loop {
        interrupt(&child);

        match child.try_wait().unwrap() {
            Some(status) => break status,
            None => thread::sleep(OUTPUT_QUIET_PERIOD),
        }
    };

    assert_eq!(status.signal(), Some(libc::SIGINT));
}

// ==================== Viewer Terminal Tests ====================

/// How long a viewer run in a pseudo-terminal may take to draw what a test
/// waits for
#[cfg(unix)]
const VIEWER_DEADLINE: Duration = Duration::from_secs(60);

/// Splash running in a pseudo-terminal that is its controlling terminal, so
/// the viewer reads keys from it and never from the terminal running the
/// tests
#[cfg(unix)]
struct TerminalRun {
    child: std::process::Child,
    keyboard: std::fs::File,
    screen: mpsc::Receiver<Vec<u8>>,
    drawn: String,
}

#[cfg(unix)]
impl TerminalRun {
    /// Starts splash in a 24 by 120 terminal, with standard input from a
    /// pipe holding `piped` when it is given
    fn start(args: &[&str], piped: Option<&str>) -> TerminalRun {
        use std::os::fd::{FromRawFd, OwnedFd};
        use std::os::unix::process::CommandExt;

        let (mut primary, mut secondary) = (0, 0);
        let mut size = libc::winsize {
            ws_row: 24,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let opened = unsafe {
            libc::openpty(
                &mut primary,
                &mut secondary,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        assert_eq!(opened, 0);

        let primary = unsafe { std::fs::File::from_raw_fd(primary) };
        let secondary_handle = unsafe { OwnedFd::from_raw_fd(secondary) };
        let keep_stdin = piped.is_some();

        let mut command = Command::new(env!("CARGO_BIN_EXE_splash"));
        command.args(args).stdin(if keep_stdin {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        });

        unsafe {
            command.pre_exec(move || {
                let saved_stdin = libc::dup(0);

                if libc::login_tty(secondary) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                if keep_stdin {
                    libc::dup2(saved_stdin, 0);
                }

                libc::close(saved_stdin);

                Ok(())
            });
        }

        let mut child = command.spawn().unwrap();
        drop(command);
        drop(secondary_handle);

        if let (Some(text), Some(mut stdin)) = (piped, child.stdin.take()) {
            stdin.write_all(text.as_bytes()).unwrap();
        }

        let keyboard = primary.try_clone().unwrap();
        let (sender, screen) = mpsc::channel();

        thread::spawn(move || {
            let mut primary = primary;
            let mut buffer = [0u8; 8192];

            while let Ok(count) = primary.read(&mut buffer) {
                if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                    break;
                }
            }
        });

        TerminalRun {
            child,
            keyboard,
            screen,
            drawn: String::new(),
        }
    }

    /// Waits until the terminal shows `text`
    fn wait_for(&mut self, text: &str) {
        let deadline = std::time::Instant::now() + VIEWER_DEADLINE;

        while !self.drawn.contains(text) {
            let left = deadline.saturating_duration_since(std::time::Instant::now());

            match self.screen.recv_timeout(left) {
                Ok(chunk) => self.drawn.push_str(&String::from_utf8_lossy(&chunk)),
                Err(_) => {
                    let _ = self.child.kill();
                    panic!("never showed {:?}, showed {:?}", text, self.drawn);
                }
            }
        }
    }

    fn type_keys(&mut self, keys: &str) {
        self.keyboard.write_all(keys.as_bytes()).unwrap();
    }

    /// Waits for splash to exit, returning whether it succeeded
    fn finish(mut self) -> bool {
        let deadline = std::time::Instant::now() + VIEWER_DEADLINE;

        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.success();
            }

            if std::time::Instant::now() > deadline {
                let _ = self.child.kill();
                panic!("splash did not exit, showed {:?}", self.drawn);
            }

            thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(unix)]
#[test]
fn test_the_viewer_shows_a_log_file_until_quit() {
    let path = example_path("stats_access.log");
    let mut run = TerminalRun::start(
        &[
            "--output",
            "curses",
            "--mode",
            "httpd",
            "--path",
            path.to_str().unwrap(),
        ],
        None,
    );

    run.wait_for("lines 1-6 of 6");
    run.type_keys("q");

    assert!(run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_searches_and_splits_the_view() {
    let path = example_path("stats_access.log");
    let mut run = TerminalRun::start(
        &[
            "--output",
            "curses",
            "--mode",
            "httpd",
            "--path",
            path.to_str().unwrap(),
        ],
        None,
    );

    run.wait_for("lines 1-6 of 6");
    run.type_keys("/503\r");
    run.wait_for("match at line 6");
    run.type_keys("s");
    run.wait_for("---- log text ----");
    run.type_keys("q");

    assert!(run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_shows_lines_a_followed_file_gains() {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("access.log");
    std::fs::write(&path, example_contents("stats_access.log")).unwrap();

    let mut run = TerminalRun::start(
        &[
            "--output",
            "curses",
            "--mode",
            "httpd",
            "--path",
            path.to_str().unwrap(),
        ],
        None,
    );

    run.wait_for("lines 1-6 of 6");
    run.type_keys("f");
    run.wait_for("[follow]");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"10.0.0.9 - - [03/Oct/2023:14:20:00 +0000] \"GET /fresh HTTP/1.1\" 200 1 \"-\" \"curl/8.4.0\"\n")
        .unwrap();
    run.wait_for("lines 1-7 of 7");
    run.type_keys("q");

    assert!(run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_shows_a_piped_log() {
    let mut run = TerminalRun::start(
        &["--output", "curses", "--mode", "httpd"],
        Some(&example_contents("stats_access.log")),
    );

    run.wait_for("lines 1-6 of 6");
    run.type_keys("q");

    assert!(run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_shows_lines_received_over_the_network() {
    let mut run = TerminalRun::start(
        &[
            "--output",
            "curses",
            "--mode",
            "httpd",
            "--listen",
            "127.0.0.1:0",
        ],
        None,
    );

    run.wait_for("Listening on 127.0.0.1:");
    run.wait_for("\n");

    let announced = run.drawn.split("Listening on ").nth(1).unwrap();
    let address = announced.split_whitespace().next().unwrap().to_string();

    std::net::TcpStream::connect(address)
        .unwrap()
        .write_all(example_contents("stats_access.log").as_bytes())
        .unwrap();

    run.wait_for("lines 1-6 of 6");
    run.type_keys("q");

    assert!(run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_shows_syslog_messages_it_receives() {
    let mut run = TerminalRun::start(&["--output", "curses", "--syslog", "127.0.0.1:0"], None);

    run.wait_for(" (TCP and UDP)");

    let announced = run
        .drawn
        .split("Receiving syslog messages on ")
        .nth(1)
        .unwrap();
    let address = announced.split_whitespace().next().unwrap().to_string();

    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .send_to(SYSLOG_UDP.as_bytes(), address)
        .unwrap();

    run.wait_for("lines 1-1 of 1");
    run.type_keys("q");

    assert!(run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_shows_one_file_at_a_time() {
    let access = example_path("stats_access.log");
    let journal = example_path("journalctl_follow.log");
    let mut run = TerminalRun::start(
        &[
            "--output",
            "curses",
            "--path",
            access.to_str().unwrap(),
            journal.to_str().unwrap(),
        ],
        None,
    );

    run.wait_for("The curses viewer shows one file; give --path one file");

    assert!(!run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_needs_a_file_in_a_directory_to_show() {
    let directory = tempfile::TempDir::new().unwrap();
    let mut run = TerminalRun::start(
        &[
            "--output",
            "curses",
            "--path",
            directory.path().to_str().unwrap(),
        ],
        None,
    );

    run.wait_for("The curses viewer shows one file; give --path one file");

    assert!(!run.finish());
}

#[cfg(unix)]
#[test]
fn test_the_viewer_needs_a_log_to_show() {
    let mut run = TerminalRun::start(&["--output", "curses"], None);

    run.wait_for("curses output needs a log; give --path or pipe one in");

    assert!(!run.finish());
}

// ==================== HTML Style Sheet and Line Number Tests ====================

fn example_css() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("html")
        .join("report.css")
}

#[test]
fn test_html_output_ends_its_style_element_with_the_style_sheet() {
    let output = run_splash_file_args(&[
        "--mode",
        "httpd",
        "--output",
        "html",
        "--css",
        example_css().to_str().unwrap(),
        "--path",
        example_path("httpd_combined.log").to_str().unwrap(),
    ])
    .unwrap();

    assert!(output.contains(&format!(
        "{}</style>\n",
        std::fs::read_to_string(example_css()).unwrap()
    )));
}

#[test]
fn test_html_line_numbers_number_every_input_line_including_blank_ones() {
    let output = run_splash_with_stdin_args(
        &["--mode", "ad-hoc", "--output", "html", "--line-numbers"],
        "service started\n\nservice stopped\n",
    )
    .unwrap();

    assert!(output.contains(
        "<span class=\"splash-line-number\">1</span><span class=\"splash-plain\">service</span>"
    ));
    assert!(output.contains(
        "<span class=\"splash-line-number\">3</span><span class=\"splash-plain\">service</span>"
    ));
    assert!(!output.contains("<span class=\"splash-line-number\">2</span>"));
}

#[test]
fn test_line_numbers_without_html_output_fail() {
    let output = run_splash(&["--line-numbers", "--output", "plain"]).unwrap();

    assert!(!output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "Error: --line-numbers works only with --output html\n"
    );
}

#[cfg(unix)]
#[test]
fn test_jsonl_output_writes_complete_lines_before_the_input_ends() {
    let input = example_contents("syslog_traditional.log");
    let (mut child, stdin, receiver, shown_before_input_ended) =
        stream_stdin_example(&["--mode", "syslog", "--output", "jsonl"], &input);

    let shown = String::from_utf8_lossy(&shown_before_input_ended).to_string();

    assert!(!shown.is_empty());
    assert!(shown.ends_with('\n'), "{:?}", shown);

    drop(stdin);
    child.wait().unwrap();

    let mut output = shown_before_input_ended;
    output.extend(receiver.iter().flatten());
    let output = String::from_utf8_lossy(&output).to_string();

    assert_eq!(output.lines().count(), input.lines().count());
    assert!(output
        .lines()
        .all(|line| line.starts_with("{\"text\":\"") && line.ends_with('}')));
}

// ==================== Plain Output Whitespace and Line Ending Tests ====================

const MIXED_ENDINGS: &str = "name\tstatus   code\r\nOct  3 12:00:01 web01 cron[5101]: job finished\n\r\nprogress 10%\rprogress 90%\r\nlast line";

const MIXED_ENDINGS_PLAIN: &str = "name\tstatus   code\nOct  3 12:00:01 web01 cron[5101]: job finished\nprogress 10%\rprogress 90%\nlast line\n";

#[test]
fn test_plain_output_of_a_file_keeps_tabs_and_spaces_and_ends_lines_with_lf() {
    let directory = log_directory(&[("mixed.log", MIXED_ENDINGS)]);

    let output = run_splash_file_args(&[
        "--output",
        "plain",
        "--path",
        directory.path().join("mixed.log").to_str().unwrap(),
    ])
    .unwrap();

    assert_eq!(output, MIXED_ENDINGS_PLAIN);
}

#[test]
fn test_plain_output_of_stdin_keeps_tabs_and_spaces_and_ends_lines_with_lf() {
    let output = run_splash_with_stdin_args(&["--output", "plain"], MIXED_ENDINGS).unwrap();

    assert_eq!(output, MIXED_ENDINGS_PLAIN);
}

#[test]
fn test_plain_output_in_auto_mode_handles_line_endings_the_same_way() {
    let output =
        run_splash_with_stdin_args(&["--auto", "--output", "plain"], MIXED_ENDINGS).unwrap();

    assert_eq!(output, MIXED_ENDINGS_PLAIN);
}

// ==================== Mixed Format Stream Tests ====================

const MIXED_STREAM_MODES: [&str; 7] = [
    "clf",
    "caddy",
    "ssh",
    "nginx-error",
    "postgresql",
    "ad-hoc",
    "kubernetes",
];

fn kinds_in_mode(line: &str, mode: &str) -> Vec<String> {
    json_kinds(
        &run_splash_with_stdin_args(
            &["--mode", mode, "--output", "json"],
            &format!("{}\n", line),
        )
        .unwrap(),
    )
    .remove(0)
}

#[test]
fn test_auto_mode_colors_each_line_of_a_mixed_stream_in_its_own_format() {
    let contents = example_contents("mixed_stream.log");
    let kinds = sample_kinds("mixed_stream.log", &["--auto"]);

    assert_eq!(kinds.len(), MIXED_STREAM_MODES.len());

    for ((line, mode), line_kinds) in contents.lines().zip(MIXED_STREAM_MODES).zip(&kinds) {
        assert_eq!(*line_kinds, kinds_in_mode(line, mode), "{}", line);
    }
}

#[test]
fn test_auto_mode_colors_lines_of_other_formats_inside_a_log_one_format_dominates() {
    let syslog = example_contents("syslog_traditional.log");
    let mixed = example_contents("mixed_stream.log");
    let contents = format!("{}{}{}", syslog.repeat(10), mixed, syslog);

    let kinds = json_kinds(
        &run_splash_with_stdin_args(&["--auto", "--output", "json"], &contents).unwrap(),
    );
    let first_mixed = syslog.lines().count() * 10;

    for (index, (line, mode)) in mixed.lines().zip(MIXED_STREAM_MODES).enumerate() {
        assert_eq!(
            kinds[first_mixed + index],
            kinds_in_mode(line, mode),
            "{}",
            line
        );
    }

    assert_eq!(
        kinds.len(),
        contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count()
    );
}

#[test]
fn test_auto_mode_keeps_every_line_of_a_mixed_stream() {
    assert_every_line_kept("mixed_stream.log", &["--auto"]);
}

#[test]
fn test_an_excluded_level_drops_its_entries_and_their_stack_traces() {
    let output = run_splash_with_stdin_args(
        &["--auto", "--output", "plain", "--level", "!error"],
        &example_contents("filter_app.log"),
    )
    .unwrap();

    assert_eq!(
        output,
        example_contents("filter_app.log")
            .lines()
            .filter(|line| !line.contains("ERROR")
                && !line.starts_with("java")
                && !line.starts_with('\t'))
            .map(|line| format!("{}\n", line))
            .collect::<String>()
    );
}

#[test]
fn test_a_negated_field_filter_drops_lines_holding_the_field() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "httpd",
            "--output",
            "plain",
            "--filter",
            "status!=200",
        ],
        &example_contents("httpd_combined.log"),
    )
    .unwrap();

    assert!(!output.is_empty());
    assert!(output.lines().all(|line| !line.contains("\" 200 ")));
}

#[test]
fn test_every_kind_of_filter_combines() {
    let output = run_splash_with_stdin_args(
        &[
            "--auto",
            "--output",
            "plain",
            "--level",
            "info,warn,error,!error",
            "--since",
            "2023-10-03T12:00:02",
            "--until",
            "2023-10-03T12:00:04",
            "--grep",
            "node-1",
            "--exclude",
            "publish_address",
            "--filter",
            "level!=INFO",
        ],
        &example_contents("filter_app.log"),
    )
    .unwrap();

    assert_eq!(
        output,
        "[2023-10-03T12:00:03,345][WARN ][o.e.c.r.a.DiskThresholdMonitor] [node-1] high disk watermark [90%] exceeded on [node-1]\n"
    );
}

#[test]
fn test_tz_writes_each_timestamp_with_the_offset_the_zone_has_at_it() {
    let output = run_splash_with_stdin_args(
        &[
            "--mode",
            "clf",
            "--output",
            "plain",
            "--tz",
            "America/New_York",
        ],
        concat!(
            "127.0.0.1 - - [15/Jan/2023:12:00:00 +0000] \"GET / HTTP/1.1\" 200 2\n",
            "127.0.0.1 - - [15/Jul/2023:12:00:00 +0000] \"GET / HTTP/1.1\" 200 2\n",
        ),
    )
    .unwrap();

    assert_eq!(
        output,
        concat!(
            "127.0.0.1 - - [2023-01-15T07:00:00-05:00] \"GET / HTTP/1.1\" 200 2\n",
            "127.0.0.1 - - [2023-07-15T08:00:00-04:00] \"GET / HTTP/1.1\" 200 2\n",
        )
    );
}

#[cfg(unix)]
#[test]
fn test_a_crlf_ending_split_across_two_writes_ends_one_followed_line() {
    let output = follow_file(
        &["--mode", "ad-hoc", "--output", "plain"],
        b"service started\r\nrequest served\r",
        Some(&|log: &Path| append_to(log, "\n")),
    );

    assert_eq!(output, "service started\nrequest served\n");
}

#[cfg(unix)]
#[test]
fn test_lines_from_several_clients_sending_at_once_are_all_rendered_in_each_clients_order() {
    let mut listening = Listening::start(&["--mode", "ad-hoc", "--output", "plain"]);
    let address = listening.address.clone();
    let clients = 4;
    let lines_each = 200;

    let senders: Vec<_> = (0..clients)
        .map(|client| {
            let address = address.clone();

            thread::spawn(move || {
                let mut connection = std::net::TcpStream::connect(&address).unwrap();

                for line in 0..lines_each {
                    connection
                        .write_all(format!("client {} line {}\n", client, line).as_bytes())
                        .unwrap();
                }
            })
        })
        .collect();

    for sender in senders {
        sender.join().unwrap();
    }

    while String::from_utf8_lossy(&listening.written).lines().count() < clients * lines_each {
        let before = listening.written.len();
        listening.collect();

        if listening.written.len() == before {
            break;
        }
    }

    let (success, output) = listening.interrupt();

    assert!(success);
    assert_eq!(output.lines().count(), clients * lines_each);

    for client in 0..clients {
        let prefix = format!("client {} ", client);
        let received: Vec<&str> = output
            .lines()
            .filter(|line| line.starts_with(&prefix))
            .collect();
        let sent: Vec<String> = (0..lines_each)
            .map(|line| format!("client {} line {}", client, line))
            .collect();

        assert_eq!(received, sent);
    }
}
