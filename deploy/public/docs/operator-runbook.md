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
systemd, `curl`, and the `sqlite3` CLI; `ops/preflight.sh` additionally needs
`jq` to judge the adapted Caddy configuration. The fixed filesystem layout is:

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

**Caddy reload risk.** Without `stream_close_delay`, every `caddy reload`
immediately severs every proxied WebSocket — every person's live section
monitoring, all at once. The example's `reverse_proxy` block therefore sets
`stream_close_delay 4h`: established streams keep flowing under the old
configuration for up to four hours after a reload, while new connections use
the new configuration at once. Do not remove that directive when adapting
the block. A reload is still not free — streams older than the delay, and
any stream still open when the delay expires, are closed, and a second
reload restarts nothing — so schedule reloads in low-traffic windows
(late night US Eastern during registration periods) and prefer batching
config changes over reloading repeatedly.

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

## Public go-live preflight

`ops/preflight.sh` is the read-only go-live gate: run it after
`ops/verify.sh` passes and before pointing real traffic at the host. It
inspects and reports — it never edits DNS, firewall rules, SSH
configuration, or Caddy. Every fix it demands is a separately authorized
manual host operation:

```bash
sudo ./ops/preflight.sh \
  --admin-source addr=<your-admin-source-ip>,host=<what-sshd-sees>,laddr=<this-host-ip>,lport=22 \
  --caddyfile /etc/caddy/Caddyfile
```

**Declare the real administrative connection.** `--admin-source` is
required and is repeatable — once per path you actually administer this
host through (each admin network, each jump host, IPv4 and IPv6
separately). Its value is sshd's own `-C` connection spec minus `user=`,
which the script pins to `root`: `addr=` the source address you connect
FROM, `host=` the name sshd resolves that source to, `laddr=`/`lport=` the
local address and port the connection arrives AT. The preflight hands each
declaration to `sshd -T -C` so the `Match Address`, `Match Host`, and
`Match LocalPort` blocks that really apply to your administrative login
are evaluated — including one that reopens root password login for exactly
the network you use.

Fill the placeholders in from the host itself rather than guessing:
`who am i` or `last -i` shows the source you arrive from, `ss -tnp state
established '( sport = :ssh )'` shows the local address and port your
session landed on, and `sshd -T | grep -E '^(port|listenaddress|usedns) '`
shows the ports sshd listens on and whether it resolves clients at all.
**When `usedns` is `no` — the OpenSSH default — sshd never resolves the
client and `host=` must be the address**, byte for byte; the preflight
refuses a name there rather than evaluate a `Match Host` block that will
never fire. Ports are discovered from both `Port` and `ListenAddress`, so
an admin listener declared only as `host:port` is accepted.

Substitutes are refused on purpose: documentation-range addresses (RFC
5737/3849) anywhere in the spec, `.invalid` hostnames, an IPv6 literal
sshd could not parse (it would silently skip every `Match Address` block),
a loopback `addr=` (a connection that never crosses the network says
nothing about administrative access), a loopback `laddr=` under a remote
`addr=` (that connection never occurs), and a declared port sshd does not
listen on. With no declaration at all the run fails rather than reporting
on a connection nobody makes.

One probe is not a declaration and not evidence: after judging the
declared paths, the preflight asks sshd about a documentation address as a
**control**. Root password login reachable from an arbitrary internet
source is a blocker no declaration can excuse — hardening your own path
does not close that door — so the control can fail the run and can never
pass it.

Hard failures (non-zero exit): `BCSP_PUBLIC_ORIGIN` missing, duplicated,
malformed, not an ordinary DNS name, or still naming the
`planner.invalid` placeholder; an out-of-range
`BCSP_PUBLIC_WS_PER_CLIENT_LIMIT` (the service would refuse to start); a
domain that does not resolve; `ufw` inactive or 80/443 not allowed by
explicit port rules (the capture snapshot showed only 22 ever open); no
declared administrative root connection, a declaration that cannot be
parsed or does not name a port sshd listens on, a missing `sshd`, a
declared connection whose effective root policy is password-reachable or
cannot be evaluated, or root password login reachable from the control
source; a missing Caddy binary or `jq`; a supplied Caddy config that fails
`caddy validate` or `caddy adapt`, whose ADAPTED route for the
`BCSP_PUBLIC_ORIGIN` host does not itself reach `127.0.0.1:8080` with
`stream_close_delay 4h` on every `reverse_proxy` to the service, or that
still names the placeholder; an inactive or lifeless service. Advisories
(reported, not fatal): failed units such as the known `fwupd` leftovers,
an unchecked operator Caddy config, a control probe that could not be
evaluated, and the reminder to confirm the resolved addresses really are
this host.

