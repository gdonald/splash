use splash::completions::{script, Completed, Shell, UnknownShell, Values};

fn options() -> Vec<Completed> {
    let option = |long: &str, short: Option<char>, help: &str, values: Values| Completed {
        long: long.to_string(),
        short,
        help: help.to_string(),
        values,
    };

    vec![
        option(
            "output",
            Some('o'),
            "Output format (ansi, plain)",
            Values::Words(vec!["ansi".to_string(), "plain".to_string()]),
        ),
        option("auto", None, "Detect the log's format", Values::Nothing),
        option("path", Some('p'), "Paths to the log files", Values::Files),
        option(
            "plugin-dir",
            None,
            "Plugin files in C:\\plugins",
            Values::Directories,
        ),
        option("listen", None, "Receive lines on this address", Values::Any),
    ]
}

#[test]
fn shells_are_named_in_any_case() {
    assert_eq!(
        ["bash", "ZSH", "Fish"].map(|name| Shell::parse(name).unwrap()),
        [Shell::Bash, Shell::Zsh, Shell::Fish]
    );
}

#[test]
fn an_unknown_shell_is_refused() {
    let error = Shell::parse("tcsh").unwrap_err();

    assert_eq!(
        (error.clone(), error.to_string()),
        (
            UnknownShell("tcsh".to_string()),
            "Unknown shell 'tcsh' (expected bash, zsh, or fish)".to_string()
        )
    );
}

const BASH_FUNCTION: &str = r#"_splash() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local prev="${COMP_WORDS[COMP_CWORD-1]}"

    case "$prev" in
        -o|--output)
            COMPREPLY=($(compgen -W "ansi plain" -- "$cur"))
            return
            ;;
        -p|--path)
            COMPREPLY=($(compgen -f -- "$cur"))
            return
            ;;
        --plugin-dir)
            COMPREPLY=($(compgen -d -- "$cur"))
            return
            ;;
        --listen)
            COMPREPLY=()
            return
            ;;
    esac

    COMPREPLY=($(compgen -W "--output -o --auto --path -p --plugin-dir --listen" -- "$cur"))
}
"#;

#[test]
fn the_bash_script_completes_options_and_their_values() {
    assert_eq!(
        script(Shell::Bash, &options()),
        format!("{}\ncomplete -F _splash splash\n", BASH_FUNCTION)
    );
}

#[test]
fn the_zsh_script_loads_the_bash_function_through_bashcompinit() {
    assert_eq!(
        script(Shell::Zsh, &options()),
        format!(
            "autoload -U +X bashcompinit && bashcompinit\n\n{}\ncomplete -F _splash splash\n",
            BASH_FUNCTION
        )
    );
}

#[test]
fn the_fish_script_describes_each_option_and_completes_its_values() {
    assert_eq!(
        script(Shell::Fish, &options()),
        concat!(
            "complete -c splash -f\n",
            "complete -c splash -s o -l output -d 'Output format' -x -a 'ansi plain'\n",
            "complete -c splash -l auto -d 'Detect the log\\'s format'\n",
            "complete -c splash -s p -l path -d 'Paths to the log files' -r -F\n",
            "complete -c splash -l plugin-dir -d 'Plugin files in C:\\\\plugins' -x -a '(__fish_complete_directories)'\n",
            "complete -c splash -l listen -d 'Receive lines on this address' -x\n",
        )
    );
}
