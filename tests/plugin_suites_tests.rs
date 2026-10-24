use splash::output::TokenKind;
use splash::registry;
use std::fs;
use std::path::PathBuf;

/// Every built-in plugin with the sample logs written wholly in its format
const SUITES: &[(&str, &[&str])] = &[
    ("apache-error", &["apache_error.log"]),
    ("apm", &["apm_apmd.log"]),
    (
        "auth",
        &["auth_security.log", "auth_sshd.log", "auth_sudo.log"],
    ),
    ("caddy", &["caddy_access.log", "caddy_error.log"]),
    (
        "ci",
        &["ci_github.log", "ci_jenkins.log", "ci_jenkins_server.log"],
    ),
    ("cloud-init", &["cloud_init.log", "cloud_init_output.log"]),
    ("cron", &["cron_file.log", "cron_syslog.log"]),
    ("distcc", &["distcc_logfile.log"]),
    ("dmesg", &["dmesg_decoded.log", "dmesg_default.log"]),
    (
        "docker",
        &[
            "compose_stack.log",
            "docker_daemon.log",
            "docker_json.log",
            "docker_timestamps.log",
        ],
    ),
    ("dovecot", &["dovecot_file.log", "dovecot_syslog.log"]),
    (
        "elasticsearch",
        &["elasticsearch_json.log", "elasticsearch_server.log"],
    ),
    ("exim", &["exim_mainlog.log"]),
    ("fetchmail", &["fetchmail_file.log", "fetchmail_syslog.log"]),
    ("ftpstats", &["ftpstats_transfers.log"]),
    ("git", &["git_daemon.log", "git_trace.log"]),
    (
        "haproxy",
        &["haproxy_error.log", "haproxy_http.log", "haproxy_tcp.log"],
    ),
    (
        "httpd",
        &[
            "httpd_apache_error.log",
            "httpd_combined.log",
            "httpd_extended.log",
            "httpd_nginx_error.log",
            "httpd_vhost_combined.log",
            "nginx_production.log",
            "real_apache.log",
        ],
    ),
    (
        "icecast",
        &["icecast_access.log", "icecast_error.log", "icecast_v1.log"],
    ),
    (
        "journalctl",
        &[
            "journalctl_follow.log",
            "journalctl_precise.log",
            "journalctl_short.log",
        ],
    ),
    (
        "kubernetes",
        &[
            "k8s_cluster.log",
            "kubernetes_cri.log",
            "kubernetes_klog.log",
        ],
    ),
    ("mongodb", &["mongodb_json.log", "mongodb_text.log"]),
    (
        "mysql",
        &[
            "mysql_error.log",
            "mysql_slow.log",
            "mysql_slow_production.log",
        ],
    ),
    ("nginx-error", &["nginx_error.log"]),
    ("oops", &["oops_kernel.log"]),
    ("php", &["php_error.log", "php_fpm.log"]),
    ("postfix", &["postfix_delivery.log", "postfix_failures.log"]),
    (
        "postgresql",
        &[
            "postgresql_debian.log",
            "postgresql_default.log",
            "postgresql_slow.log",
        ],
    ),
    (
        "procmail",
        &["procmail_abstract.log", "procmail_verbose.log"],
    ),
    (
        "proftpd",
        &[
            "proftpd_extended.log",
            "proftpd_syslog.log",
            "proftpd_system.log",
        ],
    ),
    ("pure-ftpd", &["pureftpd_altlog.log", "pureftpd_syslog.log"]),
    ("redis", &["redis_server.log"]),
    ("squid", &["squid_emulated.log", "squid_native.log"]),
    ("ssh", &["ssh_sshd.log"]),
    ("sudo", &["sudo_logfile.log", "sudo_syslog.log"]),
    ("sulog", &["sulog_solaris.log"]),
    ("super", &["super_logfile.log"]),
    (
        "syslog",
        &[
            "multi_service_syslog.log",
            "real_syslog.log",
            "syslog_rfc5424.log",
            "syslog_traditional.log",
        ],
    ),
    ("systemd-resolved", &["resolved_journal.log"]),
    ("terraform", &["terraform_cli.log", "terraform_log.log"]),
    ("ulogd", &["ulogd_kernel.log", "ulogd_logemu.log"]),
    ("varnish", &["varnish_raw.log", "varnish_transaction.log"]),
    ("vsftpd", &["vsftpd_file.log", "vsftpd_syslog.log"]),
    ("xferlog", &["xferlog_transfers.log"]),
];

