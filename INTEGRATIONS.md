# Using splash with Docker, Kubernetes, and systemd

This guide shows how to read container and service logs with splash, and how to run splash as a
systemd service that collects the logs of several hosts. The options it uses are described in the
[README](README.md).

- [Docker](#docker)
- [Kubernetes](#kubernetes)
- [systemd](#systemd)

---

## Docker

`docker logs` prints the lines a container wrote. Docker mode colors the time `--timestamps` adds,
and colors each line after it with the mode that reads that line's format:

```bash
docker logs -f --timestamps web | splash --mode docker
```

`docker compose logs` starts each line with the container's name, which docker mode colors too:

```bash
docker compose logs -f --timestamps | splash --mode docker
```

With the default `json-file` logging driver, Docker keeps each container's log in a file on the
host. `docker inspect` names the file, and reading it needs root:

```bash
splash --mode docker --follow --path "$(docker inspect --format '{{.LogPath}}' web)"
```

The `syslog` logging driver sends a container's lines to a syslog receiver, which splash can be:

```bash
splash --syslog :5514
```

```bash
docker run --log-driver syslog \
  --log-opt syslog-address=udp://127.0.0.1:5514 \
  --log-opt syslog-format=rfc5424 \
  --log-opt tag=web \
  nginx
```

splash reads the messages in syslog mode, which colors the RFC 5424 header and the message after
it.

## Kubernetes

`kubectl logs` prints a container's lines. With `--timestamps` and `--prefix`, each line starts with
the time and the pod and container it came from, and k8s mode colors both:

```bash
kubectl logs -f --prefix --timestamps deploy/web | splash --mode k8s
```

`-l` reads every pod that has a label. kubectl follows at most 5 pods at once unless
`--max-log-requests` raises the limit:

```bash
kubectl logs -f -l app=web --prefix --timestamps --max-log-requests 20 | splash --mode k8s
```

The control plane components write klog lines, which k8s mode colors by level:

```bash
kubectl logs -n kube-system kube-scheduler-node01 | splash --mode kubernetes
```

On a node, the kubelet keeps each container's log in CRI format, and `/var/log/containers` holds a
link to each one, named for its pod, namespace, and container. `--merge` interleaves them in time
order, and `--label` starts each line with the file it came from:

```bash
splash --mode k8s --merge --label --path '/var/log/containers/*.log'
```

```
web-7d9c_default_nginx-3f2a.log 2023-10-03T12:00:01.000000000Z stdout F GET /health 200
api-5b8f_default_api-9c1e.log 2023-10-03T12:00:02.000000000Z stderr F timeout calling db
web-7d9c_default_nginx-3f2a.log 2023-10-03T12:00:04.000000000Z stdout F GET /orders 200
```

## systemd

journalctl mode reads `journalctl` output, and `--auto` chooses it from the first lines:

```bash
journalctl -f | splash --auto
journalctl -u nginx -f | splash --mode journalctl
```

journalctl's own options select entries before splash reads them, and splash's filters and
statistics work on what it receives. This writes the boot's errors followed by a summary:

```bash
journalctl -b -p err -o short-iso | splash --mode journalctl --summary
```

### Forward each host's errors to a collecting host

A unit on each host runs `journalctl` and forwards the errors it prints to a collecting host. The
`systemd-journal` group lets the unit's user read the journal, and `StandardOutput=null` keeps the
lines splash writes out of the journal:

```ini
# /etc/systemd/system/splash-forward.service
[Unit]
Description=Forward journal errors to logs.example.com
Wants=network-online.target
After=network-online.target

[Service]
ExecStart=/bin/sh -c 'journalctl -f -p err -o short-iso | /usr/local/bin/splash --mode journalctl --output plain --forward tcp://logs.example.com:5170'
DynamicUser=yes
SupplementaryGroups=systemd-journal
StandardOutput=null
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

The collecting host listens, labels each line with the address of the host that sent it, and
appends the colored lines to a file:

```ini
# /etc/systemd/system/splash-collect.service
[Unit]
Description=Collect journal errors forwarded by splash
After=network.target

[Service]
ExecStart=/usr/local/bin/splash --mode journalctl --listen :5170 --label --output ansi
DynamicUser=yes
LogsDirectory=splash
StandardOutput=append:/var/log/splash/hosts.log
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Start each unit with:

```bash
systemctl daemon-reload
systemctl enable --now splash-collect.service
```

`less -R /var/log/splash/hosts.log` shows the file in color:

```
10.0.0.21 2023-10-03T12:00:04+0000 web01 systemd[1]: app.service: Main process exited, code=exited, status=1/FAILURE
```

### Receive syslog messages on port 514

Binding port 514 needs root or the `CAP_NET_BIND_SERVICE` capability. A unit can grant the
capability to a user that is not root:

```ini
# /etc/systemd/system/splash-syslog.service
[Unit]
Description=Receive syslog messages with splash
After=network.target

[Service]
ExecStart=/usr/local/bin/splash --syslog --label --output ansi
DynamicUser=yes
AmbientCapabilities=CAP_NET_BIND_SERVICE
LogsDirectory=splash
StandardOutput=append:/var/log/splash/syslog.log
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

rsyslog on each host forwards to it with `*.* @@logs.example.com:514`, as described in the
README's [Receiving syslog messages](README.md#receiving-syslog-messages) section.
