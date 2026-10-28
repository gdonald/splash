//! Shell completion scripts
//!
//! `--completions SHELL` writes a script completing splash's options, and
//! the values of the options that take one of a known set: modes, output
//! formats, themes, encodings, report formats, files, and directories. The
//! zsh script loads the bash function through zsh's `bashcompinit`.
use std::fmt;

/// The shells splash writes completion scripts for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

/// Error returned when `--completions` names a shell splash has no script for
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownShell(pub String);

impl fmt::Display for UnknownShell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unknown shell '{}' (expected bash, zsh, or fish){}",
            self.0,
            crate::suggest::hint(&self.0, ["bash", "zsh", "fish"])
        )
    }
}

impl std::error::Error for UnknownShell {}

impl Shell {
    pub fn parse(name: &str) -> Result<Shell, UnknownShell> {
        match name.to_ascii_lowercase().as_str() {
            "bash" => Ok(Shell::Bash),
            "zsh" => Ok(Shell::Zsh),
            "fish" => Ok(Shell::Fish),
            _ => Err(UnknownShell(name.to_string())),
        }
    }
}

/// What follows an option on the command line
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Values {
    /// Nothing: the option is a flag, or its value may be left out
    Nothing,
    /// One of these words
    Words(Vec<String>),
    Files,
    Directories,
    /// A value no list holds, such as an address
    Any,
}

/// One option of the command line
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completed {
    pub long: String,
    pub short: Option<char>,
    pub help: String,
    pub values: Values,
}

/// The completion script for `shell`
pub fn script(shell: Shell, options: &[Completed]) -> String {
    match shell {
        Shell::Bash => format!("{}\ncomplete -F _splash splash\n", bash_function(options)),
        Shell::Zsh => format!(
            "autoload -U +X bashcompinit && bashcompinit\n\n{}\ncomplete -F _splash splash\n",
            bash_function(options)
        ),
        Shell::Fish => fish(options),
    }
}

/// The names of an option, as `-m|--mode` or `--auto`
fn names(option: &Completed, separator: &str) -> String {
    match option.short {
        Some(short) => format!("-{}{}--{}", short, separator, option.long),
        None => format!("--{}", option.long),
    }
}

fn bash_function(options: &[Completed]) -> String {
    let mut cases = String::new();

    for option in options {
        let reply = match &option.values {
            Values::Nothing => continue,
            Values::Words(words) => {
                format!(
                    "COMPREPLY=($(compgen -W \"{}\" -- \"$cur\"))",
                    words.join(" ")
                )
            }
            Values::Files => "COMPREPLY=($(compgen -f -- \"$cur\"))".to_string(),
            Values::Directories => "COMPREPLY=($(compgen -d -- \"$cur\"))".to_string(),
            Values::Any => "COMPREPLY=()".to_string(),
        };

        cases.push_str(&format!(
            "        {})\n            {}\n            return\n            ;;\n",
            names(option, "|"),
            reply
        ));
    }

    let words: Vec<String> = options
        .iter()
        .flat_map(|option| {
            let short = option.short.map(|short| format!("-{}", short));

            std::iter::once(format!("--{}", option.long)).chain(short)
        })
        .collect();

    format!(
        concat!(
            "_splash() {{\n",
            "    local cur=\"${{COMP_WORDS[COMP_CWORD]}}\"\n",
            "    local prev=\"${{COMP_WORDS[COMP_CWORD-1]}}\"\n",
            "\n",
            "    case \"$prev\" in\n",
            "{}",
            "    esac\n",
            "\n",
            "    COMPREPLY=($(compgen -W \"{}\" -- \"$cur\"))\n",
            "}}\n",
        ),
        cases,
        words.join(" ")
    )
}

fn fish(options: &[Completed]) -> String {
    let mut script = String::from("complete -c splash -f\n");

    for option in options {
        let short = option
            .short
            .map(|short| format!(" -s {}", short))
            .unwrap_or_default();
        let values = match &option.values {
            Values::Nothing => String::new(),
            Values::Words(words) => format!(" -x -a '{}'", words.join(" ")),
            Values::Files => " -r -F".to_string(),
            Values::Directories => " -x -a '(__fish_complete_directories)'".to_string(),
            Values::Any => " -x".to_string(),
        };

        script.push_str(&format!(
            "complete -c splash{} -l {} -d '{}'{}\n",
            short,
            option.long,
            short_help(&option.help)
                .replace('\\', "\\\\")
                .replace('\'', "\\'"),
            values
        ));
    }

    script
}

/// An option's help up to a list in parentheses, which fish has no room for
fn short_help(help: &str) -> &str {
    help.split(" (").next().unwrap_or(help)
}