What "reaches" means for the Caddy check, since a config can look right
and serve something else. The judgment runs on `caddy adapt` output, for
the host and port of `BCSP_PUBLIC_ORIGIN`, in Caddy's own order at both
levels: a protected proxy behind a matching terminal route, behind an
unconditional `respond`, or behind a handler that answers the request
EARLIER IN THE SAME ROUTE's handler chain, is not protection and does not
count — Caddy stops at that handler and never calls the proxy. An ordinary
middleware (`header`, `encode`, `rewrite`) does call the next handler, so a
proxy behind one of those still counts. Neither counts a proxy on another
host, inside `handle_errors` (those run on an error, not on live traffic),
or on a different listen address: two servers on the origin port are two
interfaces, and every server that could serve this host must satisfy the
contract by itself rather than borrow a sibling's protection. The `localhost:8080` /
`[::1]:8080` / `:8080` spellings are the same upstream as `127.0.0.1:8080`,
so an unprotected one anywhere in the config fails the gate. A structure
the check cannot interpret — a handler whose next/terminal behaviour it
does not model (a plugin, say), an unknown handler nesting routes, a
subroute with handlers after it, a negated or CEL host matcher, a host
matcher holding a placeholder, a listen address with no readable port — is
refused rather than guessed at, so an exotic config may need simplifying
before it can be vouched for. The
refusal names which of those it hit.

The preflight does not replace the re-launch hard gates: the 600-second
public soak (`tests/public-soak.sh`) and the assembled-composition browser
test still have to pass on a Linux runner before the service is opened up.

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
`bcsp_websocket_outbound_queued_bytes`, the three disconnect counters
(`slow_consumer`, `global_outbound_budget`, `write_timeout`) that say why
the service cut a connection loose, and
`bcsp_websocket_heartbeat_acks_accepted` — heartbeat acknowledgements the
service ACCEPTED (decoded, matched to a sequence it issued to that
connection, not a replay). It is monotonic for the life of the process, so
read it twice and judge the difference; a flat counter while pages are
connected means the heartbeat round trip is not closing. Beside it,
`bcsp_websocket_admissions_granted` counts every WebSocket admission this
process has ever granted -- unlike `bcsp_websocket_admitted_connections`,
which is how many are open right now, its delta counts a socket that
dropped and was replaced as two.

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

`tests/public-soak.sh --candidate-root PATH` is the H9 re-launch hard gate,
run the same destructive way from a repository checkout (it needs
`packaging/tests/public-soak-browser.mjs`). It requires
`BCSP_PUBLIC_SOAK_CONFIRM=YES` and `BCSP_PLAYWRIGHT_ROOT`, installs the
candidate behind a real Caddy, and holds one browser WebSocket for 600
seconds: continuous acknowledged application pings, a mid-soak
`caddy reload --force` (forced, because Caddy skips applying a
byte-identical config and a skipped reload proves nothing) that the same
socket and the same service `MainPID` must survive, every 30-second
`MemoryCurrent` sample under 700 MiB with last-three growth within
32 MiB, and exactly one public watch connection at every sample.
Samples are timestamped and judged against the WHOLE window (thin, holed,
late, or truncated coverage fails), and the ACK-acceptance journal read is
bound to this service invocation and fails closed if `journalctl` fails.
Acceptance is proven positively as well: the service's own
`bcsp_websocket_heartbeat_acks_accepted` counter is read before and after,
inside that one invocation, and its delta must match the acknowledgements
the browser reports sending. A service that silently ignored every valid
ACK — while still refreshing its heartbeat from arbitrary inbound text —
would satisfy "the page called send()" and "the journal shows no
rejection", and fails here.

That counter is a whole-process aggregate, so the soak also proves the
window belonged to one socket: no public watch connection open before the
browser arms, exactly one admission granted across the run
(`bcsp_websocket_admissions_granted`, monotonic, so a socket that dropped
and was replaced counts as two), and exactly one connection at every
30-second sample. Without that, a second socket acknowledging normally
could supply the very acknowledgements the soak's own connection never had
accepted.

The host needs Caddy 2.7 or newer (`stream_close_delay`) with no distro
`caddy.service` active. `BCSP_SOAK_DURATION_SECONDS` below 600 exists for
harness debugging only and prints a DEBUG line that is not H9 evidence.

The naming is the contract: the soak alone prints
`P2_H9_PUBLIC_SOAK_CORE_PASS ... composition=PENDING_EXTERNAL_AUTHORIZATION`
— the H9 gate is NOT closed by it. Only a run with
`BCSP_SOAK_COMPOSITION_SCRIPT` (and `BCSP_SOAK_ALLOW_RUTGERS=YES`), which
additionally drives the assembled-composition browser gate against real
upstream data after the soak, may print the full `P2_H9_PUBLIC_SOAK_PASS`.
