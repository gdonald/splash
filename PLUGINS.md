# splash plugins

A plugin teaches splash a log format. Most plugins are TOML files: a name and the regular
expressions that read the format's lines, loaded when splash starts, with no build step. The
formats splash ships with are plugins written in Rust, compiled into splash.

- [Plugin files](#plugin-files): the TOML plugin format
- [Writing a plugin](#writing-a-plugin): a plugin file from sample lines to daily use
- [Built-in plugins](#built-in-plugins): the Rust plugin API
- [Plugin reference](#plugin-reference): each built-in plugin and the fields it reads

---

## Plugin files

### Where splash looks

splash loads every `.toml` file in these directories, in name order:

| Directory | |
| --- | --- |
| `~/.splash/plugins` | Your own plugins |
| `/usr/local/lib/splash/plugins` | Plugins installed for every user |
| `/usr/lib/splash/plugins` | Plugins a package installed |
| `%ProgramFiles%\splash\plugins` | On Windows, in place of the two above |
| `--plugin-dir DIR` | Any other directory, and the option can be repeated |

`splash --list-plugins` prints every plugin it loaded and the directories it searched. A
missing directory is skipped, and so is one splash cannot read.

### Keys

```toml
name = "orders"
version = "1.2.0"
description = "Order service logs"
author = "Platform team"

[patterns]
placed = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) order (?P<transaction>\d+) placed in (?P<duration>\d+ms)$'
other = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) (?P<message>.*)$'
```

| Key | Required | Meaning |
| --- | --- | --- |
| `name` | Yes | The mode name: `--mode orders` reads a log with this plugin |
| `version` | No | `MAJOR.MINOR.PATCH`, `1.0.0` when left out |
| `description` | No | Shown by `--list-plugins` |
| `author` | No | Shown by `--list-plugins` |
| `requires` | No | Plugins that read the lines no pattern reads, as `"NAME"` or `"NAME MAJOR.MINOR.PATCH"`, separated by commas |
| `[patterns]` | Yes | One or more regular expressions, each under a name of your choosing |

Any other key is an error. The name cannot be one splash uses itself: a built-in plugin's name,
`clf`, `ad-hoc`, `auto`, `k8s`, or `systemd`.

Write patterns as literal strings in single quotes. A literal string keeps every backslash as it
is, so `\d` and `\S` reach the regular expression unchanged, and it cannot hold a single quote. A
double-quoted string reads `\n`, `\t`, `\r`, `\"`, and `\\` as escapes and rejects any other
backslash. The pattern syntax is that of the Rust `regex` crate.

### How a line is read

The patterns are tried in the order the file lists them, and the first that matches reads the
line. A line no pattern matches is not shown in that plugin's mode, as with the built-in modes,
and in auto mode it is detected on its own.

Each named group is named for the color key its text is styled with, such as `timestamp`,
`level`, `client`, `status`, or `message`. The README lists every key under
[Color overrides](README.md#color-overrides). Text outside the named groups, including text
before or after an unanchored match, is plain. A group inside another named group, and a group
that matched no text, add nothing. A named group that is not a color key is an error, and an
unnamed group `( )` is allowed anywhere.

### Requiring other plugins

`requires` names plugins, built-in or loaded from another file, that read the lines none of the
plugin's own patterns read. They are tried in the order listed. A plugin for an application that
logs into Gunicorn's error log reads its own lines and leaves Gunicorn's to the gunicorn plugin:

```toml
name = "django"
requires = "gunicorn 1.0.0"
```

A version after the name is the lowest version the plugin works with: the required plugin must
have the same major version and a version at least that high. splash registers each plugin file
after the plugin files it requires, whatever their names.

What the groups are named for decides more than color:

| Group | Used by |
| --- | --- |
| `level` | `--level`, the error count of `--stats` and `--summary` |
| `timestamp` | `--since`, `--until`, `--tz`, `--relative`, `--merge`, `--histogram` |
| `client`, `ip` | `--filter ip=`, `--lookups`, `--anonymize`, the top addresses of `--stats` |
| `request`, `status` | The top URLs and status counts of `--stats` |
| `userid`, `email` | `--anonymize` |
| any key | `--filter KEY=VALUE`, and the fields of `--output jsonl` and `--output csv` |

### When a file cannot be loaded

splash reports the file and the reason on stderr and goes on without it:

```
Cannot load plugin /home/alice/.splash/plugins/orders.toml: pattern 'placed' names group 'txn', which is not a color key
```

The reasons are a TOML syntax error with its line number, a missing name, a built-in name, a
version that is not three numbers, an unknown key, no `[patterns]`, a pattern that is not a
regular expression, a named group that is not a color key, a requirement that is not a name and
an optional version, a required plugin that is not loaded or whose version is incompatible, and
plugin files that require each other.

---

## Writing a plugin

This builds a plugin for an order service that writes:

```
2023-10-03 12:00:01 INFO order 4821 placed in 38ms
2023-10-03 12:00:02 WARN payment retried for order 4821
2023-10-03 12:00:03 ERROR order 4822 failed: card declined
```

### Start from the lines

Save a few real lines to a file, `orders.log`, including the unusual ones: errors, lines with
missing fields, and lines another program writes into the same log.

### Write the plugin file

Create `~/.splash/plugins/orders.toml` with a pattern for the most specific line first and a
general pattern last:

```toml
name = "orders"
description = "Order service logs"

[patterns]
placed = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) order (?P<transaction>\d+) placed in (?P<duration>\d+ms)$'
failed = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) order (?P<transaction>\d+) (?P<failure>failed): (?P<message>.*)$'
other = '^(?P<timestamp>\S+ \S+) (?P<level>[A-Z]+) (?P<message>.*)$'
```

### Test the patterns

`--test-regex FILE` reads lines from `--path` or stdin and reports, for each line, the pattern
that read it and the text of each group, or that no pattern read it. It exits with status 1 when
a line was not read, so it can run in a test script:

```bash
splash --test-regex ~/.splash/plugins/orders.toml --path orders.log
```

```
1: placed
   timestamp        2023-10-03 12:00:01
   level            INFO
   transaction      4821
   duration         38ms
2: other
   timestamp        2023-10-03 12:00:02
   level            WARN
   message          payment retried for order 4821
3: failed
   timestamp        2023-10-03 12:00:03
   level            ERROR
   transaction      4822
   failure          failed
   message          card declined
3 of 3 lines matched
```

A line a required plugin reads is reported as `read by required plugin NAME`, and the required
plugins are looked up among the plugin files splash loads, so test with the same `--plugin-dir`
options the plugin is used with. Blank lines are skipped. With several `--path` files, each file's report follows a
`==> PATH <==` header. A plugin file that cannot be loaded exits with an error naming the reason.

### Use it

```bash
splash --mode orders --path /var/log/orders.log
splash --auto --path /var/log/orders.log
splash --mode orders --path /var/log/orders.log --output jsonl --fields transaction,duration
```

Auto mode tries plugin files along with the built-in modes.

### Change it while it runs

`--reload-plugins` watches the plugin directories and loads the plugin files again whenever one
changes, so lines rendered after you save use the new patterns:

```bash
splash --mode orders --path /var/log/orders.log --follow --reload-plugins
```

Each reload is reported on stderr with the plugins and versions it registered:

```
Reloaded plugins: orders 1.0.0
```

A file that cannot be loaded is reported and its plugin keeps the patterns it had, so a typo
does not stop the log. A plugin directory that cannot be watched is skipped. Deleting a file, or renaming its plugin, unregisters the old plugin.
Raising `version` in each save shows in the report which save was loaded.

### Examples

[`examples/plugins/`](examples/plugins/) holds plugins for Gunicorn error logs, Celery worker
logs, uWSGI request logs, and Django application logs in a Gunicorn error log, which requires the
Gunicorn plugin. Try one without installing it:

```bash
splash --plugin-dir examples/plugins --mode celery --path /var/log/celery/worker.log
```

---

## Built-in plugins

The formats splash ships with are Rust types implementing the `Plugin` trait in
[`src/plugin.rs`](src/plugin.rs). Write one in Rust when a pattern cannot read the format: JSON
whose fields can come in any order, a value styled by what it says, or a line that holds another
program's line.

### The trait

```rust
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> &PluginMetadata;
    fn parse_line<'a>(&self, line: &'a str) -> ParseResult<'a>;

    fn name(&self) -> &str { ... }
    fn version(&self) -> &PluginVersion { ... }
    fn parse_display_line<'a>(&self, line: &'a str) -> ParseResult<'a> { ... }
    fn can_parse(&self, line: &str) -> bool { ... }
    fn detect_format(&self, sample_lines: &[&str]) -> f32 { ... }
}
```

| Method | Purpose |
| --- | --- |
| `metadata` | The name, version, description, and author, built with `PluginMetadata::new` |
| `parse_line` | Reads one line of the format, returning `ParseResult::Parsed` with its tokens, or `ParseResult::NoMatch` |
| `parse_display_line` | Reads a line for rendering in the plugin's mode. It defaults to `parse_line`. The docker and kubernetes plugins override it to show the lines containers printed, which detection must not count |
| `can_parse` | Whether `parse_line` reads the line |
| `detect_format` | The share of a sample of lines the plugin reads, from 0.0 to 1.0, which auto mode ranks plugins by |

### Tokens

A `ParsedLine` is a list of `Token`s, each holding a run of the line's text and the `TokenKind`
it is styled with. `Token::new` borrows the text from the line, so parsing copies nothing. The
tokens must hold every character of the line, in order, with the parts that are not fields as
`TokenKind::Plain` or `TokenKind::Punctuation`: rendering, filtering, and the `text` field of
JSON output are built from the tokens.

Modules shared by several plugins read common pieces: `syslog::split_header` reads a syslog
header, `json::parse_object` scans a JSON object, `logfmt::push_pairs` reads `key=value` pairs,
and `container::push_output` colors a line in whatever format reads it.

### Adding one

1. Add `src/NAME.rs` with a `NAME` constant, the plugin type, and a `parse_line` function, and
   declare the module in `src/lib.rs`.
2. Register the plugin in `PluginRegistry::with_builtins` in `src/registry.rs`.
3. Add it to the `--mode` help text in `src/main.rs` and to the README's mode list and Modes
   section.
4. Add sample logs to `tests/examples/`, parsing tests to `tests/NAME_tests.rs`, and tests that
   run splash on the samples to `tests/examples_runner.rs`. Tests never go in `src/`.
5. Add the plugin's samples to `SUITES` in `tests/plugin_suites_tests.rs`, and its entry to the
   plugin reference below.

---

## Plugin reference

Each built-in plugin is named by `--mode` or `--plugin`, and listed by `--list-plugins`. Its
samples are the files in `tests/examples/` written wholly in its format, which the tests read. The
fields are the names of the token kinds the plugin gives to parts of its samples' lines. A field
name is what `--filter KEY=VALUE`, `--fields`, `--color KEY=COLOR`, a theme, and a JSON or HTML
output use for that kind. Every line also has the `plain` and `punctuation` kinds for the text
between fields. A plugin can produce fields its samples do not hold, so this list is the least it
reads.

### apache-error

Apache error logs.

Samples: [apache_error.log](tests/examples/apache_error.log)

Fields: `failure`, `header`, `ip`, `level`, `message`, `module`, `number`, `path`, `pid`, `tag`, `timestamp`, `transaction`, `warning`

### apm

apmd Advanced Power Management logs.

Samples: [apm_apmd.log](tests/examples/apm_apmd.log)

Fields: `duration`, `failure`, `host`, `message`, `number`, `pid`, `size`, `success`, `tag`, `timestamp`, `warning`

### auth

Authentication logs from sshd, sudo, su, and PAM.

Samples: [auth_security.log](tests/examples/auth_security.log), [auth_sshd.log](tests/examples/auth_sshd.log), [auth_sudo.log](tests/examples/auth_sudo.log)

Fields: `failure`, `header`, `host`, `ip`, `message`, `module`, `number`, `path`, `pid`, `request`, `success`, `tag`, `timestamp`, `userid`, `warning`

### caddy

Caddy structured JSON logs.

Samples: [caddy_access.log](tests/examples/caddy_access.log), [caddy_error.log](tests/examples/caddy_error.log)

Fields: `duration`, `header`, `host`, `ip`, `level`, `message`, `method`, `number`, `protocol`, `request`, `size`, `status`, `tag`, `timestamp`, `userid`

### ci

Jenkins and GitHub Actions build logs.

Samples: [ci_github.log](tests/examples/ci_github.log), [ci_jenkins.log](tests/examples/ci_jenkins.log), [ci_jenkins_server.log](tests/examples/ci_jenkins_server.log)

Fields: `failure`, `header`, `level`, `message`, `module`, `path`, `pid`, `request`, `success`, `tag`, `timestamp`, `warning`

### cloud-init

cloud-init instance initialization logs.

Samples: [cloud_init.log](tests/examples/cloud_init.log), [cloud_init_output.log](tests/examples/cloud_init_output.log)

Fields: `duration`, `failure`, `header`, `host`, `ip`, `level`, `message`, `module`, `number`, `path`, `pid`, `success`, `tag`, `timestamp`, `warning`

### cron

cron and anacron job logs.

Samples: [cron_file.log](tests/examples/cron_file.log), [cron_syslog.log](tests/examples/cron_syslog.log)

Fields: `failure`, `header`, `host`, `message`, `method`, `module`, `number`, `pid`, `request`, `success`, `tag`, `timestamp`, `userid`, `warning`

### distcc

distccd distributed compilation logs.

Samples: [distcc_logfile.log](tests/examples/distcc_logfile.log)

Fields: `duration`, `failure`, `header`, `ip`, `message`, `module`, `number`, `pid`, `success`, `tag`

### dmesg

Kernel ring buffer messages from dmesg.

Samples: [dmesg_decoded.log](tests/examples/dmesg_decoded.log), [dmesg_default.log](tests/examples/dmesg_default.log)

Fields: `failure`, `host`, `level`, `message`, `module`, `success`, `tag`, `timestamp`, `warning`

### docker

Docker container and daemon logs.

Samples: [compose_stack.log](tests/examples/compose_stack.log), [docker_daemon.log](tests/examples/docker_daemon.log), [docker_json.log](tests/examples/docker_json.log), [docker_timestamps.log](tests/examples/docker_timestamps.log)

Fields: `client`, `failure`, `header`, `host`, `ip`, `level`, `message`, `method`, `module`, `pid`, `protocol`, `referer`, `request`, `size`, `status`, `success`, `tag`, `timestamp`, `transaction`, `user_agent`, `user_identifier`, `userid`, `warning`

### dovecot

Dovecot IMAP, POP3, and LMTP logs.

Samples: [dovecot_file.log](tests/examples/dovecot_file.log), [dovecot_syslog.log](tests/examples/dovecot_syslog.log)

Fields: `failure`, `header`, `host`, `ip`, `level`, `message`, `module`, `pid`, `protocol`, `size`, `success`, `tag`, `timestamp`, `transaction`, `userid`

### elasticsearch

Elasticsearch server and slow logs.

Samples: [elasticsearch_json.log](tests/examples/elasticsearch_json.log), [elasticsearch_server.log](tests/examples/elasticsearch_server.log)

Fields: `duration`, `failure`, `header`, `host`, `ip`, `level`, `message`, `module`, `number`, `request`, `tag`, `timestamp`, `transaction`, `warning`

### exim

Exim mail routing and delivery logs.

Samples: [exim_mainlog.log](tests/examples/exim_mainlog.log)

Fields: `email`, `failure`, `header`, `host`, `ip`, `message`, `module`, `pid`, `protocol`, `queue_id`, `size`, `success`, `timestamp`, `transaction`, `warning`

### fetchmail

Fetchmail email retrieval logs.

Samples: [fetchmail_file.log](tests/examples/fetchmail_file.log), [fetchmail_syslog.log](tests/examples/fetchmail_syslog.log)

Fields: `email`, `failure`, `header`, `host`, `message`, `number`, `pid`, `size`, `success`, `tag`, `timestamp`, `userid`, `warning`

### ftpstats

pure-ftpd ftpstats transfer logs.

Samples: [ftpstats_transfers.log](tests/examples/ftpstats_transfers.log)

Fields: `duration`, `host`, `ip`, `method`, `path`, `size`, `timestamp`, `transaction`, `userid`

### git

git daemon and Git trace logs.

Samples: [git_daemon.log](tests/examples/git_daemon.log), [git_trace.log](tests/examples/git_trace.log)

Fields: `failure`, `host`, `ip`, `message`, `method`, `module`, `number`, `path`, `pid`, `request`, `success`, `tag`, `timestamp`, `warning`

### haproxy

HAProxy connection and error logs.

Samples: [haproxy_error.log](tests/examples/haproxy_error.log), [haproxy_http.log](tests/examples/haproxy_http.log), [haproxy_tcp.log](tests/examples/haproxy_tcp.log)

Fields: `backend`, `client`, `counters`, `frontend`, `header`, `host`, `ip`, `message`, `method`, `pid`, `protocol`, `request`, `server`, `size`, `status`, `tag`, `termination`, `timers`, `timestamp`

### httpd

Apache and nginx access and error logs.

Samples: [httpd_apache_error.log](tests/examples/httpd_apache_error.log), [httpd_combined.log](tests/examples/httpd_combined.log), [httpd_extended.log](tests/examples/httpd_extended.log), [httpd_nginx_error.log](tests/examples/httpd_nginx_error.log), [httpd_vhost_combined.log](tests/examples/httpd_vhost_combined.log), [nginx_production.log](tests/examples/nginx_production.log), [real_apache.log](tests/examples/real_apache.log)

Fields: `client`, `duration`, `ip`, `level`, `message`, `method`, `module`, `pid`, `protocol`, `referer`, `request`, `size`, `status`, `timestamp`, `user_agent`, `user_identifier`, `userid`, `vhost`

### icecast

Icecast streaming server logs.

Samples: [icecast_access.log](tests/examples/icecast_access.log), [icecast_error.log](tests/examples/icecast_error.log), [icecast_v1.log](tests/examples/icecast_v1.log)

Fields: `client`, `duration`, `failure`, `header`, `ip`, `level`, `message`, `method`, `module`, `number`, `protocol`, `referer`, `request`, `size`, `status`, `success`, `tag`, `timestamp`, `user_agent`, `user_identifier`, `userid`, `warning`

### journalctl

systemd journal output from journalctl.

Also named `systemd`.

Samples: [journalctl_follow.log](tests/examples/journalctl_follow.log), [journalctl_precise.log](tests/examples/journalctl_precise.log), [journalctl_short.log](tests/examples/journalctl_short.log)

Fields: `failure`, `header`, `host`, `ip`, `message`, `module`, `pid`, `success`, `tag`, `timestamp`, `transaction`, `warning`

### kubernetes

Kubernetes component and container logs.

Also named `k8s`.

Samples: [k8s_cluster.log](tests/examples/k8s_cluster.log), [kubernetes_cri.log](tests/examples/kubernetes_cri.log), [kubernetes_klog.log](tests/examples/kubernetes_klog.log)

Fields: `client`, `failure`, `header`, `host`, `level`, `message`, `method`, `module`, `number`, `path`, `pid`, `protocol`, `referer`, `request`, `size`, `status`, `tag`, `timestamp`, `transaction`, `user_agent`, `user_identifier`, `userid`, `warning`

### mongodb

MongoDB structured and text logs.

Samples: [mongodb_json.log](tests/examples/mongodb_json.log), [mongodb_text.log](tests/examples/mongodb_text.log)

Fields: `duration`, `failure`, `header`, `host`, `ip`, `level`, `message`, `module`, `number`, `tag`, `timestamp`, `transaction`, `userid`, `warning`

### mysql

MySQL and MariaDB error and slow query logs.

Samples: [mysql_error.log](tests/examples/mysql_error.log), [mysql_slow.log](tests/examples/mysql_slow.log), [mysql_slow_production.log](tests/examples/mysql_slow_production.log)

Fields: `duration`, `failure`, `header`, `host`, `ip`, `level`, `message`, `module`, `number`, `pid`, `request`, `success`, `timestamp`, `transaction`, `userid`, `warning`

### nginx-error

nginx error logs.

Samples: [nginx_error.log](tests/examples/nginx_error.log)

Fields: `failure`, `header`, `host`, `ip`, `level`, `message`, `module`, `number`, `path`, `pid`, `referer`, `request`, `timestamp`, `transaction`, `vhost`, `warning`

### oops

Linux kernel oops, warning, and panic messages.

Samples: [oops_kernel.log](tests/examples/oops_kernel.log)

Fields: `failure`, `header`, `message`, `module`, `number`, `tag`, `timestamp`

### php

PHP error logs and PHP-FPM logs.

Samples: [php_error.log](tests/examples/php_error.log), [php_fpm.log](tests/examples/php_fpm.log)

Fields: `failure`, `header`, `level`, `message`, `module`, `number`, `path`, `pid`, `success`, `tag`, `timestamp`, `warning`

### postfix

Postfix mail queue, SMTP, and delivery logs.

Samples: [postfix_delivery.log](tests/examples/postfix_delivery.log), [postfix_failures.log](tests/examples/postfix_failures.log)

Fields: `duration`, `email`, `failure`, `header`, `host`, `ip`, `message`, `number`, `pid`, `protocol`, `queue_id`, `size`, `status`, `success`, `tag`, `timers`, `timestamp`, `transaction`, `warning`

### postgresql

PostgreSQL server logs and slow statements.

Samples: [postgresql_debian.log](tests/examples/postgresql_debian.log), [postgresql_default.log](tests/examples/postgresql_default.log), [postgresql_slow.log](tests/examples/postgresql_slow.log)

Fields: `duration`, `failure`, `header`, `ip`, `level`, `message`, `module`, `pid`, `request`, `timestamp`, `userid`, `warning`

### procmail

Procmail mail filtering and delivery logs.

Samples: [procmail_abstract.log](tests/examples/procmail_abstract.log), [procmail_verbose.log](tests/examples/procmail_verbose.log)

Fields: `email`, `failure`, `header`, `message`, `path`, `pid`, `size`, `success`, `tag`, `timestamp`, `warning`

### proftpd

ProFTPD system and extended logs.

Samples: [proftpd_extended.log](tests/examples/proftpd_extended.log), [proftpd_syslog.log](tests/examples/proftpd_syslog.log), [proftpd_system.log](tests/examples/proftpd_system.log)

Fields: `failure`, `host`, `ip`, `message`, `method`, `pid`, `request`, `size`, `status`, `success`, `tag`, `timestamp`, `user_identifier`, `userid`, `vhost`

### pure-ftpd

pure-ftpd syslog and transfer logs.

Samples: [pureftpd_altlog.log](tests/examples/pureftpd_altlog.log), [pureftpd_syslog.log](tests/examples/pureftpd_syslog.log)

Fields: `failure`, `header`, `host`, `ip`, `level`, `message`, `method`, `number`, `path`, `request`, `size`, `status`, `success`, `tag`, `timestamp`, `user_identifier`, `userid`, `warning`

### redis

Redis server logs.

Samples: [redis_server.log](tests/examples/redis_server.log)

Fields: `ip`, `level`, `message`, `module`, `pid`, `success`, `timestamp`, `warning`

### squid

Squid access logs, native and httpd-emulated.

Samples: [squid_emulated.log](tests/examples/squid_emulated.log), [squid_native.log](tests/examples/squid_native.log)

Fields: `cache_hit`, `cache_miss`, `cache_result`, `client`, `content_type`, `duration`, `hierarchy`, `method`, `protocol`, `request`, `size`, `status`, `timestamp`, `user_identifier`, `userid`

### ssh

OpenSSH server logs.

Samples: [ssh_sshd.log](tests/examples/ssh_sshd.log)

Fields: `failure`, `header`, `host`, `ip`, `message`, `module`, `number`, `pid`, `protocol`, `success`, `tag`, `timestamp`, `transaction`, `userid`, `warning`

### sudo

sudo and su privilege escalation logs.

Samples: [sudo_logfile.log](tests/examples/sudo_logfile.log), [sudo_syslog.log](tests/examples/sudo_syslog.log)

Fields: `failure`, `header`, `host`, `message`, `module`, `number`, `path`, `pid`, `request`, `success`, `tag`, `timestamp`, `userid`

### sulog

System V su command logs.

Samples: [sulog_solaris.log](tests/examples/sulog_solaris.log)

Fields: `failure`, `path`, `success`, `tag`, `timestamp`, `userid`

### super

super(1) superuser access logs.

Samples: [super_logfile.log](tests/examples/super_logfile.log)

Fields: `host`, `message`, `method`, `pid`, `request`, `tag`, `timestamp`, `userid`

### syslog

Syslog lines from any program, including RFC 5424.

Samples: [multi_service_syslog.log](tests/examples/multi_service_syslog.log), [real_syslog.log](tests/examples/real_syslog.log), [syslog_rfc5424.log](tests/examples/syslog_rfc5424.log), [syslog_traditional.log](tests/examples/syslog_traditional.log)

Fields: `failure`, `header`, `host`, `ip`, `level`, `message`, `module`, `number`, `pid`, `tag`, `timestamp`, `transaction`, `warning`

### systemd-resolved

systemd-resolved DNS resolution logs.

Samples: [resolved_journal.log](tests/examples/resolved_journal.log)

Fields: `failure`, `header`, `host`, `ip`, `message`, `pid`, `protocol`, `success`, `tag`, `timestamp`, `warning`

### terraform

Terraform logs, JSON output, and CLI output.

Samples: [terraform_cli.log](tests/examples/terraform_cli.log), [terraform_log.log](tests/examples/terraform_log.log)

Fields: `duration`, `failure`, `header`, `level`, `message`, `method`, `module`, `number`, `success`, `tag`, `timestamp`, `transaction`, `warning`

### ulogd

Netfilter packet logs from ulogd and the kernel.

Samples: [ulogd_kernel.log](tests/examples/ulogd_kernel.log), [ulogd_logemu.log](tests/examples/ulogd_logemu.log)

Fields: `failure`, `header`, `host`, `ip`, `message`, `module`, `number`, `protocol`, `size`, `success`, `tag`, `timestamp`, `userid`

### varnish

Varnish request and response logs.

Samples: [varnish_raw.log](tests/examples/varnish_raw.log), [varnish_transaction.log](tests/examples/varnish_transaction.log)

Fields: `header`, `ip`, `message`, `method`, `number`, `protocol`, `request`, `status`, `tag`, `timestamp`, `transaction`

### vsftpd

vsftpd FTP server logs.

Samples: [vsftpd_file.log](tests/examples/vsftpd_file.log), [vsftpd_syslog.log](tests/examples/vsftpd_syslog.log)

Fields: `failure`, `header`, `host`, `ip`, `message`, `method`, `number`, `path`, `pid`, `request`, `size`, `status`, `success`, `tag`, `timestamp`, `user_identifier`, `userid`

### xferlog

Generic xferlog FTP transfer logs.

Samples: [xferlog_transfers.log](tests/examples/xferlog_transfers.log)

Fields: `duration`, `failure`, `host`, `ip`, `level`, `method`, `module`, `number`, `path`, `protocol`, `size`, `success`, `tag`, `timestamp`, `user_identifier`, `userid`
