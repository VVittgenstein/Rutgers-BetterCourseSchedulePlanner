# RBCSP public package operator runbook

This package installs one non-root `bcsp-server` behind an operator-managed
Caddy instance. It manages the application service only. It does not install or
reload Caddy, change a firewall, request certificates, modify DNS, contact
Vultr, or send production traffic.

## Package and host contract

The assembled candidate root contains `bin/bcsp-server`, release metadata, and
the `systemd`, `caddy`, `config`, `ops`, and `docs` directories. The public web
UI is embedded in `bcsp-server`; no external web-assets directory is installed
or served. The Linux host needs
systemd, `curl`, and the `sqlite3` CLI. The fixed filesystem layout is:

| Path | Purpose |
| --- | --- |
| `/opt/bcsp/releases/<version>` | Immutable binary, release metadata, and matching support assets |
| `/opt/bcsp/current` | Active-release symlink |
| `/etc/bcsp` | Service environment and operator examples |
| `/var/lib/bcsp/rbcsp.sqlite` | Service-owned operational database |
| `/var/backups/bcsp` | Online SQLite backups |

`bcsp-server` listens only on `127.0.0.1:8080`. The service creates an empty
database and its schema on first start; a release never contains course data.

## First installation

Prepare an environment file containing the real test or deployment origin:

```bash
printf '%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.invalid' > /tmp/bcsp.env
sudo BCSP_ENV_SOURCE=/tmp/bcsp.env ./ops/install.sh 0.1.0 /path/to/package
sudo ./ops/verify.sh
```

The installer creates the `bcsp` system user, fixed directories, service unit,
examples, release, and atomic `current` symlink. Re-running the same command is
safe when the release id and binary are unchanged. Use `upgrade.sh` for a new
release id.

Copy the relevant site block from `caddy/Caddyfile.example` into the Caddy
configuration managed by the operator. Replace the `.invalid` hostname. The
block rejects `/metrics`, gives hashed assets immutable caching, keeps dynamic
responses `no-store`, sets `nosniff` and a referrer policy, preserves the
application's nonce-bearing Content Security Policy, and supports HTTP and
WebSocket proxying. It intentionally sets neither CSP nor HSTS. Validate and
reload Caddy in a separately authorized host operation.

## Routine operations

Create a transactionally consistent online backup; never copy a live SQLite
file directly. Each root-only backup directory contains `rbcsp.sqlite` and a
manifest whose SHA-256 and SQLite integrity check are verified before restore:

```bash
sudo ./ops/backup.sh
```

Install and activate a new release. The script backs up the database, restarts
the service, and requires `/health/live` to return 200. It automatically restores
the previous symlink if the new binary fails that check:

```bash
sudo ./ops/upgrade.sh 0.2.0 /path/to/new-package
```

Roll back the last successful upgrade. This restores both the named previous
release and the database snapshot paired with that upgrade; it never combines
an old binary with a post-migration database:

```bash
sudo ./ops/rollback.sh 0.1.0
```

Restore a named backup directory. The service is stopped during replacement, and a
pre-restore backup is retained. A failed post-restore liveness check restores
the prior database:

```bash
sudo ./ops/restore.sh /var/backups/bcsp/backup-YYYYmmddTHHMMSSZ-manual-PID
```

Inspect service output with `journalctl -u bcsp.service`. Readiness may return
503 immediately after first start because no catalog snapshot has been loaded;
that is not a liveness failure.

## Monitoring and recovery

The unit carries three resource decisions worth knowing when reading alerts:

- `LimitNOFILE=65536`: every WebSocket, HTTP keep-alive, and SQLite handle
  holds a file descriptor. Without this line the systemd default of 1024
  turns an ordinary selection-day crowd into `EMFILE` for every new
  connection, including the health probes. `ops/verify.sh` reads the value
  back from the loaded unit.
- `MemoryHigh=700M`: soft memory pressure on a roughly 955 MiB host. Above
  it the kernel reclaims and throttles the service; nothing is killed
  (`MemoryMax` is deliberately unset). Watch
  `systemctl show bcsp -p MemoryCurrent` — sustained readings near the
  threshold mean the service is being slowed, not that it is about to die.
- `StartLimitIntervalSec=0` with `Restart=on-failure` and `RestartSec=5s`:
  a crashing service retries forever at five-second intervals instead of
  parking itself in `failed` after five crashes. The service therefore
  recovers from crash loops (an OOM storm included) without a human running
  `reset-failed`, at the cost that a persistent crash shows up as a restart
  storm rather than a dead unit.

Because a broken service now keeps restarting rather than staying down,
monitor for the loop itself, not only for "inactive":

```bash
journalctl -u bcsp.service --since -15min | grep -c 'Scheduled restart'
```

A steadily growing count is the crash-loop signal; read the panic or exit
message immediately above each restart line. `systemctl stop` remains an
ordinary stop — a deliberate stop is not a failure and is not restarted.

Watch capacity and backpressure on the loopback metrics surface (the public
edge blocks `/metrics`): `bcsp_websocket_admitted_connections`,
`bcsp_websocket_global_capacity_refusals` and
`bcsp_websocket_client_capacity_refusals` (each refusal is a 503 to a
browser; sustained client-capacity refusals from a campus NAT are the
signal to raise `BCSP_PUBLIC_WS_PER_CLIENT_LIMIT`), the queued-byte gauge
`bcsp_websocket_outbound_queued_bytes`, and the three disconnect counters
(`slow_consumer`, `global_outbound_budget`, `write_timeout`) that say why
the service cut a connection loose.

## Disposable-host test hooks

All scripts accept `BCSP_ROOT=/temporary/root` to redirect managed filesystem
paths. `BCSP_SYSTEMCTL`, `BCSP_CURL`, `BCSP_SQLITE3`, `BCSP_SHA256SUM`, and
`BCSP_FLOCK` inject command paths. Tests may set `BCSP_SKIP_USER_MANAGEMENT=1`
and `BCSP_SKIP_OWNERSHIP=1`; those flags must not be used on a real host.

CI passes the assembled candidate to `tests/disposable-host.sh --candidate-root
PATH` on a clean systemd Ubuntu host. That destructive test requires
`BCSP_DISPOSABLE_HOST_CONFIRM=YES`, installs the real service at the real FHS
paths, and adds a temporary systemd IP deny/loopback allow drop-in before first
start. The service therefore cannot contact Rutgers or any external host during
this packaging test. It also loads the embedded HTML document and its hashed
JavaScript asset from the real service, without installing an external web
directory. The trap removes the service, user, paths, and drop-in.
