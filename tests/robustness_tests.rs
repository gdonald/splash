use splash::detect;
use std::fs;
use std::path::PathBuf;

/// A small deterministic random number generator, so a failing input is the
/// same on every run
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }
}

fn modes() -> Vec<String> {
    let mut modes = detect::candidate_modes();
    modes.extend(["ad-hoc".to_string(), "auto".to_string()]);
    modes
}

fn sample_lines() -> Vec<String> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("examples");
    let mut names: Vec<String> = fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".log"))
        .collect();
    names.sort();

    names
        .iter()
        .filter_map(|name| fs::read_to_string(directory.join(name)).ok())
        .flat_map(|text| text.lines().map(str::to_string).collect::<Vec<_>>())
        .filter(|line| !line.trim().is_empty())
        .collect()
}

const PIECES: [&str; 16] = [
    "\"", "[", "]", "{", "}", ":", "=", " ", "\t", "\u{1b}", "\u{0}", "é", "日本", "-", "\\", "%",
];

/// A sample line with characters inserted, removed, or replaced
fn mutated(random: &mut Random, line: &str) -> String {
    let mut characters: Vec<String> = line.chars().map(String::from).collect();

    for _ in 0..1 + random.below(4) {
        let at = random.below(characters.len() + 1);
        let piece = PIECES[random.below(PIECES.len())].to_string();

        match random.below(3) {
            0 => characters.insert(at, piece),
            1 if at < characters.len() => {
                characters.remove(at);
            }
            _ if at < characters.len() => characters[at] = piece,
            _ => characters.push(piece),
        }
    }

    characters.concat()
}

/// A line of random bytes, read as UTF-8 with invalid bytes replaced
fn random_bytes(random: &mut Random) -> String {
    let bytes: Vec<u8> = (0..random.below(120))
        .map(|_| random.next() as u8)
        .filter(|byte| *byte != b'\n')
        .collect();

    String::from_utf8_lossy(&bytes).into_owned()
}

fn assert_every_mode_keeps_the_line(line: &str, modes: &[String]) {
    for mode in modes {
        let mode = detect::resolve_mode(mode, line);

        if let Some(parsed) = splash::parser::parse_line(line, &mode) {
            let joined: String = parsed
                .tokens
                .iter()
                .map(|token| token.text.as_ref())
                .collect();

            assert_eq!(joined, line, "{} changed {:?}", mode, line);
        }
    }
}

#[test]
fn every_mode_keeps_every_character_of_mutated_sample_lines() {
    let modes = modes();
    let lines = sample_lines();
    let mut random = Random(0x5eed_1234_abcd_0001);

    for _ in 0..1_500 {
        let original = &lines[random.below(lines.len())];
        let line = mutated(&mut random, original);

        assert_every_mode_keeps_the_line(&line, &modes);
    }
}

#[test]
fn every_mode_keeps_every_character_of_random_bytes() {
    let modes = modes();
    let mut random = Random(0x0dd_ba11_cafe_f00d);

    for _ in 0..500 {
        let line = random_bytes(&mut random);

        assert_every_mode_keeps_the_line(&line, &modes);
    }
}

fn splash_on_file(args: &[&str], contents: &[u8]) -> std::process::Output {
    let directory = tempfile::TempDir::new().unwrap();
    let path = directory.path().join("input.log");
    fs::write(&path, contents).unwrap();

    std::process::Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .arg("--path")
        .arg(&path)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap()
}

fn splash_on_stdin(args: &[&str], contents: &[u8]) -> std::process::Output {
    use std::io::Write;

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_splash"))
        .args(args)
        .env("CLICOLOR_FORCE", "1")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.take().unwrap().write_all(contents).unwrap();
    child.wait_with_output().unwrap()
}

const OUTPUTS: [&str; 6] = ["ansi", "plain", "json", "jsonl", "csv", "html"];

const ACCESS_LINES: &str = concat!(
    "10.0.0.5 - - [03/Oct/2023:12:00:01 +0000] \"GET / HTTP/1.1\" 200 612\n",
    "10.0.0.6 - - [03/Oct/2023:12:00:02 +0000] \"GET /missing HTTP/1.1\" 404 0",
);

#[test]
fn a_last_line_without_a_line_break_is_written_like_any_other_in_every_output() {
    for output in OUTPUTS {
        let args = ["--mode", "httpd", "--output", output];
        let ended = format!("{}\n", ACCESS_LINES);

        let unended_file = splash_on_file(&args, ACCESS_LINES.as_bytes());
        let ended_file = splash_on_file(&args, ended.as_bytes());
        let unended_stdin = splash_on_stdin(&args, ACCESS_LINES.as_bytes());

        assert_eq!(unended_file.stdout, ended_file.stdout, "{}", output);
        assert_eq!(unended_stdin.stdout, ended_file.stdout, "{}", output);
    }
}

const BINARY: &[u8] = b"service started\n\x00\x01\x02 null bytes \x00 inside\nbad \xff\xfe utf-8\n\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\nservice stopped\n";

#[test]
fn binary_data_is_read_without_failing_and_keeps_every_line() {
    for output in OUTPUTS {
        let args = ["--mode", "ad-hoc", "--output", output];

        let from_file = splash_on_file(&args, BINARY);
        let from_stdin = splash_on_stdin(&args, BINARY);

        assert!(from_file.status.success(), "{}", output);
        assert!(from_stdin.status.success(), "{}", output);
        assert_eq!(from_file.stdout, from_stdin.stdout, "{}", output);
    }
}

#[test]
fn plain_output_keeps_null_bytes_and_replaces_invalid_utf_8() {
    let output = splash_on_file(&["--mode", "ad-hoc", "--output", "plain"], BINARY);
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(text.contains("\u{0}\u{1}\u{2} null bytes \u{0} inside\n"));
    assert!(text.contains("bad \u{fffd}\u{fffd} utf-8\n"));
    assert!(text.ends_with("service stopped\n"));
}

#[test]
fn json_output_escapes_null_bytes() {
    let output = splash_on_file(&["--mode", "ad-hoc", "--output", "jsonl"], BINARY);
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(text.contains(r#"\u0000\u0001\u0002 null bytes \u0000 inside"#));
    assert!(!text.contains('\u{0}'));
}

#[test]
fn ansi_output_writes_control_characters_as_text_the_terminal_shows() {
    let output = splash_on_file(
        &["--mode", "ad-hoc", "--output", "ansi", "--no-wordcolor"],
        b"user \x1b]0;owned\x07 logged \x1b[2J in\n",
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let colors = regex::Regex::new("\u{1b}\\[[0-9;]*m").unwrap();
    let shown = colors.replace_all(&text, "");

    assert_eq!(shown, "user ^[]0;owned^G logged ^[[2J in\n");
    assert!(!shown.contains('\u{1b}'));
}

#[test]
fn the_viewer_and_ansi_output_share_the_rendering_that_hides_controls() {
    let parsed = splash::output::ParsedLine::new(vec![splash::output::Token::new(
        "\u{1b}[31mred",
        splash::output::TokenKind::Plain,
    )]);

    let rendered =
        splash::output::OutputMode::Curses.render(&parsed, &splash::theme::Theme::default());

    assert!(rendered.contains("^[[31mred"), "{:?}", rendered);
}
