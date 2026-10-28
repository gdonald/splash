# Customizing splash's colors

This guide changes the colors splash paints a log in, from picking a theme to a saved scheme that
every run uses. The settings it describes are listed in the README's
[Configuration](README.md#configuration) section.

- [How a color is chosen](#how-a-color-is-chosen)
- [Find the field to recolor](#find-the-field-to-recolor)
- [Pick a theme](#pick-a-theme)
- [Recolor fields](#recolor-fields)
- [Recolor fields in one format](#recolor-fields-in-one-format)
- [Color words and patterns in messages](#color-words-and-patterns-in-messages)
- [Save a scheme as a profile](#save-a-scheme-as-a-profile)
- [Colors in HTML output](#colors-in-html-output)

---

## How a color is chosen

splash splits each line into tokens and gives each token a kind, such as `ip`, `status`, or
`timestamp`. A color scheme is a style for every kind. splash builds the scheme for a run in
layers, each one replacing the styles the one before it set:

1. The theme named by `--theme` or the config file's `theme` key, or the profile named by
   `--profile`. Without either, the `dark` theme.
2. The `[colors]` table of the config file.
3. The `[plugins.NAME]` table for the mode the input is read in.
4. Each `--color KEY=COLOR`, in the order given.

A style is a color, optionally followed by `bold`. A color is one of these names, or a hex value
such as `#ff5555`:

| Name | Also written |
| --- | --- |
| `default` | `normal` |
| `black`, `red`, `green`, `yellow`, `blue`, `cyan`, `white` | |
| `magenta` | `purple` |
| `bright black` | `gray` |
| `bright red`, `bright green`, `bright yellow`, `bright blue`, `bright cyan`, `bright white` | |
| `bright magenta` | `bright purple` |

Names are case-insensitive, and an underscore or a hyphen reads as a space, so `bright_cyan`,
`bright-cyan`, and `Bright Cyan` are the same color. `default` leaves the token in the terminal's
own text color. A hex value is written to the terminal as a 24-bit color, which the terminal must
support to show it.

## Find the field to recolor

`--output json` names the kind of every token in a line:

```bash
echo 'Oct  3 12:00:01 web01 sshd[5101]: Failed password for alice from 203.0.113.50 port 40122 ssh2' \
  | splash --mode ssh --output json
```

```json
{"text":"Oct  3 12:00:01 web01 sshd[5101]: Failed password for alice from 203.0.113.50 port 40122 ssh2","tokens":[{"kind":"timestamp","text":"Oct  3 12:00:01"},{"kind":"plain","text":" "},{"kind":"host","text":"web01"},{"kind":"plain","text":" "},{"kind":"tag","text":"sshd"},{"kind":"punctuation","text":"["},{"kind":"pid","text":"5101"},{"kind":"punctuation","text":"]"},{"kind":"punctuation","text":":"},{"kind":"plain","text":" "},{"kind":"failure","text":"Failed"},{"kind":"plain","text":" "},{"kind":"protocol","text":"password"},{"kind":"message","text":" "},{"kind":"message","text":"for"},{"kind":"plain","text":" "},{"kind":"userid","text":"alice"},{"kind":"message","text":" from "},{"kind":"ip","text":"203.0.113.50"},{"kind":"message","text":" "},{"kind":"message","text":"port"},{"kind":"plain","text":" "},{"kind":"number","text":"40122"},{"kind":"message","text":" "},{"kind":"protocol","text":"ssh2"}]}
```

Here the user name is `userid`, the address is `ip`, and `Failed` is `failure`. The
[plugin reference](PLUGINS.md#plugin-reference) lists the kinds each built-in format gives its
fields. The README's [Color overrides](README.md#color-overrides) section lists every kind.

## Pick a theme

Start from the theme closest to the scheme you want. `--list-themes` prints them:

| Theme | For |
| --- | --- |
| `dark` | Bright ANSI colors on a dark terminal. The default |
| `light` | Darker colors for a light terminal or a printed page |
| `solarized` | The Solarized Dark palette, as hex colors |
| `dracula` | The Dracula palette, as hex colors |
| `colorblind` | The Okabe-Ito palette, which stays apart for protanopia, deuteranopia, and tritanopia, with success sky blue and failure vermillion |
| `high-contrast` | Light colors on black, each at least 7:1 against the background by the WCAG contrast ratio |

Try each one on a log you read often:

```bash
splash --mode syslog --path /var/log/syslog --theme solarized
```

To use a theme on every run, set it in `~/.splash/config.toml`:

```toml
theme = "solarized"
```

## Recolor fields

`--color KEY=COLOR` changes one kind for one run, and can be repeated. Quote a style that holds a
space:

```bash
splash --mode ssh --path /var/log/auth.log --color userid="bright yellow bold" --color ip=cyan
```

The `[colors]` table of the config file makes the change on every run:

```toml
theme = "solarized"

[colors]
userid = "bright yellow bold"
ip = "cyan"
failure = "#ff5555 bold"
```

A `--color` still wins over the file for one run.

## Recolor fields in one format

The same kind can need a different color in different logs. A `[plugins.NAME]` table recolors
kinds only when the input is read in that plugin's mode, whether `--mode` names it or `--auto`
chooses it:

```toml
[colors]
pid = "bright black"

[plugins.redis]
pid = "green"
level = "bright red bold"
```

Here process ids are gray in every log except Redis logs, where they are green. The plugin name is
the one `--mode` takes, and `--list-plugins` prints the settings splash read for each plugin.

## Color words and patterns in messages

The kinds above come from the fields a format defines. Inside a message, splash also colors
keywords such as `error` and `warning` with the `failure` and `warning` styles. The config file
adds words under `[keywords]` and regular expressions under `[patterns]`, keyed by the kind whose
style they take:

```toml
[keywords]
failure = "timed out, refused"
success = "deployed"

[patterns]
transaction = "req-[0-9a-f]{8}"
```

A message holding `deployed` is now painted in the `success` style, and a request id such as
`req-3f9a1c2e` in the `transaction` style. `--no-wordcolor` turns keyword coloring off for one
run. The README's [Keywords](README.md#keywords) section lists the built-in keywords.

## Save a scheme as a profile

A profile is a named scheme stored in `~/.splash/profiles/NAME.toml`. `--save-profile NAME` saves
the scheme splash resolved from the theme, the config file, and the `--color` options, then exits:

```bash
splash --theme light --color ip="#ff5555 bold" --save-profile printout
```

```
Saved color profile 'printout' to /home/alice/.splash/profiles/printout.toml
```

The file names the theme and the style of every kind:

```toml
theme = "light"

[colors]
plain = "black"
punctuation = "bright black"
ip = "#ff5555 bold"
number = "blue"
datetime = "cyan"
```

`--profile NAME` loads it in place of a theme, and `--list-profiles` prints the saved profiles.
The file can be edited by hand. A profile keeps several schemes ready, such as one for a light
terminal and one for a dark one, where the config file holds one.

## Colors in HTML output

`--output html` writes the scheme as a style sheet at the top of the page, with a rule for each
kind, named `splash-KIND`:

```css
.splash-ip { color: #ff5555; font-weight: bold; }
```

The page's background and text colors come from the theme. A named color is written as the hex
value of its standard ANSI color, so `bright red` is `#ff5555`. `--css FILE` adds a style sheet
after the theme's rules, so its rules win over them:

```css
body { background: #fdf6e3; }
.splash-status { color: #268bd2; text-decoration: underline; }
```

```bash
splash --mode httpd --path access.log --output html --css report.css > report.html
```

HTML output takes the `[plugins.NAME]` colors of the mode `--mode` names, since the style sheet is
written before `--auto` reads the input.
