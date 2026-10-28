use splash::manual::{man_page, ManOption};

fn option(names: &str, value: Option<&str>, help: &str) -> ManOption {
    ManOption {
        names: names.to_string(),
        value: value.map(str::to_string),
        help: help.to_string(),
    }
}

fn page() -> String {
    man_page(
        "1.2.3",
        "2026-10-27",
        &[
            option("-m, --mode", Some("MODE"), "Log Parsing Mode"),
            option("--auto", None, ".dotted help with a C:\\path"),
            option("--graph", Some("[PERIOD]"), &"word ".repeat(30)),
        ],
    )
}

#[test]
fn the_page_is_titled_and_dated_with_the_version() {
    assert!(page().starts_with(concat!(
        ".TH SPLASH 1 2026-10-27 \"splash 1.2.3\" \"User Commands\"\n",
        ".SH NAME\n",
        "splash \\- color log files and streams\n",
        ".SH SYNOPSIS\n",
        ".B splash\n",
        "[\\fIOPTIONS\\fR]\n",
        ".SH DESCRIPTION\n",
    )));
}

#[test]
fn each_option_is_listed_with_its_value_and_help() {
    assert!(page().contains(concat!(
        ".SH OPTIONS\n",
        ".TP\n",
        "\\fB\\-m, \\-\\-mode\\fR \\fIMODE\\fR\n",
        "Log Parsing Mode\n",
        ".TP\n",
        "\\fB\\-\\-auto\\fR\n",
        "\\&.dotted help with a C:\\epath\n",
        ".TP\n",
        "\\fB\\-\\-graph\\fR \\fI[PERIOD]\\fR\n",
    )));
}

#[test]
fn text_is_wrapped_at_eighty_bytes() {
    let page = page();

    assert_eq!(
        page.lines().filter(|line| line.len() > 80).count(),
        0,
        "{}",
        page
    );
    assert!(page.contains(&format!(
        "{}\n{}\n",
        "word ".repeat(16).trim_end(),
        "word ".repeat(14).trim_end()
    )));
}

#[test]
fn the_page_describes_the_environment_files_and_examples() {
    let page = page();

    assert_eq!(
        [
            ".SH ENVIRONMENT\n.TP\n.B CLICOLOR_FORCE\n",
            ".TP\n.B NO_COLOR\nSet, no colors are written.\n",
            ".SH FILES\n.TP\n.I ~/.splash/config.toml\n",
            ".SH EXAMPLES\n.TP\n.B splash \\-\\-auto \\-\\-path /var/log/syslog\n",
        ]
        .map(|section| page.contains(section)),
        [true; 4]
    );
}