fn sample(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("examples")
        .join(name);

    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {}", path.display(), error))
}

fn annotated(line: &str, mode: &str) -> String {
    splash::parser::parse_line(line, mode)
        .unwrap_or_else(|| panic!("{} did not read '{}'", mode, line))
        .tokens
        .iter()
        .map(|token| match token.kind {
            TokenKind::Plain => token.text.to_string(),
            kind => format!("{{{}:{}}}", kind.name(), token.text),
        })
        .collect()
}

/// The first line of each plugin's first sample, as the plugin's mode reads
/// it: plain text as written, and every other token as `{kind:text}`
const GOLDEN: &[(&str, &str, &str)] = &[
    ("apache-error", "apache_error.log", "{punctuation:[}{timestamp:Wed Oct 11 14:32:52.123456 2023}{punctuation:]} {punctuation:[}{module:core}{punctuation::}{failure:error}{punctuation:]} {punctuation:[}{header:pid} {pid:35708}{punctuation::}{header:tid} {pid:4328636416}{punctuation:]} {punctuation:[}{header:client} {ip:72.15.99.187}{punctuation::}{number:1234}{punctuation:]} {transaction:AH00128}{punctuation::}{message: File does not exist: /var/www/favicon.ico}"),
    ("apm", "apm_apmd.log", "{timestamp:Oct  3 12:00:00} {host:laptop} {tag:apmd}{punctuation:[}{pid:800}{punctuation:]}{punctuation::} {message:Battery: }{size:87%}{message:, }{warning:discharging}{message: (}{number:-0.50%/min}{message: over }{duration:0:10:00}{message:), }{duration:1:23:45}{message: (}{duration:2:54:00}{message:) to empty}"),
    ("auth", "auth_security.log", "{timestamp:Oct  3 03:12:01} {host:web01} {tag:sshd}{punctuation:[}{pid:5101}{punctuation:]}{punctuation::} {failure:Invalid user} {userid:oracle}{message: from }{ip:203.0.113.50}{message: }{message:port} {number:40122}"),
    ("caddy", "caddy_access.log", "{punctuation:{}{punctuation:\"}{header:level}{punctuation:\"}{punctuation::}{punctuation:\"}{level:info}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:ts}{punctuation:\"}{punctuation::}{timestamp:1646861401.5241024}{punctuation:,}{punctuation:\"}{header:logger}{punctuation:\"}{punctuation::}{punctuation:\"}{tag:http.log.access}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:msg}{punctuation:\"}{punctuation::}{punctuation:\"}{message:handled request}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:request}{punctuation:\"}{punctuation::}{punctuation:{}{punctuation:\"}{header:remote_ip}{punctuation:\"}{punctuation::}{punctuation:\"}{ip:127.0.0.1}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:remote_port}{punctuation:\"}{punctuation::}{punctuation:\"}41342{punctuation:\"}{punctuation:,}{punctuation:\"}{header:client_ip}{punctuation:\"}{punctuation::}{punctuation:\"}{ip:127.0.0.1}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:proto}{punctuation:\"}{punctuation::}{punctuation:\"}{protocol:HTTP/2.0}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:method}{punctuation:\"}{punctuation::}{punctuation:\"}{method:GET}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:host}{punctuation:\"}{punctuation::}{punctuation:\"}{host:localhost}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:uri}{punctuation:\"}{punctuation::}{punctuation:\"}{request:/}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:headers}{punctuation:\"}{punctuation::}{punctuation:{}{punctuation:\"}{header:User-Agent}{punctuation:\"}{punctuation::}{punctuation:[}{punctuation:\"}curl/7.82.0{punctuation:\"}{punctuation:]}{punctuation:,}{punctuation:\"}{header:Accept}{punctuation:\"}{punctuation::}{punctuation:[}{punctuation:\"}*/*{punctuation:\"}{punctuation:]}{punctuation:}}{punctuation:,}{punctuation:\"}{header:tls}{punctuation:\"}{punctuation::}{punctuation:{}{punctuation:\"}{header:resumed}{punctuation:\"}{punctuation::}false{punctuation:,}{punctuation:\"}{header:version}{punctuation:\"}{punctuation::}{number:772}{punctuation:,}{punctuation:\"}{header:cipher_suite}{punctuation:\"}{punctuation::}{number:4865}{punctuation:,}{punctuation:\"}{header:proto}{punctuation:\"}{punctuation::}{punctuation:\"}{protocol:h2}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:server_name}{punctuation:\"}{punctuation::}{punctuation:\"}{host:localhost}{punctuation:\"}{punctuation:}}{punctuation:}}{punctuation:,}{punctuation:\"}{header:bytes_read}{punctuation:\"}{punctuation::}{size:0}{punctuation:,}{punctuation:\"}{header:user_id}{punctuation:\"}{punctuation::}{punctuation:\"}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:duration}{punctuation:\"}{punctuation::}{duration:0.000929675}{punctuation:,}{punctuation:\"}{header:size}{punctuation:\"}{punctuation::}{size:10900}{punctuation:,}{punctuation:\"}{header:status}{punctuation:\"}{punctuation::}{status:200}{punctuation:,}{punctuation:\"}{header:resp_headers}{punctuation:\"}{punctuation::}{punctuation:{}{punctuation:\"}{header:Server}{punctuation:\"}{punctuation::}{punctuation:[}{punctuation:\"}Caddy{punctuation:\"}{punctuation:]}{punctuation:,}{punctuation:\"}{header:Content-Type}{punctuation:\"}{punctuation::}{punctuation:[}{punctuation:\"}text/html; charset=utf-8{punctuation:\"}{punctuation:]}{punctuation:}}{punctuation:}}"),
    ("ci", "ci_github.log", "{timestamp:2023-10-03T12:00:01.1234567Z} {punctuation:##[}{header:group}{punctuation:]}{message:Run actions/checkout@v4}"),
    ("cloud-init", "cloud_init.log", "{timestamp:2023-10-03 12:00:01,123}{punctuation: - }{path:util.py}{punctuation:[}{level:DEBUG}{punctuation:]}{punctuation:: }{tag:Cloud-init v. }{number:23.3.1}{message: running }{punctuation:'}{module:init-local}{punctuation:'}{message: at Tue, 03 Oct 2023 12:00:01 +0000. }{message:Up }{duration:5.12}{message: seconds}{message:.}"),
    ("cron", "cron_file.log", "{userid:root} {punctuation:(}{timestamp:10/03-12:00:01}{punctuation:-}{pid:5101}{punctuation:)} {method:CMD} {punctuation:(}{request:run-parts /etc/cron.hourly}{punctuation:)}"),
    ("distcc", "distcc_logfile.log", "{tag:distccd}{punctuation:[}{pid:5100}{punctuation:]} {punctuation:(}{module:dcc_listen_by_addr}{punctuation:)} {message:listening on }{ip:0.0.0.0}{punctuation::}{number:3632}"),
    ("dmesg", "dmesg_decoded.log", "{module:kern}  {punctuation::}{level:info}  {punctuation::} {timestamp:[    1.234567]} {module:usb 1-1}{punctuation::} {message:new high-speed USB device number 2 using xhci_hcd}"),
    ("docker", "compose_stack.log", "{module:db-1}{punctuation:     | }{timestamp:2023-10-03 12:00:00.123 UTC} {punctuation:[}{pid:1}{punctuation:]} {level:LOG}{punctuation::  }{message:database system is ready to accept connections}"),
    ("dovecot", "dovecot_file.log", "{timestamp:Oct 03 12:00:01} {module:imap-login}{punctuation::} {level:Info}{punctuation::} {success:Login}{message:: }{header:user}{punctuation:=}{punctuation:<}{userid:alice}{punctuation:>}{message:, }{header:method}{punctuation:=}{protocol:PLAIN}{message:, }{header:rip}{punctuation:=}{ip:10.0.0.5}{message:, }{header:lip}{punctuation:=}{ip:10.0.0.1}{message:, }{header:mpid}{punctuation:=}{pid:4321}{message:, TLS, }{header:session}{punctuation:=}{punctuation:<}{transaction:Xy7AbC}{punctuation:>}"),
    ("elasticsearch", "elasticsearch_json.log", "{punctuation:{}{punctuation:\"}{header:type}{punctuation:\"}{punctuation::} {punctuation:\"}{tag:server}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:timestamp}{punctuation:\"}{punctuation::} {punctuation:\"}{timestamp:2023-10-03T12:00:01,123Z}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:level}{punctuation:\"}{punctuation::} {punctuation:\"}{level:INFO}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:component}{punctuation:\"}{punctuation::} {punctuation:\"}{module:o.e.n.Node}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:cluster.name}{punctuation:\"}{punctuation::} {punctuation:\"}{tag:es-prod}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:node.name}{punctuation:\"}{punctuation::} {punctuation:\"}{host:node-1}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:message}{punctuation:\"}{punctuation::} {punctuation:\"}{message:started}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:cluster.uuid}{punctuation:\"}{punctuation::} {punctuation:\"}{transaction:Xy7AbC12Qz}{punctuation:\"}{punctuation:,} {punctuation:\"}{header:node.id}{punctuation:\"}{punctuation::} {punctuation:\"}{transaction:Ab12Cd34Ef}{punctuation:\"}{punctuation:}}"),
    ("exim", "exim_mainlog.log", "{timestamp:2023-10-03 12:00:01} {queue_id:1qnXYZ-000ABC-12} {success:<=} {email:alice@example.com}{message: }{header:H}{punctuation:=}{host:mail.example.com}{message: [}{ip:10.0.0.5}{message:] }{header:P}{punctuation:=}{protocol:esmtps}{message: }{header:X}{punctuation:=}{protocol:TLS1.3:TLS_AES_256_GCM_SHA384:256}{message: }{header:S}{punctuation:=}{size:4512}{message: }{header:id}{punctuation:=}{transaction:20231003120001.abc@example.com}"),
    ("fetchmail", "fetchmail_file.log", "{tag:fetchmail}{punctuation::} {number:3}{message: messages (}{number:1}{message: seen) }{message:for }{userid:alice}{message: at }{host:mail.example.com}{message: (}{size:12345}{message: octets}{message:).}"),
    ("ftpstats", "ftpstats_transfers.log", "{timestamp:1696334401} {transaction:651c0b41.10e1} {userid:alice} {ip:10.0.0.5} {method:D} {size:4096} {duration:2} {path:/home/alice/report.pdf}"),
    ("git", "git_daemon.log", "{timestamp:Oct  3 12:00:00} {host:git01} {tag:git-daemon}{punctuation:[}{pid:3000}{punctuation:]}{punctuation::} {success:Ready to rumble}"),
    ("haproxy", "haproxy_error.log", "{timestamp:Feb  6 12:12:56} {host:gateway} {tag:haproxy}{punctuation:[}{pid:14387}{punctuation:]:} {client:127.0.0.1:34550} {timestamp:[06/Feb/2009:12:12:51.443]} {frontend:frt}{punctuation:/}{server:f1}{punctuation::}{message: invalid request}"),
    ("httpd", "httpd_apache_error.log", "{punctuation:[}{timestamp:Wed Oct 11 14:32:52.123456 2023}{punctuation:]} {punctuation:[}{module:core}{punctuation::}{level:error}{punctuation:]} {pid:[pid 35708:tid 4328636416]} {client:[client 72.15.99.187:1234]} {message:AH00128: File does not exist: /var/www/favicon.ico}"),
    ("icecast", "icecast_access.log", "{client:10.0.0.5} {user_identifier:-} {userid:-} {timestamp:[03/Oct/2023:12:00:01 +0000]} {punctuation:\"}{method:GET} {request:/live.mp3} {protocol:HTTP/1.1}{punctuation:\"} {status:200} {size:3456789} {punctuation:\"}{referer:-}{punctuation:\"} {punctuation:\"}{user_agent:VLC/3.0.18 LibVLC/3.0.18}{punctuation:\"} {duration:3600}"),
    ("journalctl", "journalctl_follow.log", "{timestamp:Oct 03 12:00:01} {host:web01} {tag:systemd}{punctuation:[}{pid:1}{punctuation:]}{punctuation::} {message:Starting }{module:nginx.service}{message: - A high performance web server...}"),
    ("kubernetes", "k8s_cluster.log", "{module:[pod/web-7d4b9c6f5-abcde/nginx]} {client:10.244.0.1} {user_identifier:-} {userid:-} {timestamp:[03/Oct/2023:12:00:01 +0000]} {punctuation:\"}{method:GET} {request:/healthz} {protocol:HTTP/1.1}{punctuation:\"} {status:200} {size:2} {punctuation:\"}{referer:-}{punctuation:\"} {punctuation:\"}{user_agent:kube-probe/1.28}{punctuation:\"}"),
    ("mongodb", "mongodb_json.log", "{punctuation:{}{punctuation:\"}{header:t}{punctuation:\"}{punctuation::}{punctuation:{}{punctuation:\"}{header:$date}{punctuation:\"}{punctuation::}{punctuation:\"}{timestamp:2023-10-03T12:00:01.123+00:00}{punctuation:\"}{punctuation:}}{punctuation:,}{punctuation:\"}{header:s}{punctuation:\"}{punctuation::}{punctuation:\"}{level:I}{punctuation:\"}{punctuation:,}  {punctuation:\"}{header:c}{punctuation:\"}{punctuation::}{punctuation:\"}{module:NETWORK}{punctuation:\"}{punctuation:,}  {punctuation:\"}{header:id}{punctuation:\"}{punctuation::}{transaction:23016}{punctuation:,}   {punctuation:\"}{header:ctx}{punctuation:\"}{punctuation::}{punctuation:\"}{tag:listener}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:msg}{punctuation:\"}{punctuation::}{punctuation:\"}{message:Waiting for connections}{punctuation:\"}{punctuation:,}{punctuation:\"}{header:attr}{punctuation:\"}{punctuation::}{punctuation:{}{punctuation:\"}{header:port}{punctuation:\"}{punctuation::}{number:27017}{punctuation:,}{punctuation:\"}{header:ssl}{punctuation:\"}{punctuation::}{punctuation:\"}off{punctuation:\"}{punctuation:}}{punctuation:}}"),
    ("mysql", "mysql_error.log", "{timestamp:2023-10-03T12:00:01.123456Z} {pid:0} {punctuation:[}{level:System}{punctuation:]} {punctuation:[}{transaction:MY-010116}{punctuation:]} {punctuation:[}{module:Server}{punctuation:]}{message: /usr/sbin/mysqld (mysqld 8.0.34) starting as process 1234}"),
    ("nginx-error", "nginx_error.log", "{timestamp:2023/10/03 12:00:01} {punctuation:[}{failure:error}{punctuation:]} {pid:1234}{punctuation:#}{pid:5678}{punctuation:: }{punctuation:*}{transaction:42} {module:open()}{message: }{punctuation:\"}{path:/usr/share/nginx/html/favicon.ico}{punctuation:\"}{message: }{failure:failed}{message: }{punctuation:(}{number:2}{punctuation:: }{failure:No such file or directory}{punctuation:)}{punctuation:, }{header:client}{punctuation:: }{ip:10.0.0.5}{punctuation:, }{header:server}{punctuation:: }{vhost:example.com}{punctuation:, }{header:request}{punctuation:: }{punctuation:\"}{request:GET /favicon.ico HTTP/1.1}{punctuation:\"}{punctuation:, }{header:host}{punctuation:: }{punctuation:\"}{host:example.com}{punctuation:\"}{punctuation:, }{header:referrer}{punctuation:: }{punctuation:\"}{referer:https://example.com/}{punctuation:\"}"),
    ("oops", "oops_kernel.log", "{timestamp:[ 1234.567890]} {failure:BUG}{message:: }{failure:kernel NULL pointer dereference}{message:, }{header:address}{punctuation::}{message: }{number:0000000000000008}"),
    ("php", "php_error.log", "{punctuation:[}{timestamp:03-Oct-2023 12:00:01 UTC}{punctuation:]} {tag:PHP} {warning:Warning}{punctuation::}{message:  Undefined variable $total }{message:in} {path:/var/www/html/cart.php} {message:on line} {number:42}"),
    ("postfix", "postfix_delivery.log", "{timestamp:Oct  3 12:00:01} {host:mail} {tag:postfix/smtpd}{punctuation:[}{pid:1234}{punctuation:]}{punctuation::} {message:connect from unknown[}{ip:10.0.0.5}{message:]}"),
    ("postgresql", "postgresql_debian.log", "{timestamp:2023-10-03 12:00:01.123 UTC} {punctuation:[}{pid:1234}{punctuation:]} {level:LOG}{punctuation::  }{message:checkpoint starting: time}"),
    ("procmail", "procmail_abstract.log", "{header:From} {email:alice@example.com}  {timestamp:Tue Oct  3 12:00:01 2023}"),
    ("proftpd", "proftpd_extended.log", "{ip:10.0.0.5} {user_identifier:-} {userid:alice} {timestamp:[03/Oct/2023:12:00:01 +0000]} {punctuation:\"}{method:USER} {request:alice}{punctuation:\"} {status:331} {size:-}"),
    ("pure-ftpd", "pureftpd_altlog.log", "{punctuation:#}{header:Software}{punctuation::}{message: Pure-FTPd 1.0.51}"),
    ("redis", "redis_server.log", "{pid:1234}{punctuation::}{module:C} {timestamp:03 Oct 2023 12:00:00.001} {warning:#}{message: oO0OoO0OoO0Oo Redis is starting oO0OoO0OoO0Oo}"),
    ("squid", "squid_emulated.log", "{client:10.0.0.5} {user_identifier:-} {userid:-} {timestamp:[09/Mar/2021:12:00:00 +0000]} {punctuation:\"}{method:GET} {request:http://example.com/} {protocol:HTTP/1.1}{punctuation:\"} {status:200} {size:4512} {cache_miss:TCP_MISS}{punctuation::}{hierarchy:HIER_DIRECT}"),
    ("ssh", "ssh_sshd.log", "{timestamp:Oct  3 12:00:00} {host:web01} {tag:sshd}{punctuation:[}{pid:700}{punctuation:]}{punctuation::} {success:Server listening on}{message: }{ip:0.0.0.0}{message: }{message:port} {number:22}{message:.}"),
    ("sudo", "sudo_logfile.log", "{timestamp:Oct  3 12:00:05}{punctuation: : }{userid:alice}{punctuation: : }{header:TTY}{punctuation:=}{message:pts/0}{message: ; }{header:PWD}{punctuation:=}{path:/home/alice}{message: ; }{header:USER}{punctuation:=}{userid:root}{message: ; }{header:COMMAND}{punctuation:=}{request:/usr/bin/apt update}"),
    ("sulog", "sulog_solaris.log", "{tag:SU} {timestamp:10/03 12:00} {success:+} {path:pts/1} {userid:alice}{punctuation:-}{userid:root}"),
    ("super", "super_logfile.log", "{userid:alice}{punctuation:@}{host:web01} {timestamp:Tue Oct  3 12:00:01 2023}\t{method:shutdown} {punctuation:(}{request:-h now}{punctuation:)}"),
    ("syslog", "multi_service_syslog.log", "{timestamp:Oct  3 12:00:01} {host:web01} {tag:systemd}{punctuation:[}{pid:1}{punctuation:]}{punctuation::} {message:Starting nginx.service - A high performance web server...}"),
    ("systemd-resolved", "resolved_journal.log", "{timestamp:Oct 03 12:00:00} {host:web01} {tag:systemd-resolved}{punctuation:[}{pid:600}{punctuation:]}{punctuation::} {header:Positive Trust Anchors}{message::}"),
    ("terraform", "terraform_cli.log", "  {punctuation:#} {message:aws_instance.web }{success:will be created}"),
    ("ulogd", "ulogd_kernel.log", "{timestamp:Oct  3 12:00:01} {host:fw01} {tag:kernel}{punctuation::} {timestamp:[12345.678901]} {message:[UFW }{failure:BLOCK}{message:] }{header:IN}{punctuation:=}{module:eth0} {header:OUT}{punctuation:=} {header:MAC}{punctuation:=}{message:52:54:00:12:34:56:52:54:00:65:43:21:08:00} {header:SRC}{punctuation:=}{ip:203.0.113.7} {header:DST}{punctuation:=}{ip:10.0.0.1} {header:LEN}{punctuation:=}{size:60} {header:TOS}{punctuation:=}{number:0x00} {header:PREC}{punctuation:=}{number:0x00} {header:TTL}{punctuation:=}{number:52} {header:ID}{punctuation:=}{number:54321} {tag:DF} {header:PROTO}{punctuation:=}{protocol:TCP} {header:SPT}{punctuation:=}{number:40022} {header:DPT}{punctuation:=}{number:22} {header:WINDOW}{punctuation:=}{number:64240} {header:RES}{punctuation:=}{number:0x00} {tag:SYN} {header:URGP}{punctuation:=}{number:0}"),
    ("varnish", "varnish_raw.log", "        {number:32770} {tag:Begin}          {transaction:c} {message:req 32769 rxreq}"),
    ("vsftpd", "vsftpd_file.log", "{timestamp:Tue Oct  3 12:00:01 2023} {punctuation:[}{header:pid} {pid:4321}{punctuation:]} {method:CONNECT}{punctuation::} {header:Client} {punctuation:\"}{ip:10.0.0.5}{punctuation:\"}"),
    ("xferlog", "xferlog_transfers.log", "{timestamp:Tue Oct  3 12:00:01 2023} {duration:2} {ip:10.0.0.5} {size:4096} {path:/home/alice/report.pdf} {protocol:b} {tag:_} {method:o} {level:r} {userid:alice} {module:ftp} {number:0} {user_identifier:*} {success:c}"),
];

