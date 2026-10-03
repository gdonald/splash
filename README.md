# splash

A fast, modern log colorizer built in Rust.

###

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/gdonald/splash/blob/main/LICENSE) [![CI](https://github.com/gdonald/splash/workflows/CI/badge.svg)](https://github.com/gdonald/splash/actions) [![codecov](https://codecov.io/gh/gdonald/splash/graph/badge.svg?token=GQ4LA1VMRE)](https://codecov.io/gh/gdonald/splash)

## Quick Start

```bash
# Install
cargo install --path .

# Colorize a log file (Common Log Format)
splash --mode clf --path /var/log/apache2/access.log

# Colorize Apache and nginx access and error logs
splash --mode httpd --path /var/log/nginx/error.log

# Colorize a Squid access log, marking cache hits and misses
splash --mode squid --path /var/log/squid/access.log

# Colorize a Varnish transaction log
varnishlog | splash --mode varnish

# Colorize a HAProxy log
splash --mode haproxy --path /var/log/haproxy.log

# Colorize Caddy's structured JSON logs
splash --mode caddy --path /var/log/caddy/access.log

# Colorize mail server logs
splash --mode postfix --path /var/log/mail.log
splash --mode exim --path /var/log/exim4/mainlog
splash --mode dovecot --path /var/log/dovecot.log
splash --mode fetchmail --path ~/.fetchmail.log
splash --mode procmail --path ~/.procmail.log

# Colorize FTP server logs
splash --mode vsftpd --path /var/log/vsftpd.log
splash --mode proftpd --path /var/log/proftpd/proftpd.log
splash --mode pure-ftpd --path /var/log/messages
splash --mode xferlog --path /var/log/xferlog
splash --mode ftpstats --path /var/log/pureftpd.log

# Colorize system logs
splash --mode syslog --path /var/log/syslog
journalctl -b | splash --mode journalctl
dmesg | splash --mode dmesg
splash --mode auth --path /var/log/auth.log
splash --mode cron --path /var/log/cron
splash --mode ulogd --path /var/log/ulog/syslogemu.log

# Colorize application and database logs
splash --mode php --path /var/log/php_errors.log
splash --mode apache-error --path /var/log/apache2/error.log
splash --mode mysql --path /var/log/mysql/error.log
splash --mode postgresql --path /var/log/postgresql/postgresql-16-main.log
splash --mode redis --path /var/log/redis/redis-server.log
splash --mode mongodb --path /var/log/mongodb/mongod.log
splash --mode elasticsearch --path /var/log/elasticsearch/es-prod.log

# Colorize specialized service logs
splash --mode ssh --path /var/log/auth.log
splash --mode sudo --path /var/log/sudo.log
splash --mode super --path /var/log/super.log
splash --mode sulog --path /var/adm/sulog
splash --mode distcc --path /var/log/distccd.log
splash --mode icecast --path /var/log/icecast2/error.log
splash --mode apm --path /var/log/syslog
dmesg | splash --mode oops

# Colorize container, infrastructure, and CI logs
docker logs --timestamps web | splash --mode docker
kubectl logs -n kube-system kube-scheduler-node01 | splash --mode kubernetes
journalctl -u systemd-resolved | splash --mode systemd-resolved
splash --mode nginx-error --path /var/log/nginx/error.log
splash --mode git --path /var/log/git-daemon.log
splash --mode ci --path build.log
splash --mode cloud-init --path /var/log/cloud-init.log
terraform apply 2>&1 | splash --mode terraform

# Colorize with ad-hoc mode (auto-detects patterns)
splash --mode ad-hoc --path /var/log/syslog

# Pipe from stdin
tail -f /var/log/nginx/access.log | splash --mode clf

# Or just
cat logfile.log | splash
```

---

## Usage

```
Usage: splash [OPTIONS]

Options:
  -m, --mode <MODE>                      Log Parsing Mode (apache-error, apm, auth, caddy, ci, clf,
                                         cloud-init, cron, distcc, dmesg, docker, dovecot,
                                         elasticsearch, exim, fetchmail, ftpstats, git, haproxy,
                                         httpd, icecast, journalctl, kubernetes, mongodb, mysql,
                                         nginx-error, oops, php, postfix, postgresql, procmail,
                                         proftpd, pure-ftpd, redis, squid, ssh, sudo, sulog, super,
                                         syslog, systemd-resolved, terraform, ulogd, varnish, vsftpd,
                                         xferlog, ad-hoc)
  -p, --path <PATH>                      Path to the log file
  -o, --output <OUTPUT>                  Output format (ansi, curses, html, json, plain)
  -j, --jobs <JOBS>                      Worker threads used to render a file (default: one per
                                         core)
      --config <CONFIG>                  Config file to read instead of ~/.splash/config.toml or
                                         ~/.splashrc
      --theme <THEME>                    Color theme (dark, light, solarized, dracula)
      --color <KEY=COLOR>                Override one token color, as KEY=COLOR (repeatable)
      --profile <PROFILE>                Load a saved color profile
      --save-profile <NAME>              Save the resolved colors as a profile and exit
      --list-profiles                    List saved color profiles
      --list-themes                      List available color themes
      --list-plugins                     List all available plugins
      --plugin <PLUGIN>                  Use a specific plugin by name
      --disable-plugin <DISABLE_PLUGIN>  Disable a specific plugin by name
  -h, --help                             Print help
  -V, --version                          Print version
```

---

## Configuration

splash reads `~/.splash/config.toml`, falling back to `~/.splashrc`. Both use the same format.
`--config PATH` reads a different file instead.

```toml
# Defaults used when the command line does not say otherwise
mode = "clf"
output = "ansi"
theme = "dracula"
jobs = "4"

# Repaint individual fields
[colors]
ip = "bright cyan"
status = "white bold"

# Settings for one plugin
[plugins.syslog]
enabled = "true"
facility = "cyan"
```

Command line options win over the config file. Colors are layered: the theme or profile first,
then the file's `[colors]` table, then any `--color` overrides.

### Themes

`--theme NAME` selects one of the presets `dark` (the default), `light`, `solarized`, and
`dracula`. `--list-themes` prints them. A theme sets a color for every token kind and the page
colors used by HTML output.

```bash
splash --mode clf --path access.log --theme solarized
```

### Color overrides

`--color KEY=COLOR` repaints one token kind. The key is the kind name used in JSON output and in
HTML class names: `plain`, `punctuation`, `ip`, `number`, `datetime`, `tz_offset`, `http_verb`,
`http_version`, `client`, `user_identifier`, `userid`, `timestamp`, `method`, `request`,
`protocol`, `status`, `size`, `referer`, `user_agent`, `vhost`, `duration`, `level`,
`module`, `pid`, `message`, `cache_hit`, `cache_miss`, `cache_result`, `hierarchy`, and
`content_type`, `transaction`, `tag`, `header`, `host`, `frontend`, `backend`, `server`,
`timers`, `termination`, and `counters`.

A color is one of the sixteen ANSI color names (`red`, `bright cyan`, `gray`), a hex value
(`#ff5555`), or either of those followed by `bold`.

```bash
splash --mode clf --path access.log --color ip=bright_cyan --color status="white bold"
```

### Color profiles

`--save-profile NAME` writes the colors splash resolved to `~/.splash/profiles/NAME.toml`, and
`--profile NAME` loads them back. `--list-profiles` prints the saved profiles.

```bash
splash --theme solarized --color ip=green --save-profile work
splash --mode clf --path access.log --profile work
```

A profile is a config file holding a `theme` key and a `[colors]` table, so it can also be edited
by hand.

### Per-plugin settings

A `[plugins.NAME]` table holds settings for one plugin. `enabled = "false"` turns a plugin off.
`--list-plugins` prints each configured plugin and its settings.

---

## Modes

### Common Log Format (CLF)

Parses and colorizes logs in the [Common Log Format](https://en.wikipedia.org/wiki/Common_Log_Format) used by Apache, nginx, and other web servers.

**Example:**
```bash
splash --mode clf --path /var/log/apache2/access.log
```

**Format:**
```
127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326
```

**Note:** Nothing will be shown if the log file is not actually formatted in CLF format. Use ad-hoc mode if you are unsure.

### Apache and nginx (httpd)

Reads the four shapes an Apache or nginx log takes. Each line is tried against them in turn and the
first that fits wins, so an access log and an error log can be colorized with the same mode.

**Example:**
```bash
splash --mode httpd --path /var/log/apache2/access.log
splash --mode httpd --path /var/log/nginx/error.log
```

**Combined access log**, the Common Log Format plus a quoted referer and user agent:
```
127.0.0.1 - frank [10/Oct/2000:13:55:36 -0700] "GET /apache_pb.gif HTTP/1.0" 200 2326 "http://www.example.com/start.html" "Mozilla/5.0"
```

**vhost combined access log**, which prefixes the server name:
```
example.com:80 10.0.0.42 - - [10/Oct/2000:13:55:37 -0700] "GET / HTTP/1.1" 200 512 "-" "curl/8.4.0"
```

**Extended access log**, whose trailing fields follow the user agent. A field of digits alone is
the microseconds the server spent on the request, colored as `duration`:
```
127.0.0.1 - - [10/Oct/2000:13:55:36 -0700] "GET /index.html HTTP/1.1" 200 4512 "-" "Mozilla/5.0" 1532
```

**Apache error log**, whose module, level, process, and client are each colored separately:
```
[Wed Oct 11 14:32:52.123456 2023] [core:error] [pid 35708:tid 4328636416] [client 72.15.99.187:1234] AH00128: File does not exist
```

**nginx error log**:
```
2023/10/11 14:32:52 [error] 1234#0: *1 open() failed (2: No such file or directory), client: 10.0.0.9
```

A plain Common Log Format line is read too, and addresses inside an error message are colored. A
line in none of these formats is dropped.

### Squid

Reads a Squid access log in either shape it is written in, and colors the result code by whether
Squid served the request from its cache. A code ending in `_HIT` is a cache hit, one ending in
`_MISS` is a cache miss, and anything else, such as `TCP_DENIED`, gets a neutral style.

**Example:**
```bash
splash --mode squid --path /var/log/squid/access.log
```

**Native format**, which leads with a Unix timestamp and pads its fields to fixed widths:
```
1614729600.123    123 10.0.0.5 TCP_MISS/200 4512 GET http://example.com/ - HIER_DIRECT/93.184.216.34 text/html
```

**httpd-emulated format**, written when `emulate_httpd_log` is on, which appends the result code
and the hierarchy code to a Common Log Format line:
```
10.0.0.5 - - [09/Mar/2021:12:00:00 +0000] "GET http://example.com/ HTTP/1.1" 200 4512 TCP_MISS:HIER_DIRECT
```

The padding between native fields is kept, so plain output reproduces the file. A line in neither
format is dropped.

### Varnish

Reads what `varnishlog` writes. A transaction header names the transaction and its id, and the
records under it are indented with one dash per level of nesting. Each record's payload is colored
by the tag that introduces it, so a `ReqMethod` payload is painted as a method and a `RespStatus`
payload as a status. A tag splash has no field for keeps its payload as message text, with any
addresses inside it colored.

**Example:**
```bash
varnishlog | splash --mode varnish
varnishlog -g raw | splash --mode varnish
```

**Transaction format:**
```
*   << Request  >> 32770
-   ReqMethod      GET
-   ReqURL         /index.html
-   ReqHeader      Host: example.com
-   RespStatus     200
-   End
```

**Raw format**, written by `varnishlog -g raw`, which leads each record with its transaction id and
the side it came from, `c` for the client and `b` for the backend:
```
        32770 ReqMethod      c GET
        32771 BerespStatus   b 503
```

The padding varnishlog uses to align its tags is kept, so plain output reproduces the stream. A
line in neither format is dropped.

### HAProxy

Reads the three kinds of line HAProxy writes, with or without the syslog header that names the
host and the process. A line logged straight to stdout, as it is in a container, is read the same
way.

**Example:**
```bash
splash --mode haproxy --path /var/log/haproxy.log
```

**`option httplog`**, which adds the status, the captured cookies, and the quoted request to the
connection fields:
```
Feb  6 12:14:14 gateway haproxy[14389]: 10.0.1.2:33317 [06/Feb/2009:12:14:14.655] http-in static/srv1 10/0/30/69/109 200 2750 - - ---- 1/1/1/1/0 0/0 "GET /index.html HTTP/1.1"
```

**`option tcplog`**, which stops at the connection counters:
```
Feb  6 12:12:56 gateway haproxy[14387]: 10.0.1.2:33313 [06/Feb/2009:12:12:51.443] fnt bck/srv1 0/0/5007 212 -- 0/0/0/0/3 0/0
```

**Error lines**, which name the listener that refused the connection and then say what went wrong:
```
Feb  6 12:12:56 gateway haproxy[14387]: 127.0.0.1:34550 [06/Feb/2009:12:12:51.443] frt/f1: invalid request
```

The frontend, the backend, and the server each get their own color, as do the timers, the
termination state, and the connection and queue counters. Captured headers before the quoted
request are kept as message text. A line in none of the three formats is dropped.

### Caddy

Reads the JSON objects Caddy's structured encoder writes, one per line. The line is scanned as
JSON rather than matched against a pattern, so a field is colored wherever it sits in the object,
including inside the nested `request`, `headers`, and `tls` objects.

**Example:**
```bash
splash --mode caddy --path /var/log/caddy/access.log
```

**Format:**
```json
{"level":"info","ts":1646861401.52,"logger":"http.log.access","msg":"handled request","request":{"remote_ip":"127.0.0.1","proto":"HTTP/2.0","method":"GET","host":"localhost","uri":"/"},"duration":0.0009,"size":10900,"status":200}
```

Every key is colored as a key, and a value takes its color from the key above it: `level`,
`ts`, `logger`, `msg`, `error`, `remote_ip`, `client_ip`, `proto`, `method`, `host`, `uri`,
`status`, `size`, `bytes_read`, `duration`, and `user_id`. A key splash has no field for keeps its
value in the default style for that value's type, so nothing Caddy adds goes uncolored. Spacing
between the pieces of the object is kept, so plain output reproduces the file. A line that is not
a single well formed JSON object is dropped.

### Mail servers

The five mail modes share how they read a message. A `key=value` field has its key colored as a
`header` and its value colored by the key: `from=` and `to=` are `email`, `relay=` and `H=` are
`host` with the bracketed address after the name colored as `ip`, `size=` is `size`, `delay=` is
`duration`, and `session=` and `id=` are `transaction`. A value in quotes or angle brackets keeps
them as punctuation. An address in angle brackets, or standing alone, is colored as `email`.

Whether the mail went through is colored with three styles: `success`, `warning`, and `failure`.
Each mode applies them to the words its program uses, listed below. A field splash has no style
for keeps its value as message text, with any IP addresses in it colored. A line the mode does not
recognize is dropped.

A syslog header, in any of the forms the syslog mode reads, is colored as a timestamp, a host, a
program, and a process id. The Postfix, Exim, Dovecot, and Fetchmail modes
only read syslog lines logged by their own program.

#### Postfix

Reads the lines every Postfix daemon sends to syslog. The queue id is colored as `queue_id`, in
the short hexadecimal form, the long form `enable_long_queue_ids` turns on, or `NOQUEUE`.
`status=sent` is a success, `status=deferred` a warning, and `status=bounced` or `expired` a
failure. A leading `warning:` or `hold:` is a warning, and `reject:`, `discard:`,
`milter-reject:`, `error:`, `fatal:`, and `panic:` are failures. In the message, `removed` is a
success, `lost connection` and `timeout` are warnings, and `Relay access denied`,
`Recipient address rejected`, `Sender address rejected`, and `authentication failed` are failures.

**Example:**
```bash
splash --mode postfix --path /var/log/mail.log
```

```
Oct  3 12:00:02 mail postfix/smtp[1236]: 4F2A1C0123: to=<bob@example.org>, relay=mx.example.org[93.184.216.34]:25, delay=0.52, delays=0.1/0/0.2/0.22, dsn=2.0.0, status=sent (250 2.0.0 OK)
Oct  3 12:05:12 mail postfix/smtpd[1243]: NOQUEUE: reject: RCPT from unknown[192.0.2.10]: 554 5.7.1 <erin@example.org>: Relay access denied; from=<spam@example.biz> to=<erin@example.org> proto=ESMTP helo=<example.biz>
```

#### Exim

Reads Exim's main log, and the same lines sent to syslog. The message id is colored as
`queue_id`, in the form Exim wrote before 4.97 and the longer one it writes since. The flag after
it is colored by what it says: `<=`, `=>`, and `->` are successes, `*>` and `==` are warnings, and
`**` is a failure. The router `R=` and the transport `T=` are colored as `module`. `Completed` is
a success, `Frozen` and `retry time not reached` are warnings, and `rejected` and `SMTP error` are
failures.

**Example:**
```bash
splash --mode exim --path /var/log/exim4/mainlog
```

```
2023-10-03 12:00:01 1qnXYZ-000ABC-12 <= alice@example.com H=mail.example.com [10.0.0.5] P=esmtps S=4512 id=20231003120001.abc@example.com
2023-10-03 12:00:02 1qnXYZ-000ABC-12 => bob@example.org R=dnslookup T=remote_smtp H=mx.example.org [93.184.216.34] C="250 2.0.0 OK"
2023-10-03 12:05:11 1qnXZb-000ABE-4G ** erin@example.com R=dnslookup T=remote_smtp H=mx.example.com [198.51.100.4]: SMTP error from remote mail server after RCPT TO:<erin@example.com>: 550 5.1.1 User unknown
```

A timestamp with milliseconds and a time zone, and the process id `log_selector = +pid` adds, are
read too.

#### Dovecot

Reads Dovecot's lines from syslog and from its own log file. The service is colored as `module`,
the user in parentheses after it as `userid` (or `pid` when it is a process id), and the ids in
angle brackets as `pid` and then `transaction`. A level such as `Info:` or `Error:` is colored as
`level`. `Login` and `saved mail to` are successes, and `auth failed`, `Aborted login`,
`Password mismatch`, `unknown user`, and `Quota exceeded` are failures.

**Example:**
```bash
splash --mode dovecot --path /var/log/dovecot.log
```

```
Oct  3 12:00:01 mail dovecot: imap-login: Login: user=<alice>, method=PLAIN, rip=10.0.0.5, lip=10.0.0.1, mpid=4321, TLS, session=<Xy7AbC>
Oct 03 12:00:05 imap(alice)<4321><Xy7AbC>: Info: Disconnected: Logged out in=1024 out=65536
```

#### Fetchmail

Reads fetchmail's lines from syslog and from its own log file, where they carry a `fetchmail: `
prefix, including the per-message `reading message` and `skipping message` lines that carry no
prefix. The user and server a poll was for are colored as `userid` and `host`, a size in octets
as `size`, counts and version numbers as `number`, and a date as `timestamp`. `flushed` and
`SUCCESS` are successes, `not flushed`, `skipped`, `timeout`, and `LOCKBUSY` are warnings, and
`Authorization failure`, `AUTHFAIL`, `SOCKET`, `PROTOCOL`, and `error` are failures.

**Example:**
```bash
splash --mode fetchmail --path ~/.fetchmail.log
```

```
fetchmail: 3 messages (1 seen) for alice at mail.example.com (12345 octets).
reading message alice@mail.example.com:1 of 3 (4096 octets) flushed
Oct  3 12:00:03 laptop fetchmail[2200]: Query status=3 (AUTHFAIL)
```

#### Procmail

Reads procmail's `LOGFILE`. Each delivered mail is logged as a `From` line with the envelope
sender and the date, a `Subject:` line, and a `Folder:` line with where the mail went, colored as
`path`, and its size. Diagnostics carry a `procmail: ` prefix and, under `VERBOSE=on`, a process
id. In a diagnostic, a quoted path is colored as `path` and a quoted assignment has its variable
colored as `header`. `Match on` is a success, `No match on`, `Skipped`, and `Timeout` are warnings,
and `Couldn't`, `Error`, `Unable`, `Lock failure`, and `Bad substitution` are failures.

**Example:**
```bash
splash --mode procmail --path ~/.procmail.log
```

```
From alice@example.com  Tue Oct  3 12:00:01 2023
 Subject: Quarterly report
  Folder: /home/bob/Mail/inbox						   4512
procmail: Match on "^From:.*alice@example.com"
procmail: Couldn't create "/var/mail/bob"
```

The spacing between fields is kept in every mail mode, so plain output reproduces the file.

### FTP servers

In every FTP mode a remote host is colored as `ip` when it is an IPv4 or IPv6 address and as
`host` otherwise, a user is colored as `userid`, a file as `path`, and a byte count as `size`. A
line the mode does not recognize is dropped, and the spacing between fields is kept, so plain
output reproduces the file.

#### vsftpd

Reads vsftpd's own log format, from `vsftpd.log`, where each line opens with the date and
`[pid N]`, and from syslog. The user in brackets is colored as `userid`, `OK` as a success and
`FAIL` as a failure, the event (`CONNECT`, `LOGIN`, `DOWNLOAD`, `UPLOAD`, `MKDIR`, `DELETE`,
`RENAME`, `RMDIR`, `CHMOD`, `FTP command`, `FTP response`, `DEBUG`) as `method`, and the client
address as `ip`. After the client, a quoted file is colored as `path`, a quoted `FTP command` as
`request`, the reply code of an `FTP response` as `status`, the bytes as `size`, the rate as
`number`, and an anonymous password as `user_identifier`. vsftpd's xferlog output is read by the
`xferlog` mode.

**Example:**
```bash
splash --mode vsftpd --path /var/log/vsftpd.log
```

```
Tue Oct  3 12:00:02 2023 [pid 4320] [alice] OK LOGIN: Client "10.0.0.5"
Tue Oct  3 12:00:03 2023 [pid 4322] [alice] OK DOWNLOAD: Client "10.0.0.5", "/home/alice/report.pdf", 4096 bytes, 512.00Kbyte/sec
Oct  3 12:00:07 ftp vsftpd[4324]: [bob] FAIL LOGIN: Client "::ffff:10.0.0.9"
```

#### ProFTPD

Reads three ProFTPD logs. A `SystemLog` line opens with a date carrying milliseconds, an optional
host, `proftpd[pid]`, and the server name, colored as `vhost`, with the client's name and address
in parentheses after it. A syslog line carries the same server and client after the syslog header,
followed by ` - `. An `ExtendedLog` line in the default format (`%h %l %u %t "%r" %s %b`) is
colored like a Common Log Format line, with the FTP command as `method`, its argument as
`request`, and the reply code as `status`. In a message, the user after `USER` or `ANON` is colored
as `userid`. `Login successful` and `Authenticated without password` are successes, and
`Login failed`, `Incorrect password`, `No such user found`, `Password expired`, `Account disabled`,
and `Limit access denies login` are failures. ProFTPD's `TransferLog` is read by the `xferlog` mode.

**Example:**
```bash
splash --mode proftpd --path /var/log/proftpd/proftpd.log
```

```
2023-10-03 12:00:01,123 ftp proftpd[4321] ftp.example.com (client.example.org[10.0.0.5]): USER alice: Login successful.
Oct  3 12:00:02 ftp proftpd[4322]: ftp.example.com (10.0.0.9[10.0.0.9]) - USER bob (Login failed): Incorrect password
10.0.0.5 - alice [03/Oct/2023:12:00:03 +0000] "RETR report.pdf" 226 4096
```

#### pure-ftpd

Reads pure-ftpd's syslog lines, `(user@host) [LEVEL] message`, with the level colored as `level`,
and its `AltLog` transfer log in the `clf` and `w3c` formats. A finished transfer colors the file
as `path`, `downloaded` and `uploaded` as successes, `partially uploaded` as a warning, and the
bytes and rate. `is now logged in` and `Anonymous user logged in` are successes, `Timeout` is a
warning, and `Authentication failed`, `Login authentication failed`, and `Could not delete` are
failures. In a `w3c` log, a `#Fields:` style header has its name colored as `header`, and a record
colors the action (`sent` or `created`) as `method`. The `stats` and `xferlog` formats are read by
the `ftpstats` and `xferlog` modes.

**Example:**
```bash
splash --mode pure-ftpd --path /var/log/messages
```

```
Oct  3 12:00:02 ftp pure-ftpd: (?@10.0.0.5) [INFO] alice is now logged in
Oct  3 12:00:03 ftp pure-ftpd: (alice@10.0.0.5) [NOTICE] /home/alice//report.pdf downloaded  (4096 bytes, 512.00KB/sec)
2023-10-03 12:00:03 10.0.0.5 []sent /home/alice/report.pdf 226 alice 4096
```

#### xferlog

Reads the xferlog transfer format that wu-ftpd introduced and vsftpd, ProFTPD, and pure-ftpd also
write. The date is colored as `timestamp`, the transfer time as `duration`, the transfer type as
`protocol`, the special action flag as `tag`, the direction as `method`, the access mode as
`level`, the service as `module`, the authentication method as `number`, and the authenticated
user id as `user_identifier`. A completed transfer (`c`) is a success and an incomplete one (`i`)
is a failure. A file name holding a space is read as one path.

**Example:**
```bash
splash --mode xferlog --path /var/log/xferlog
```

```
Tue Oct  3 12:00:01 2023 2 10.0.0.5 4096 /home/alice/report.pdf b _ o r alice ftp 0 * c
Tue Oct  3 12:00:09 2023 0 client.example.org 0 /pub/release notes.txt a _ o a guest@example.org ftp 0 * i
```

#### ftpstats

Reads pure-ftpd's `stats` transfer log. The Unix time is colored as `timestamp`, the session id as
`transaction`, the direction (`U` or `D`) as `method`, and the transfer time as `duration`.

**Example:**
```bash
splash --mode ftpstats --path /var/log/pureftpd.log
```

```
1696334401 651c0b41.10e1 alice 10.0.0.5 D 4096 2 /home/alice/report.pdf
```

### System logs

#### Syslog

Reads syslog lines from any program. The header is colored as a `timestamp`, a `host`, a program
(`tag`), and a process id (`pid`). Its timestamp can be in the traditional `Oct  3 12:00:01` form,
with or without microseconds, or in RFC 3339 form, with or without a colon in the offset. The
`journalctl` timestamp forms listed under journalctl are read too. A line with no program, such as
`last message repeated 3 times`, keeps its timestamp and host colored.

A message read off the wire opens with its priority in angle brackets, such as `<34>`. The number
is colored by the severity it holds: `emerg` through `err` as `failure`, `warning` as `warning`,
and the rest as `level`. After a priority, an RFC 5424 message has its version colored as
`number`, its message id as `transaction`, and a field left out as a `-`. Each structured data
element has its id colored as `module`, its parameter names as `header`, and its values as
`message`. In any message, `error`, `failed`, and `failure` are failures and `warning` is a warning.

**Example:**
```bash
splash --mode syslog --path /var/log/syslog
```

```
Oct  3 12:00:01 web01 nginx[2200]: worker process 2201 exited on signal 9
<34>Oct 11 22:14:15 mymachine su: 'su root' failed for alice on /dev/pts/8
<165>1 2003-10-11T22:14:15.003Z mymachine.example.com evntslog - ID47 [exampleSDID@32473 iut="3" eventSource="Application"] An application event log entry
```

#### journalctl

Reads `journalctl` output in the `short`, `short-precise`, `short-iso`, `short-iso-precise`,
`short-full`, `short-monotonic`, and `short-unix` modes, and the `-- Boot <id> --`,
`-- No entries --`, and `-- Journal begins at ... --` markers between entries. A boot id is
colored as `transaction`. A unit name such as `nginx.service` or `multi-user.target` is colored as
`module`, and a `key=value` field such as `code=exited` has its key colored as `header`.
`Started`, `Finished`, `Reached target`, and `Listening on` are successes, `Stopping`, `Stopped`,
and `Main process exited` are warnings, and `Failed to start`, `Failed`, `failed`, and `error` are
failures.

**Example:**
```bash
journalctl -b | splash --mode journalctl
```

```
Oct 03 12:00:02 web01 systemd[1]: Started nginx.service - A high performance web server.
Oct 03 12:00:04 web01 systemd[1]: app.service: Main process exited, code=exited, status=1/FAILURE
-- Boot 6f1d2c3b4a5968778695a4b3c2d1e0f9 --
```

#### dmesg

Reads `dmesg` output. A line can open with the raw priority `dmesg -r` prints, colored by severity
as in the syslog mode, the facility and level `dmesg -x` prints, and a timestamp: the seconds since
boot, the date `dmesg -T` prints, or the ISO date `dmesg --time-format iso` prints. The facility is
colored as `module`, and the level is colored as `failure` for `emerg`, `alert`, `crit`, and
`err`, `warning` for `warn`, and `level` otherwise. Kernel lines in syslog, logged by the `kernel`
program, are read too. The subsystem or device a message opens with, such as `usb 1-1` or
`EXT4-fs (sda1)`, is colored as `module`. `Link is Up` is a success, `Link is Down` and `warning`
are warnings, and `error`, `I/O error`, `failed`, `segfault`, `Out of memory`, and `Call Trace`
are failures.

**Example:**
```bash
dmesg -x | splash --mode dmesg
```

```
kern  :info  : [    1.234567] usb 1-1: new high-speed USB device number 2 using xhci_hcd
kern  :err   : [ 1234.567890] blk_update_request: I/O error, dev sdb, sector 2048
```

#### auth.log

Reads the syslog lines of `auth.log` (`secure` on Red Hat systems) from sshd, sudo, su, login,
PAM, and systemd-logind. The user a phrase is about, such as `for alice`, `invalid user admin`, or
`(to root)`, is colored as `userid`, a port as `number`, and a PAM module and the service it ran
for, such as `pam_unix(sshd:session)`, as `module`. A sudo line colors the user who ran it, and its
`PWD=` as `path`, `USER=` as `userid`, and `COMMAND=` as `request`. In a `key=value` field,
`user=`, `ruser=`, and `logname=` are `userid`, `uid=` and `euid=` are `number`, and `rhost=` is a
host or address. `Accepted`, `session opened`, `New session`, `Successful su`, and
`Server listening on` are successes, `session closed`, `Removed session`, `Disconnected`,
`Received disconnect`, and `Connection closed` are warnings, and `Failed password`,
`Failed publickey`, `invalid user`, `authentication failure`, `FAILED SU`, `FAILED su`,
`incorrect password attempts`, `NOT in sudoers`, `command not allowed`, `a password is required`,
`maximum authentication attempts exceeded`, `Timeout before authentication`,
`Unable to negotiate`, `error`, and `fatal` are failures.

**Example:**
```bash
splash --mode auth --path /var/log/auth.log
```

```
Oct  3 12:00:01 web01 sshd[4101]: Accepted password for alice from 10.0.0.5 port 52144 ssh2
Oct  3 12:00:02 web01 sshd[4102]: Failed password for invalid user admin from 203.0.113.7 port 40022 ssh2
Oct  3 12:00:05 web01 sudo:    alice : TTY=pts/0 ; PWD=/home/alice ; USER=root ; COMMAND=/usr/bin/apt update
```

#### cron

Reads the syslog lines of `CRON`, `cron`, `crond`, `CROND`, and `anacron`, and cronie's own log
file. An event, `(user) EVENT (detail)`, has its user colored as `userid` and its event as
`method`, or `failure` for an `ERROR`. The detail of a `CMD`, `CMDOUT`, or `CMDEND` event is the
command, colored as `request`. In cronie's log file the date and process id before the event are
colored as `timestamp` and `pid`. anacron's job names, such as ``Job `cron.daily' started``, are
colored as `module`, and the PAM session lines cron writes are colored as in the auth mode.

**Example:**
```bash
splash --mode cron --path /var/log/cron
```

```
Oct  3 12:00:01 web01 CRON[5101]: (root) CMD (cd / && run-parts --report /etc/cron.hourly)
Oct  3 12:10:00 web01 anacron[5300]: Job `cron.daily' started
root (10/03-12:00:01-5101) CMD (run-parts /etc/cron.hourly)
```

#### ulogd

Reads netfilter packet logs: ulogd's `LOGEMU` output, a date and host followed by the packet, and
the kernel's `LOG` target in syslog, logged by the `kernel` program, with or without the seconds
since boot. The prefix a rule sets, such as `[UFW BLOCK]`, colors `ACCEPT` and `ALLOW` as
successes, `AUDIT` and `LIMIT` as warnings, and `BLOCK`, `DENY`, `DROP`, and `REJECT` as
failures. Each `KEY=value` field has its key colored as `header`. `IN=` and `OUT=` are `module`,
`SRC=` and `DST=` are `ip` (IPv4 or IPv6), `PROTO=` is `protocol`, `LEN=` is `size`, `UID=` and
`GID=` are `userid`, `MAC=` is `message`, and the rest are `number`. A bare flag such as `SYN` or
`DF` is colored as `tag`.

**Example:**
```bash
splash --mode ulogd --path /var/log/ulog/syslogemu.log
```

```
Oct  3 12:00:01 fw01 [UFW BLOCK] IN=eth0 OUT= SRC=203.0.113.7 DST=10.0.0.1 LEN=60 TOS=00 TTL=52 ID=54321 DF PROTO=TCP SPT=40022 DPT=22 SYN URGP=0
```

The spacing between fields is kept in every system log mode, so plain output reproduces the file.

### Application logs

#### PHP

Reads PHP's `error_log` file and PHP-FPM's log. Each line opens with a bracketed date, which the
error log follows with its time zone. After `PHP`, colored as `tag`, the error type is colored
by how severe it is: `Fatal error`, `Recoverable fatal error`, `Parse error`, and `Unknown error`
are failures, `Warning` is a warning, and `Notice` and `Deprecated` are levels. The file and line
an error happened on are colored as `path` and `number`, whether written as `in /path on line 42`
or `/path:42`. The stack trace after an uncaught exception is read too, with each frame's number,
file, and line colored. A PHP-FPM level is colored as `failure` for `ERROR` and `ALERT`, `warning`
for `WARNING`, and `level` otherwise, and a pool and child process are colored as `module` and
`pid`.

**Example:**
```bash
splash --mode php --path /var/log/php_errors.log
```

```
[03-Oct-2023 12:00:01 UTC] PHP Warning:  Undefined variable $total in /var/www/html/cart.php on line 42
[03-Oct-2023 12:00:05 UTC] PHP Fatal error:  Uncaught Exception: Payment gateway timed out in /var/www/html/pay.php:88
#0 /var/www/html/checkout.php(20): charge()
[03-Oct-2023 12:00:07] WARNING: [pool www] server reached pm.max_children setting (5), consider raising it
```

#### Apache error

Reads Apache's error log alone, where the httpd mode reads it mixed with access lines, and colors
each bracketed field apart. The level is colored as `failure` for `emerg`, `alert`, `crit`, and
`error`, `warning` for `warn`, and `level` otherwise. The process and thread ids are each colored
as `pid`, and the client's address and port as `ip` and `number`, with an IPv6 address keeping its
own colons. An error code such as `AH00128` is colored as `transaction`, and the message is colored
as in the PHP mode, so the PHP errors mod_php and proxy_fcgi pass through keep their colors.

**Example:**
```bash
splash --mode apache-error --path /var/log/apache2/error.log
```

```
[Wed Oct 11 14:32:52.123456 2023] [core:error] [pid 35708:tid 4328636416] [client 72.15.99.187:1234] AH00128: File does not exist: /var/www/favicon.ico
[Wed Oct 11 14:33:05.000100 2023] [php7:error] [pid 35710] [client 2001:db8::1:52144] PHP Warning:  Undefined variable $total in /var/www/html/cart.php on line 42
```

#### MySQL

Reads MySQL's error log, `time thread [label] [err_code] [subsystem] message`, along with the
MySQL 5.7 and MariaDB forms that leave out the code and subsystem. The thread is colored as `pid`,
the code as `transaction`, and the subsystem as `module`. The label is colored as `failure` for
`ERROR`, `warning` for `Warning`, and `level` otherwise. In the slow query log, `# Time:` colors
its timestamp, `# User@Host:` colors the users, host, address, and connection id, and
`# Query_time:` colors each time as `duration` and each count as `number`. A statement is colored
as `request`.

**Example:**
```bash
splash --mode mysql --path /var/log/mysql/mysql-slow.log
```

```
2023-10-03T12:00:02.234567Z 0 [Warning] [MY-010068] [Server] CA certificate ca.pem is self signed.
# User@Host: app[app] @ localhost [10.0.0.5]  Id:    42
# Query_time: 2.345678  Lock_time: 0.000123 Rows_sent: 1  Rows_examined: 1000000
SELECT * FROM orders WHERE customer_note LIKE '%gift%';
```

#### PostgreSQL

Reads PostgreSQL's server log with the default `log_line_prefix` of `%m [%p] `, or with Debian's
`%m [%p] %q%u@%d `, which adds the user (`userid`) and database (`module`). The severity is
colored as `failure` for `ERROR`, `FATAL`, and `PANIC`, `warning` for `WARNING`, and `level` for
the rest. The `DETAIL`, `HINT`, `QUERY`, `CONTEXT`, `LOCATION`, and `STATEMENT` lines that belong
to a message are colored as `header`, and the text of `STATEMENT` and `QUERY` as `request`. A
SQLSTATE is colored as `status`. A slow statement from `log_min_duration_statement` colors its
duration and statement, and a quoted user or database inside a message is colored too.

**Example:**
```bash
splash --mode postgresql --path /var/log/postgresql/postgresql-16-main.log
```

```
2023-10-03 12:00:03.345 UTC [1302] FATAL:  password authentication failed for user "app"
2023-10-03 12:00:04.456 UTC [1303] app@appdb LOG:  duration: 12.345 ms  execute <unnamed>: SELECT 1
```

#### Redis

Reads Redis's log, `pid:role date mark message`. The role (`M`, `S`, `C`, or `X`) is colored as
`module`, and the level mark is colored as `warning` for `#` and `level` for `.`, `-`, and `*`.
`Ready to accept connections`, `DB saved on disk`, and the success messages of background saving
and replica sync are successes, `WARNING` is a warning, and `error`, `failed`, `MISCONF`, and
`Out Of Memory` are failures.

**Example:**
```bash
splash --mode redis --path /var/log/redis/redis-server.log
```

```
1234:M 03 Oct 2023 12:00:00.123 * Ready to accept connections tcp
1234:M 03 Oct 2023 12:06:00.000 # WARNING: The TCP backlog setting of 511 cannot be enforced
```

#### MongoDB

Reads the JSON lines MongoDB 4.4 and later write, and the text lines of earlier releases. The
severity in `s` is colored as `failure` for `F` and `E`, `warning` for `W`, and `level` otherwise.
The component is colored as `module`, the log id as `transaction`, the thread in `ctx` as `tag`,
and `msg` as `message`. Among the attributes, a remote address is colored as `host`, a user as
`userid`, a namespace or database as `module`, `durationMillis` as `duration`, and an error as
`failure`.

**Example:**
```bash
splash --mode mongodb --path /var/log/mongodb/mongod.log
```

```
{"t":{"$date":"2023-10-03T12:00:03.345+00:00"},"s":"W",  "c":"COMMAND",  "id":51803,   "ctx":"conn12","msg":"Slow query","attr":{"ns":"appdb.orders","durationMillis":2345}}
2019-10-03T12:00:01.123+0000 I  NETWORK  [listener] connection accepted from 10.0.0.5:52144
```

#### Elasticsearch

Reads Elasticsearch's plain text logs, `[date][level][logger] [node] message`, for the server log
and the search and indexing slow logs, along with the JSON logs of Elasticsearch 7 and the ECS
JSON logs of Elasticsearch 8. The level is colored as `failure` for `ERROR` and `FATAL`, `warning`
for `WARN`, and `level` otherwise. The logger is colored as `module` and the node as `host`. In a
message, a bracketed name such as an index and shard is colored as `module`, and a slow log value
is colored by its name: `took` and `took_millis` as `duration`, `total_hits` and `total_shards` as
`number`, and `source` as `request`.

**Example:**
```bash
splash --mode elasticsearch --path /var/log/elasticsearch/es-prod_index_search_slowlog.log
```

```
[2023-10-03T12:00:05,567][WARN ][i.s.s.query              ] [node-1] [logs][0] took[2.3s], took_millis[2345], total_hits[10 hits]
```

The spacing between fields is kept in every application log mode, so plain output reproduces the
file.

### Specialized services

#### SSH

Reads sshd's syslog lines, under `sshd` or `sshd-session`, and drops every other program's lines.
The outcome of a login attempt is colored as `success` for `Accepted`, `failure` for `Failed`, and
`warning` for `Postponed` and `Partial`, and the method after it, such as `publickey` or
`keyboard-interactive/pam`, as `protocol`. A key's type is colored as `module` and its
fingerprint as `transaction`, `ssh2` as `protocol`, and `[preauth]` as `tag`. Users, addresses,
ports, PAM lines, and the words that say whether a login worked are colored as in the auth mode.

**Example:**
```bash
splash --mode ssh --path /var/log/auth.log
```

```
Oct  3 12:00:01 web01 sshd[4101]: Accepted publickey for alice from 10.0.0.5 port 52144 ssh2: ED25519 SHA256:4a2Xb9QeZ0kR7pYt3sLmN1cVwH8uJfD6gB5oK2iE0Aq
Oct  3 12:00:03 web01 sshd[4103]: Connection closed by authenticating user root 203.0.113.7 port 40100 [preauth]
```

#### sudo and su

Reads the syslog lines of `sudo` and `su`, and sudo's own log file, whose entries open with the
date and the user, as `Oct  3 12:00:05 : alice : `. A sudo entry is colored as in the auth mode:
the user who ran it, `PWD=` as `path`, `USER=` as `userid`, `COMMAND=` as `request`, and
`incorrect password attempts`, `NOT in sudoers`, `command not allowed`, and
`a password is required` as failures. su's `(to root) alice on pts/1`, `FAILED SU`,
`Successful su for root by alice`, and `FAILED su` lines color the users involved. shadow's
`+ /dev/pts/1 alice:root` colors `+` as a success and `-` as a failure, the terminal as `path`,
and both users as `userid`.

**Example:**
```bash
splash --mode sudo --path /var/log/sudo.log
```

```
Oct  3 12:00:07 web01 sudo:      bob : command not allowed ; TTY=pts/1 ; PWD=/home/bob ; USER=root ; COMMAND=/sbin/reboot
Oct  3 12:00:10 web01 su[4202]: + /dev/pts/1 alice:root
Oct  3 12:00:07 2023 : bob : user NOT in sudoers ; TTY=pts/1 ; USER=root ; COMMAND=/bin/bash
```

#### super

Reads super(1)'s log file, `user@host date<TAB>command (arguments)`, with or without the program
name a `logfile` setting can put first, and its syslog lines under `super`. The user is colored as
`userid`, the host as `host`, the command as `method`, and its arguments as `request`. Any other
message, such as an error super reports, is colored as message text.

**Example:**
```bash
splash --mode super --path /var/log/super.log
```

```
alice@web01 Tue Oct  3 12:00:01 2023	shutdown (-h now)
```

#### sulog

Reads the System V `sulog`, `SU 10/03 12:00 + pts/1 alice-root`. `+` is colored as a success and
`-` as a failure, the terminal as `path`, and the user who ran su and the user they became as
`userid`.

**Example:**
```bash
splash --mode sulog --path /var/adm/sulog
```

```
SU 10/03 12:00 + pts/1 alice-root
SU 10/03 12:05 - pts/2 bob-root
```

#### distcc

Reads distccd's log file, `distccd[pid] (function) message`, and its syslog lines under
`distccd`. The function is colored as `module`. A job summary colors the client's address and
port, each `exit:`, `sig:`, `core:`, and `ret:` field as `number`, and `time:` as `duration`.
`COMPILE_OK` is a success, `CLI_DISCONN` and `REJ_OVERLOAD` are warnings, and `COMPILE_ERROR`,
`COMPILE_TIMEOUT`, `REJ_BAD_REQ`, and `ERROR` are failures.

**Example:**
```bash
splash --mode distcc --path /var/log/distccd.log
```

```
distccd[5101] (dcc_job_summary) client: 10.0.0.5:52144 COMPILE_OK exit:0 sig:0 core:0 ret:0 time:1234ms gcc main.c
```

#### Icecast

Reads Icecast 2's error log, `[2023-10-03  12:00:01] INFO main/main message`, with the level
colored as `failure` for `EROR`, `warning` for `WARN`, and `level` otherwise, and the module and
function as `module`. Its access log is read as in the httpd mode, so the seconds a listener stayed
connected are colored as `duration`. Icecast 1's `[date] [thread:name] message` lines and its
usage lines, with the bandwidth and the source, client, and admin counts, are read too.

**Example:**
```bash
splash --mode icecast --path /var/log/icecast2/error.log
```

```
[2023-10-03  12:00:01] INFO connection/_handle_source_request Source logging in at mountpoint "/live.mp3" from 10.0.0.20
[2023-10-03  12:00:03] EROR sock/sock_write_bytes_wrapper Failed to write to socket
```

#### APM

Reads apmd's syslog lines. A battery line colors the charge as `size`, the rate it measured as
`number`, and each time as `duration`. `charging` is a success, `discharging` and `not charging`
are warnings, and `absent` is a failure. `Warning` is a warning, and `BATTERY IS LOW`, a kernel
rejecting a suspend, and an unknown event are failures.

**Example:**
```bash
splash --mode apm --path /var/log/syslog
```

```
Oct  3 12:00:00 laptop apmd[800]: Battery: 87%, discharging (-0.50%/min over 0:10:00), 1:23:45 (2:54:00) to empty
Oct  3 12:41:00 laptop apmd[800]: Warning: BATTERY IS LOW
```

#### Kernel oops

Reads the lines of a Linux kernel oops, warning, or panic, with or without the prefixes dmesg and
syslog put on kernel lines, and drops every other kernel line, so it pulls an oops out of a whole
kernel log. `BUG`, `Oops`, `Kernel panic - not syncing`, `general protection fault`,
`kernel NULL pointer dereference`, and `soft lockup` are failures, and `WARNING` and
`blocked for more than` are warnings. The CPU and process id are colored as `number` and the
command as `tag`. A register or other field name is colored as `header`, a function and its offset
as `module`, the module it lives in as `tag`, and a hexadecimal value as `number`.

ccze's `oops` plugin reads logs from the Oops web proxy. This mode reads kernel oopses instead.

**Example:**
```bash
dmesg | splash --mode oops
```

```
[ 1234.567890] BUG: kernel NULL pointer dereference, address: 0000000000000008
[ 1234.567967] RIP: 0010:ext4_do_writepages+0x1a/0x40 [ext4]
[ 1234.568044]  ext4_writepages+0x1a/0x30 [ext4]
```

The spacing between fields is kept in every specialized service mode, so plain output reproduces
the file.

### Containers, infrastructure, and CI

#### Docker

Reads the json-file logging driver's lines, with the line the container printed colored as
`message`, `stderr` as `warning`, `stdout` as `level`, and the time as `timestamp`. Reads the
logfmt lines dockerd and containerd write, from a file or from syslog and the journal, with
`level=` colored by how severe it is, `msg=` as `message`, `error=` as `failure`, a container or
task id as `transaction`, and `module=` as `module`. Reads `docker logs --timestamps` output,
coloring the time and any address in the line.

**Example:**
```bash
splash --mode docker --path /var/lib/docker/containers/4f2a1c0123ab/4f2a1c0123ab-json.log
```

```
{"log":"listening on :8080\n","stream":"stdout","time":"2023-10-03T12:00:01.123456789Z"}
time="2023-10-03T12:00:02.234567890Z" level=error msg="Handler for POST /v1.43/containers/create returned error: No such image: app:missing"
```

#### Kubernetes

Reads klog lines, `I1003 12:00:01.123456    1234 controller.go:123] message`, on their own or
behind a syslog header. The severity is colored as `level` for `I`, `warning` for `W`, and
`failure` for `E` and `F`, the thread id as `pid`, and the file and line as `path` and `number`.
A structured message colors its quoted message and its `key=value` pairs: a pod, node, or
namespace as `module`, a UID as `transaction`, and an error as `failure`. Reads the CRI lines the
kubelet writes to `/var/log/containers`, `2023-10-03T12:00:01.123456789Z stdout F message`,
coloring the stream and the `F` or `P` tag, and a container line that is itself a klog line.

**Example:**
```bash
kubectl logs -n kube-system kube-scheduler-node01 | splash --mode kubernetes
```

```
E1003 12:00:03.345678    1234 pod_workers.go:965] "Error syncing pod, skipping" err="failed" pod="default/web-0"
2023-10-03T12:00:02.234567890Z stderr P panic: runtime error: index out of range
```

#### systemd-resolved

Reads systemd-resolved's journal lines. A feature level such as `UDP` or `UDP+EDNS0` is colored as
`protocol`, a DNS server's IPv4 or IPv6 address as `ip`, and a resource record such as
`. IN DS` as a `host` and its type. `Using degraded feature set`, the downgrades, and
`Switching to` are warnings, `Grace period over, resuming full feature set` is a success, and
`NXDOMAIN`, `SERVFAIL`, and `REFUSED` are failures.

**Example:**
```bash
journalctl -u systemd-resolved | splash --mode systemd-resolved
```

```
Oct 03 12:00:01 web01 systemd-resolved[600]: Using degraded feature set UDP instead of UDP+EDNS0 for DNS server 10.0.0.53.
```

#### nginx error

Reads nginx's error log alone, where the httpd mode reads it mixed with access lines, and colors
the parts of each message. The level is colored as `failure` for `emerg`, `alert`, `crit`, and
`error`, `warning` for `warn`, and `level` otherwise. The process and thread ids are colored as
`pid` and the connection number as `transaction`. A system call such as `open()` is colored as
`module`, a quoted file as `path`, and an error number and its text as `number` and `failure`.
The request's context colors `client` as an address, `server` as `vhost`, `request` and
`upstream` as `request`, `host` as `host`, and `referrer` as `referer`.

**Example:**
```bash
splash --mode nginx-error --path /var/log/nginx/error.log
```

```
2023/10/03 12:00:01 [error] 1234#5678: *42 open() "/usr/share/nginx/html/favicon.ico" failed (2: No such file or directory), client: 10.0.0.5, server: example.com, request: "GET /favicon.ico HTTP/1.1", host: "example.com"
```

#### Git

Reads `git daemon` lines, through syslog under `git-daemon` or from standard error with the process
id in brackets. A client's address and port are colored as `ip` and `number`, the service in a
request as `method`, and the repository as `path`. `Ready to rumble` is a success, `Disconnected`
is a warning, and the refusals `git daemon` logs, such as `repository not exported`, are failures.
Reads Git's `GIT_TRACE` output, coloring the time, the file and line, and what Git ran, as in
`trace: built-in: git fetch origin`.

**Example:**
```bash
GIT_TRACE=1 git fetch 2>&1 | splash --mode git
```

```
Oct  3 12:00:01 git01 git-daemon[3101]: Request upload-pack for '/srv/git/app.git'
12:00:01.123456 git.c:463               trace: built-in: git fetch origin
```

#### CI

Reads Jenkins console logs, Jenkins' server log, and GitHub Actions logs. A build's output belongs
to its log, so this mode keeps every line. It colors `Started by` and the other lines a build
starts with, `[Pipeline]` steps, the commands a shell step echoes, and `Finished:` with the result
colored by how the build went. In Jenkins' server log the level is colored as `failure` for
`SEVERE`, `warning` for `WARNING`, and `level` otherwise. In a GitHub Actions log, the time,
`##[group]` and the other markers, and the `::error::` workflow commands are colored. Words such
as `FAILED`, `error`, and `warning` are colored in the build output.

**Example:**
```bash
splash --mode ci --path build.log
```

```
[Pipeline] sh
+ make test
Finished: FAILURE
2023-10-03T12:00:05.5678901Z ##[error]Process completed with exit code 1.
```

#### cloud-init

Reads `/var/log/cloud-init.log`, `2023-10-03 12:00:01,123 - util.py[DEBUG]: message`, its
`[CLOUDINIT]` syslog lines, and the stage banners and `ci-info:` lines of
`cloud-init-output.log`. The file is colored as `path` and the level by how severe it is. A stage
banner colors the version, the stage, and the seconds since boot. A `start:` or `finish:` event
colors the module, and `SUCCESS` and `ran successfully` are successes, and `FAIL` and `Failed` are
failures.

**Example:**
```bash
splash --mode cloud-init --path /var/log/cloud-init.log
```

```
2023-10-03 12:00:01,123 - util.py[DEBUG]: Cloud-init v. 23.3.1 running 'init-local' at Tue, 03 Oct 2023 12:00:01 +0000. Up 5.12 seconds.
2023-10-03 12:00:03,345 - handlers.py[DEBUG]: finish: init-network/config-ssh: SUCCESS: config-ssh ran successfully
```

#### Terraform

Reads `TF_LOG` lines, with the level, the logger, and the `key=value` pairs colored, and the JSON
lines `-json` writes, with `@level`, `@message`, `@module`, `@timestamp`, and `type` colored by
key. Reads the CLI's output: a resource operation colors the resource as `module`, the operation
as a warning while it runs, a success when complete, and a failure when errored, how long it took
as `duration`, and the resource's id as `transaction`. The plan's `+`, `-`, `-/+`, and `~` signs
are colored by their action, a summary such as `Apply complete!` colors its counts, and
`Error:` and `Warning:` diagnostics are colored, inside their box or not.

**Example:**
```bash
terraform apply 2>&1 | splash --mode terraform
```

```
aws_instance.web: Creation complete after 32s [id=i-0abc123def4567890]
Apply complete! Resources: 1 added, 0 changed, 1 destroyed.
```

The spacing between fields is kept in every one of these modes, so plain output reproduces the
file.

### Ad-hoc Mode

Automatically detects and highlights patterns in unstructured logs:

- **IP addresses** (IPv4)
- **HTTP verbs** (GET, POST, PUT, DELETE, etc.)
- **Numbers**
- **Timestamps**
- **Special characters** (quotes, brackets)

**Example:**
```bash
splash --mode ad-hoc --path /var/log/syslog
```

**Default mode:** If no mode is specified, `ad-hoc` is used by default.

---

## Output Formats

Every parsing mode can be written out in five formats, selected with `--output`.

### ansi

ANSI escape sequences for a color terminal. This is the default.

```bash
splash --mode clf --path access.log
```

### curses

A full-screen scrollable viewer. splash reads the whole input, colorizes it, and shows one
screenful at a time with a status line giving the visible range.

```bash
splash --mode clf --path access.log --output curses
```

| Key | Action |
| --- | --- |
| `j`, down arrow | Scroll down one line |
| `k`, up arrow | Scroll up one line |
| `f`, space, page down | Scroll down one screen |
| `b`, page up | Scroll up one screen |
| `g`, home | Jump to the first line |
| `G`, end | Jump to the last screen |
| `q`, escape | Quit |

The viewer needs a terminal. Redirecting its output exits with an error naming the other modes.

### html

A standalone HTML document. Each token becomes a `<span>` with a `splash-*` class, and the
document carries a stylesheet defining a color for every class, so the colors can be changed
by editing the CSS.

```bash
cat access.log | splash --mode clf --output html > access.html
```

### json

One JSON object per line, holding the line text and the styled tokens it was split into.
In `clf` mode the token kinds are the Common Log Format field names.

```bash
cat access.log | splash --mode clf --output json
```

```json
{"text":"127.0.0.1 - frank ...","tokens":[{"kind":"client","text":"127.0.0.1"}]}
```

### plain

The parsed line with no styling, for scripting.

```bash
cat access.log | splash --mode clf --output plain
```

---

## Performance

Parsed tokens borrow the text of the line they came from, so a line is read once and never copied
on its way to the renderer. Patterns are compiled the first time they are needed and kept for the
rest of the run, and patterns supplied at run time are cached by `parser::cached_pattern`.

Files are read whole and rendered before splash starts watching for new lines. A file of 64 KiB or
more is memory mapped rather than read into memory, and its lines are split across worker threads.
`--jobs N` sets the worker count, which defaults to one per core. Inputs under 512 lines are
rendered on one thread, where splitting the work costs more than it saves.

```bash
splash --mode clf --path /var/log/apache2/access.log --jobs 8
```

### Benchmarks

`cargo bench` times splash against itself at several settings and, when ccze is installed, against
ccze over the same log.

```
Rendering 20000 log lines
  splash clf ansi                 20000 lines in    33.636 ms        594603 lines/sec    1.00x
  splash clf ansi -j 18           20000 lines in     5.355 ms       3734740 lines/sec    6.28x
  splash clf plain                20000 lines in    18.454 ms       1083771 lines/sec    1.82x
  splash ad-hoc ansi              20000 lines in    36.841 ms        542869 lines/sec    0.91x
```

The numbers above are from one machine, on a generated Common Log Format sample. Run the benchmark
on yours before drawing conclusions from them.

---

## Features

### Current (v0.1.0)

**Log Format Support**
- Common Log Format (CLF) parsing
- Apache and nginx access logs: combined, vhost combined, and extended
- Apache and nginx error logs
- Squid access logs, native and httpd-emulated, with cache hit and miss highlighting
- Varnish transaction and raw logs, with each record payload colored by its tag
- HAProxy logs from `option httplog` and `option tcplog`, plus error lines
- Caddy structured JSON logs, scanned as JSON and colored by key
- Postfix syslog lines, with queue ids, addresses, relays, and delivery status
- Exim main logs, with message ids and arrival, delivery, deferral, and failure flags
- Dovecot syslog and log file lines, with the service, user, and session
- Fetchmail syslog and log file lines, with the user, server, sizes, and flush state
- Procmail delivery abstracts and diagnostics
- vsftpd log file and syslog lines, with the event, client, file, and outcome
- ProFTPD system, syslog, and default extended log lines
- pure-ftpd syslog lines and `clf` and `w3c` transfer logs
- xferlog transfer logs from any FTP server that writes them
- pure-ftpd ftpstats transfer logs
- Syslog lines from any program, with `<PRI>` priorities and RFC 5424 structured data
- journalctl output in its `short` modes, with units and boot markers
- dmesg output, raw, decoded, and timestamped
- auth.log lines from sshd, sudo, su, and PAM, with users and login results
- cron and anacron job logs, from syslog and cronie's log file
- Netfilter packet logs from ulogd's `LOGEMU` output and the kernel's `LOG` target
- PHP error logs with stack traces, and PHP-FPM logs
- Apache error logs on their own, with each field and passed-through PHP errors colored
- MySQL and MariaDB error logs and slow query logs
- PostgreSQL server logs, with severities, statements, and slow statement durations
- Redis server logs
- MongoDB structured JSON and text logs
- Elasticsearch plain text, JSON, and slow logs
- OpenSSH server logs, with login outcomes, methods, and keys
- sudo and su logs, from syslog and sudo's own log file
- super(1) log files and syslog lines
- System V sulog
- distccd logs, with job summaries
- Icecast 2 error and access logs, and Icecast 1 logs
- apmd battery and power logs
- Linux kernel oops, warning, and panic messages
- Docker json-file, daemon, and timestamped container logs
- Kubernetes klog and CRI container logs
- systemd-resolved DNS resolution logs
- nginx error logs on their own, with system calls, files, and request context colored
- git daemon logs and Git trace output
- Jenkins console and server logs, and GitHub Actions logs
- cloud-init logs and output logs
- Terraform logs, JSON output, and CLI output
- Ad-hoc pattern detection

**Pattern Highlighting**
- IP addresses (192.168.x.x, 10.x.x.x, etc.)
- HTTP methods (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, CONNECT, TRACE)
- HTTP status codes (200, 404, 500, etc.)
- Timestamps (multiple formats)
- Numbers
- Quotes and brackets

**Input Sources**
- File input with live watching
- Memory-mapped reads for files of 64 KiB or more
- Stdin streaming

**Performance**
- Parsing that borrows the input rather than copying it
- Patterns compiled on first use and cached
- Rendering split across worker threads, set with `--jobs`
- `cargo bench` benchmark suite, comparing to ccze when it is installed

**Configuration**
- `~/.splash/config.toml` and `~/.splashrc` config files
- Per-plugin settings
- Color themes: dark, light, solarized, dracula
- Per-field color overrides from the command line
- Saved color profiles

**Output Formats**
- ANSI colors (default)
- Curses viewer with scrolling and vim-style keys
- HTML with a customizable stylesheet
- JSON with named token kinds
- Plain text

---

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/gdonald/splash.git
cd splash

# Build and install
cargo install --path .

# Run tests
cargo test
```

### Requirements

- Rust 1.70+ (edition 2021)
- Cargo

---

## Development

### Running Tests

```bash
# All tests (27 tests)
cargo test

# Integration tests only
cargo test --test examples_runner

# Specific test
cargo test test_clf_basic_parsing

# With output
cargo test -- --nocapture

# Single-threaded (for debugging)
cargo test -- --test-threads=1
```

### Running Clippy

```bash
# Check for warnings
cargo clippy

# Treat warnings as errors
cargo clippy -- -D warnings
```

### Manual Testing

```bash
# Test CLF mode
cargo run -- --mode clf --path tests/examples/clf_basic.log

# Test ad-hoc mode
cargo run -- --mode ad-hoc --path tests/examples/adhoc_mixed.log

# Test stdin
cat tests/examples/real_apache.log | cargo run -- --mode clf

# Test with real logs
tail -f /var/log/syslog | cargo run
```
