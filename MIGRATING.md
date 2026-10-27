# Moving from ccze to splash

This guide translates ccze commands, options, and `~/.cczerc` colors into splash's. The options
splash adds are described in the [README](README.md).

- [Run splash where ccze ran](#run-splash-where-ccze-ran)
- [Options](#options)
- [Plugins](#plugins)
- [Colors](#colors)
- [What splash does not do](#what-splash-does-not-do)

---

## Run splash where ccze ran

ccze reads stdin and tries each of its plugins on every line. `--auto` does the same in splash:

```bash
ccze -A < /var/log/syslog
splash --auto < /var/log/syslog
```

`--path` reads files, and `--follow` keeps reading them as they grow, in place of `tail -f`:

```bash
tail -f /var/log/syslog | ccze -A
splash --auto --follow --path /var/log/syslog
```

ccze's default output is a curses screen, and splash's is ANSI escape sequences. `--output curses`
opens splash's scrollable viewer. ccze's `-A` writes escape sequences to a pipe, and splash writes
them only to a terminal unless `CLICOLOR_FORCE=1` is set:

```bash
ccze -A < /var/log/syslog | less -R
CLICOLOR_FORCE=1 splash --auto < /var/log/syslog | less -R
```

## Options

Four short options mean something else in splash. ccze's `-m` chooses the output, where splash's
chooses the log format. ccze's `-p` chooses a plugin, where splash's names a file. `-h` prints
splash's help, and `-r` reads a directory's subdirectories.

| ccze | splash |
| --- | --- |
| `-A`, `--raw-ansi` | The default output, with `CLICOLOR_FORCE=1` when stdout is not a terminal |
| `-m curses` | `--output curses` |
| `-m ansi` | `--output ansi` |
| `-h`, `--html`, `-m html` | `--output html` |
| `-p PLUGIN`, `--plugin PLUGIN` | `--mode PLUGIN`, `-m PLUGIN`, or `--plugin PLUGIN` |
| no `-p` | `--auto` |
| `-l`, `--list-plugins` | `--list-plugins` |
| `-c KEY=COLOR`, `--color KEY=COLOR` | `--color KIND=COLOR`, with the key renamed as in [Colors](#colors) |
| `-F FILE`, `--rcfile FILE` | `--config FILE`, written in splash's format |
| `-C`, `--convert-date` | `--convert-date` |
| `-o lookups` | `--lookups` |
| `-o nolookups` | The default |
| `-o wordcolor` | The default |
| `-o nowordcolor` | `--no-wordcolor` |
| `-o cssfile` | `--css FILE` adds a style sheet after the one splash writes into the page |

ccze looks up names by default and splash does not. `lookups = true` under `[lookups]` in
`~/.splash/config.toml` turns them on for every run.

## Plugins

Each ccze plugin has a splash mode of the same name: `apm`, `distcc`, `exim`, `fetchmail`,
`ftpstats`, `httpd`, `icecast`, `oops`, `php`, `postfix`, `procmail`, `proftpd`, `squid`, `sulog`,
`super`, `syslog`, `ulogd`, `vsftpd`, and `xferlog`.

```bash
ccze -A -p squid < /var/log/squid/access.log
splash --mode squid --path /var/log/squid/access.log
```

ccze's `oops` plugin reads the log of the Oops web proxy. splash's `oops` mode reads Linux kernel
oopses, and splash has no mode for the proxy's log.

## Colors

ccze colors a keyword, and splash colors a token kind. `--output json` prints the kind of each
token in a line, as [COLORS.md](COLORS.md#find-the-field-to-recolor) shows. These kinds take the
fields ccze's keywords color:

| ccze keyword | splash kinds |
| --- | --- |
| `date` | `timestamp`, `datetime` |
| `host` | `host`, `ip`, `client` |
| `pid` | `pid` |
| `process` | `tag` |
| `get`, `post`, `head`, `put`, `connect`, `trace` | `method`, `http_verb` |
| `httpcodes` | `status` |
| `getsize`, `size` | `size` |
| `gettime` | `duration` |
| `uri` | `request` |
| `prot` | `protocol` |
| `user` | `userid` |
| `email` | `email` |
| `dir`, `file` | `path` |
| `service` | `service` |
| `error` | `failure` |
| `warning` | `warning` |
| `good` | `success` |
| `keyword` | `keyword` |
| `miss` | `cache_miss` |
| `hit` | `cache_hit` |
| `parent`, `direct` | `hierarchy` |
| `ctype` | `content_type` |

A cczerc line names a keyword, then a color with `bold` before it. A splash style names the color,
then `bold` after it, under the `[colors]` table of `~/.splash/config.toml`. This `~/.cczerc`:

```
date cyan
host bold blue
error bold red
user bold yellow
```

becomes:

```toml
[colors]
timestamp = "cyan"
host = "blue bold"
ip = "blue bold"
failure = "red bold"
userid = "yellow bold"
```

splash's styles also take `bright` colors and hex values such as `#ff5555`, as
[COLORS.md](COLORS.md) describes.

## What splash does not do

- splash does not read `/etc/cczerc`, `~/.cczerc`, or `~/.colorizerc`.
- A style is a foreground color and `bold`. `underline`, `reverse`, `blink`, and background colors
  such as `on_white` are not accepted, and splash exits with an error naming the color.
- A style cannot copy another keyword's color, as cczerc's `'keyword'` does.
- There is no option to treat black backgrounds as transparent, as `-o transparent` does, or to
  turn off scrolling, as `-o noscroll` does.
- There is no option to remove the syslog-ng facility before each message, as `-r` does.
- There is no option to pass arguments to a plugin, as `-a` does.