fn lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

#[test]
fn every_built_in_plugin_has_a_suite() {
    let mut registered = registry::builtins().list_plugins().unwrap();
    registered.sort();

    let suites: Vec<&str> = SUITES.iter().map(|(name, _)| *name).collect();

    assert_eq!(suites, registered);
}

#[test]
fn every_built_in_plugin_has_a_golden_line() {
    let goldens: Vec<&str> = GOLDEN.iter().map(|(name, _, _)| *name).collect();
    let suites: Vec<&str> = SUITES.iter().map(|(name, _)| *name).collect();

    assert_eq!(goldens, suites);
}

#[test]
fn each_plugin_reads_every_line_of_its_samples() {
    for (name, files) in SUITES {
        let plugin = registry::builtins().get(name).unwrap();

        for file in *files {
            for line in lines(&sample(file)) {
                assert!(
                    plugin.can_parse(line),
                    "{} did not read this line of {}: {}",
                    name,
                    file,
                    line
                );
            }
        }
    }
}

#[test]
fn each_plugin_keeps_every_character_of_its_sample_lines() {
    for (name, files) in SUITES {
        for file in *files {
            for line in lines(&sample(file)) {
                let parsed = splash::parser::parse_line(line, name).unwrap();
                let joined: String = parsed
                    .tokens
                    .iter()
                    .map(|token| token.text.as_ref())
                    .collect();

                assert_eq!(joined, line, "{} in {}", name, file);
            }
        }
    }
}

#[test]
fn each_plugin_styles_a_field_in_every_line_of_its_samples() {
    for (name, files) in SUITES {
        for file in *files {
            for line in lines(&sample(file)) {
                let parsed = splash::parser::parse_line(line, name).unwrap();

                assert!(
                    parsed
                        .tokens
                        .iter()
                        .any(|token| token.kind != TokenKind::Plain),
                    "{} styled nothing in this line of {}: {}",
                    name,
                    file,
                    line
                );
            }
        }
    }
}

#[test]
fn each_plugin_reads_the_first_line_of_its_first_sample_as_recorded() {
    for (name, file, expected) in GOLDEN {
        let text = sample(file);
        let first = lines(&text)[0];

        assert_eq!(annotated(first, name), *expected, "{} in {}", name, file);
    }
}
