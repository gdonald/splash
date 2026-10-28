//! The man page
//!
//! `--man` writes splash's manual page in roff, from the same option list
//! `--help` prints, with the environment variables and files splash reads.

/// One option as the manual lists it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManOption {
    /// The option's names, such as `-m, --mode`
    pub names: String,
    /// The name of its value, such as `MODE` or `[PERIOD]`, if it takes one
    pub value: Option<String>,
    pub help: String,
}

/// The variables the ENVIRONMENT section describes
const ENVIRONMENT: [(&str, &str); 7] = [
    (
        "CLICOLOR_FORCE",
        "Set to anything but 0, colors are written to a pipe or a file as well as to a terminal.",
    ),
    ("NO_COLOR", "Set, no colors are written."),
    (
        "TERM",
        "dumb writes no colors, and --output curses exits with an error.",
    ),
    ("CLICOLOR", "Set to 0, no colors are written."),
    (
        "COLORTERM",
        "truecolor or 24bit writes hex colors as they are. Otherwise each is written as the nearest of the 16 ANSI colors.",
    ),
    (
        "LC_ALL, LC_CTYPE, LANG",
        "The charset of the first that is set chooses the encoding when --encoding is not given: ISO-8859-1 is Latin-1, and any other charset is UTF-8.",
    ),
    (
        "AWS_PROFILE, AWS_REGION",
        "Choose the credentials and region --cloudwatch reads with, as the AWS CLI does.",
    ),
];

/// The files the FILES section describes
const FILES: [(&str, &str); 4] = [
    (
        "~/.splash/config.toml",
        "The config file, read in place of ~/.splashrc when both exist.",
    ),
    (
        "~/.splashrc",
        "The config file splash reads when ~/.splash/config.toml does not exist.",
    ),
    ("~/.splash/profiles/", "Saved color profiles."),
    (
        "~/.splash/plugins/, /usr/local/lib/splash/plugins/, /usr/lib/splash/plugins/",
        "Plugin files splash loads, along with those in each --plugin-dir.",
    ),
];

/// The examples the EXAMPLES section shows, each with what it does
const EXAMPLES: [(&str, &str); 5] = [
    (
        "splash --auto --path /var/log/syslog",
        "Colors a log in the format detected from its first lines.",
    ),
    (
        "tail -f /var/log/nginx/access.log | splash --mode httpd",
        "Colors lines as they arrive on stdin.",
    ),
    (
        "splash --mode syslog --path /var/log/syslog --follow --level error",
        "Follows a log, keeping the error lines.",
    ),
    (
        "splash --mode clf --path access.log --output html > access.html",
        "Writes a log as an HTML page.",
    ),
    (
        "splash --diff --path good.log failed.log",
        "Compares two runs, leaving timestamps and IDs out.",
    ),
];

/// The longest text line the page holds, as `mandoc -T lint` asks
const LINE_WIDTH: usize = 80;

/// The manual page for this `version` of splash, dated `date`, such as
/// `2026-10-27`
pub fn man_page(version: &str, date: &str, options: &[ManOption]) -> String {
    let mut page = format!(
        concat!(
            ".TH SPLASH 1 {} \"splash {}\" \"User Commands\"\n",
            ".SH NAME\n",
            "splash \\- color log files and streams\n",
            ".SH SYNOPSIS\n",
            ".B splash\n",
            "[\\fIOPTIONS\\fR]\n",
            ".SH DESCRIPTION\n",
            "{}\n",
            ".SH OPTIONS\n",
        ),
        date,
        escape(version),
        line(concat!(
            "splash reads log lines from files, stdin, the network, or CloudWatch Logs, ",
            "and writes them with each field colored by its kind: ",
            "as ANSI colors, in a scrollable viewer, or as HTML, JSON, JSON Lines, CSV, or plain text."
        ))
    );

    for option in options {
        let value = option
            .value
            .as_ref()
            .map(|value| format!(" \\fI{}\\fR", escape(value)))
            .unwrap_or_default();

        page.push_str(&format!(
            ".TP\n\\fB{}\\fR{}\n{}\n",
            escape(&option.names),
            value,
            line(&option.help)
        ));
    }

    page.push_str(".SH ENVIRONMENT\n");

    for (name, effect) in ENVIRONMENT {
        page.push_str(&format!(".TP\n.B {}\n{}\n", escape(name), line(effect)));
    }

    page.push_str(".SH FILES\n");

    for (path, holds) in FILES {
        page.push_str(&format!(".TP\n.I {}\n{}\n", escape(path), line(holds)));
    }

    page.push_str(".SH EXAMPLES\n");

    for (command, does) in EXAMPLES {
        page.push_str(&format!(".TP\n.B {}\n{}\n", escape(command), line(does)));
    }

    page
}

/// Text with roff's special characters escaped: a backslash as `\e` and
/// each hyphen as `\-`, so it reads as a minus sign and is not broken
fn escape(text: &str) -> String {
    text.replace('\\', "\\e").replace('-', "\\-")
}

/// Text escaped and wrapped into lines of at most `LINE_WIDTH` bytes, each
/// starting with `\&` where a leading `.` or `'` would be read as a request
fn line(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();

    for word in escape(text).split_whitespace() {
        match lines.last_mut() {
            Some(last) if last.len() + 1 + word.len() <= LINE_WIDTH => {
                last.push(' ');
                last.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }

    lines
        .into_iter()
        .map(|line| match line.starts_with(['.', '\'']) {
            true => format!("\\&{}", line),
            false => line,
        })
        .collect::<Vec<String>>()
        .join("\n")
}
