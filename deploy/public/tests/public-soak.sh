#!/usr/bin/env bash

# P2 hardening H9: the 600-second public soak against the real stack.
#
# Installs the candidate as the real systemd service on a disposable Linux
# host, fronts it with a real Caddy (tls internal, planner.test), and drives
# a real Chromium page (packaging/tests/public-soak-browser.mjs) that holds
# ONE watch WebSocket for the whole soak: continuous application-PING
# sequences, every PING acknowledged, a real `caddy reload` mid-soak that
# the same socket and the same service MainPID must survive, MemoryCurrent
# sampled every 30 seconds (each sample under 700 MiB, last-three growth
# within 32 MiB of the first three), and exactly one public watch connection
# at every sample. The acknowledgements are proven from the SERVER's side
# too: its accepted-ACK counter, read before and after inside one service
# invocation, must move by what the browser reports sending -- and because
# that counter is a whole-process aggregate, the window is first shown to
# have held this browser's socket and no other (nothing connected before it
# armed, exactly one admission granted across the run).
#
# The soak core runs under the no-Rutgers drop-in and needs no data. The
# assembled-composition browser gate (public START -> server watch -> event
# -> browser observation) is a separately authorized tier: set
# BCSP_SOAK_COMPOSITION_SCRIPT (packaging/tests/real-world-browser.mjs) AND
# BCSP_SOAK_ALLOW_RUTGERS=YES to run it after the soak with the drop-in
# removed; without both, the composition stays a manual gate.
#
# Destructive only to a disposable host, exactly like disposable-host.sh.
set -Eeuo pipefail

usage() {
  printf 'usage: public-soak.sh --candidate-root PATH\n' >&2
  exit 2
}

CANDIDATE_ROOT=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --candidate-root)
      [[ "$#" -ge 2 ]] || usage
      CANDIDATE_ROOT="$2"
      shift 2
      ;;
    *) usage ;;
  esac
done

[[ "$EUID" -eq 0 ]] || { printf 'public-soak: run as root\n' >&2; exit 1; }
[[ "${BCSP_PUBLIC_SOAK_CONFIRM:-}" == "YES" ]] || {
  printf 'public-soak: set BCSP_PUBLIC_SOAK_CONFIRM=YES on a disposable host\n' >&2
  exit 1
}
[[ -n "$CANDIDATE_ROOT" ]] || usage
CANDIDATE_ROOT="$(cd -- "$CANDIDATE_ROOT" && pwd)"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/../../.." && pwd)"
SOAK_DRIVER="$REPO_ROOT/packaging/tests/public-soak-browser.mjs"
[[ -f "$SOAK_DRIVER" ]] || {
  printf 'public-soak: run from a repository checkout (missing %s)\n' "$SOAK_DRIVER" >&2
  exit 1
}

# The official gate is 600 seconds with at least 50 acknowledged pings. The
# knobs exist for harness debugging only: anything below the frozen numbers
# is labelled DEBUG and cannot stand in for H9 evidence.
SOAK_DURATION="${BCSP_SOAK_DURATION_SECONDS:-600}"
SOAK_EXPECTED_PINGS="${BCSP_SOAK_EXPECTED_PINGS:-50}"
[[ "$SOAK_DURATION" =~ ^[0-9]+$ && "$SOAK_EXPECTED_PINGS" =~ ^[0-9]+$ ]] || {
  printf 'public-soak: duration and ping knobs must be integers\n' >&2
  exit 1
}

[[ -n "${BCSP_PLAYWRIGHT_ROOT:-}" && -f "$BCSP_PLAYWRIGHT_ROOT/package.json" ]] || {
  printf 'public-soak: BCSP_PLAYWRIGHT_ROOT is not a frontend package root\n' >&2
  exit 1
}
SOAK_NODE="${BCSP_NODE_BIN:-$(command -v node || true)}"
[[ -n "$SOAK_NODE" ]] || {
  printf 'public-soak: Node.js is required\n' >&2
  exit 1
}
for command_name in awk caddy certutil curl flock getent install journalctl jq sed \
  sha256sum sqlite3 systemctl systemd-analyze update-ca-certificates useradd userdel; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'public-soak: required command is absent: %s\n' "$command_name" >&2
    exit 1
  }
done
[[ -d /run/systemd/system ]] || {
  printf 'public-soak: systemd is not running\n' >&2
  exit 1
}
# The harness runs its own Caddy on :80/:443; a distro caddy.service would
# collide mid-soak instead of failing here. (stream_close_delay also needs
# Caddy >= 2.7 -- an older binary fails loudly at the validate step.)
if systemctl is-active --quiet caddy.service 2>/dev/null; then
  printf 'public-soak: a system caddy.service is active; stop and disable it first\n' >&2
  exit 1
fi
for path in /opt/bcsp /etc/bcsp /var/lib/bcsp /var/backups/bcsp \
  /etc/systemd/system/bcsp.service /etc/systemd/system/bcsp.service.d; do
  [[ ! -e "$path" ]] || {
    printf 'public-soak: host is not clean: %s exists\n' "$path" >&2
    exit 1
  }
done
if id -u bcsp >/dev/null 2>&1; then
  printf 'public-soak: host is not clean: bcsp user already exists\n' >&2
  exit 1
fi
if getent hosts planner.test >/dev/null 2>&1; then
  printf 'public-soak: planner.test already resolves on this host\n' >&2
  exit 1
fi

# The candidate's own validator owns the shipped-file contract.
bash -c 'source "$1"; bcsp_validate_candidate_root "$2"' _ \
  "$CANDIDATE_ROOT/ops/lib.sh" "$CANDIDATE_ROOT"

if [[ -n "${BCSP_SOAK_COMPOSITION_SCRIPT:-}" ]]; then
  [[ "${BCSP_SOAK_ALLOW_RUTGERS:-}" == "YES" ]] || {
    printf 'public-soak: the composition tier polls real Rutgers; it needs BCSP_SOAK_ALLOW_RUTGERS=YES\n' >&2
    exit 1
  }
  [[ -f "$BCSP_SOAK_COMPOSITION_SCRIPT" && ! -L "$BCSP_SOAK_COMPOSITION_SCRIPT" ]] || {
    printf 'public-soak: composition script is not a regular file\n' >&2
    exit 1
  }
fi

TEST_TMP="$(mktemp -d)"
DROP_IN_ROOT=/etc/systemd/system/bcsp.service.d
CADDY_STARTED=0
SOAK_CA_PATH=""
SOAK_HOSTS_ADDED=0
CADDY_DATA_ROOT="$TEST_TMP/caddy-data"
CADDY_CONFIG_ROOT="$TEST_TMP/caddy-config"
SAMPLER_PID=""
DRIVER_PID=""

caddy_soak() {
  env XDG_DATA_HOME="$CADDY_DATA_ROOT" XDG_CONFIG_HOME="$CADDY_CONFIG_ROOT" \
    caddy "$@"
}

cleanup() {
  if [[ -n "$DRIVER_PID" ]] && kill -0 "$DRIVER_PID" 2>/dev/null; then
    kill "$DRIVER_PID" 2>/dev/null || true
  fi
  if [[ -n "$SAMPLER_PID" ]] && kill -0 "$SAMPLER_PID" 2>/dev/null; then
    kill "$SAMPLER_PID" 2>/dev/null || true
  fi
  if [[ "$CADDY_STARTED" -eq 1 ]]; then
    caddy_soak stop >/dev/null 2>&1 || true
  fi
  if [[ -n "$SOAK_CA_PATH" && -e "$SOAK_CA_PATH" ]]; then
    rm -f -- "$SOAK_CA_PATH"
    update-ca-certificates >/dev/null 2>&1 || true
  fi
  if [[ "$SOAK_HOSTS_ADDED" -eq 1 ]]; then
    sed -i '\|^127\.0\.0\.1 planner\.test # bcsp-public-soak$|d' /etc/hosts || true
  fi
  systemctl stop bcsp.service >/dev/null 2>&1 || true
  systemctl disable bcsp.service >/dev/null 2>&1 || true
  rm -rf -- /opt/bcsp /etc/bcsp /var/lib/bcsp /var/backups/bcsp
  rm -f -- /etc/systemd/system/bcsp.service
  rm -rf -- "$DROP_IN_ROOT"
  systemctl daemon-reload >/dev/null 2>&1 || true
  systemctl reset-failed bcsp.service >/dev/null 2>&1 || true
  userdel bcsp >/dev/null 2>&1 || true
  rm -rf -- "$TEST_TMP"
}
trap cleanup EXIT

# The soak core needs no upstream: loopback-only, no Rutgers.
install -d -m 0755 "$DROP_IN_ROOT"
cat > "$DROP_IN_ROOT/90-public-soak-network.conf" <<'DROPIN'
[Service]
Environment=BCSP_CI_NO_RUTGERS=1
IPAddressDeny=any
IPAddressAllow=127.0.0.0/8
IPAddressAllow=::1/128
DROPIN
chmod 0644 "$DROP_IN_ROOT/90-public-soak-network.conf"

ENV_SOURCE="$TEST_TMP/bcsp.env"
printf '%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.test' > "$ENV_SOURCE"
export BCSP_ENV_SOURCE="$ENV_SOURCE"
export BCSP_HEALTH_ATTEMPTS="${BCSP_HEALTH_ATTEMPTS:-30}"
export BCSP_HEALTH_DELAY_SECONDS="${BCSP_HEALTH_DELAY_SECONDS:-1}"
OPS_ROOT="$CANDIDATE_ROOT/ops"

bash "$OPS_ROOT/install.sh" soak-v1 "$CANDIDATE_ROOT"
systemd-analyze verify /etc/systemd/system/bcsp.service
bash "$OPS_ROOT/verify.sh"

printf '%s\n' '127.0.0.1 planner.test # bcsp-public-soak' >> /etc/hosts
SOAK_HOSTS_ADDED=1
NO_PROXY="${NO_PROXY:+$NO_PROXY,}planner.test,127.0.0.1,localhost"
no_proxy="$NO_PROXY"
export NO_PROXY no_proxy

SOAK_CADDYFILE="$TEST_TMP/Caddyfile.public-soak"
awk '
  /^planner\.invalid[[:space:]]*\{$/ {
    replacements += 1
    print "planner.test {"
    print "\ttls internal"
    next
  }
  { print }
  END { if (replacements != 1) exit 42 }
' "$CANDIDATE_ROOT/caddy/Caddyfile.example" > "$SOAK_CADDYFILE" || {
  printf 'public-soak: Caddy example did not contain one planner.invalid site\n' >&2
  exit 1
}
grep -q 'stream_close_delay 4h' "$SOAK_CADDYFILE" || {
  printf 'public-soak: the candidate Caddyfile lost stream_close_delay 4h (H2)\n' >&2
  exit 1
}
caddy_soak validate --config "$SOAK_CADDYFILE" --adapter caddyfile >/dev/null
caddy_soak start --config "$SOAK_CADDYFILE" --adapter caddyfile
CADDY_STARTED=1

CADDY_CA_SOURCE="$CADDY_DATA_ROOT/caddy/pki/authorities/local/root.crt"
for _ in {1..40}; do
  [[ -f "$CADDY_CA_SOURCE" ]] && break
  sleep 0.25
done
[[ -f "$CADDY_CA_SOURCE" ]] || {
  printf 'public-soak: Caddy test CA was not generated\n' >&2
  exit 1
}
SOAK_CA_PATH=/usr/local/share/ca-certificates/bcsp-public-soak.crt
install -m 0644 "$CADDY_CA_SOURCE" "$SOAK_CA_PATH"
update-ca-certificates >/dev/null
BROWSER_HOME="$TEST_TMP/browser-home"
BROWSER_NSSDB="$BROWSER_HOME/.pki/nssdb"
install -d -m 0700 "$BROWSER_NSSDB"
certutil -N --empty-password -d "sql:$BROWSER_NSSDB"
certutil -A -d "sql:$BROWSER_NSSDB" -n bcsp-public-soak -t 'C,,' -i "$CADDY_CA_SOURCE"
for _ in {1..40}; do
  if curl --fail --silent --show-error --connect-timeout 2 --max-time 5 \
    https://planner.test/ >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done
curl --fail --silent --show-error --connect-timeout 2 --max-time 5 \
  https://planner.test/ >/dev/null

SERVICE_PID="$(systemctl show --property MainPID --value bcsp.service)"
[[ "$SERVICE_PID" =~ ^[1-9][0-9]*$ ]] || {
  printf 'public-soak: bcsp.service has no stable MainPID\n' >&2
  exit 1
}
# The invocation id pins every later journal read to THIS service run:
# old logs from a previous run can neither convict nor acquit this soak.
SERVICE_INVOCATION="$(systemctl show --property InvocationID --value bcsp.service)"
[[ "$SERVICE_INVOCATION" =~ ^[0-9a-f]{32}$ ]] || {
  printf 'public-soak: bcsp.service has no InvocationID; cannot bind journal evidence\n' >&2
  exit 1
}

# H9: the server's own numbers, read on the loopback metrics surface (the
# public edge 404s /metrics). An unreadable or absent metric yields a marker
# the analyzer refuses: this is positive evidence or it is nothing.
#
#   heartbeat_acks_accepted   ACKs the server DECODED, matched to a sequence
#                             it really issued, and took (R2)
#   admitted_connections      capacity permits held right now, including
#                             upgrade-pending sockets (R4)
#   admissions_granted        every admission ever granted, monotonic (R3):
#                             its delta counts a replaced socket as two, which
#                             a gauge reading 1 at every sample cannot
read_public_metric() {
  local name="$1" value
  value="$(curl --silent --connect-timeout 2 --max-time 5 --header 'Host: planner.test' http://127.0.0.1:8080/metrics 2>/dev/null | awk -v metric="$name" '$1 == metric { print $2 }')" || value=""
  [[ -n "$value" ]] || value="COUNTER_READ_FAILURE"
  printf '%s\n' "$value"
}

# Taken INSIDE the invocation asserted above and again after the soak, so the
# deltas belong to this run and to no other. Read the monotonic admissions
# baseline first, the permit gauge second, and the aggregate ACK baseline
# last. That order closes both sides of the pre-browser window: a socket that
# obtains a permit after the first read either appears in the gauge or moves
# the final admissions delta, while an earlier socket's ACKs are excluded by
# the last read.
ADMISSIONS_BASELINE="$(read_public_metric bcsp_websocket_admissions_granted)"
ADMITTED_BEFORE="$(read_public_metric bcsp_websocket_admitted_connections)"
ACK_BASELINE="$(read_public_metric bcsp_websocket_heartbeat_acks_accepted)"

MEMORY_SAMPLES="$TEST_TMP/memory.samples"
CONNECTION_SAMPLES="$TEST_TMP/connections.samples"
ACK_REPORT="$TEST_TMP/ack-report.json"
ARMED_MARKER="$TEST_TMP/soak-armed"
RELOAD_MARKER="$TEST_TMP/reload-done"
DONE_MARKER="$TEST_TMP/soak-winding-down"
: > "$MEMORY_SAMPLES"
: > "$CONNECTION_SAMPLES"

env HOME="$BROWSER_HOME" "$SOAK_NODE" "$SOAK_DRIVER" \
  --base-url https://planner.test \
  --playwright-root "$BCSP_PLAYWRIGHT_ROOT" \
  --armed-marker "$ARMED_MARKER" \
  --reload-marker "$RELOAD_MARKER" \
  --done-marker "$DONE_MARKER" \
  --ack-report "$ACK_REPORT" \
  --duration-seconds "$SOAK_DURATION" \
  --expected-pings "$SOAK_EXPECTED_PINGS" &
DRIVER_PID="$!"

# A cold runner can spend minutes launching Chromium before the page and
# socket come up; four minutes covers Playwright's own launch budget.
for _ in {1..960}; do
  [[ -f "$ARMED_MARKER" ]] && break
  if ! kill -0 "$DRIVER_PID" 2>/dev/null; then
    wait "$DRIVER_PID" || true
    printf 'public-soak: the browser driver exited before arming\n' >&2
    exit 1
  fi
  sleep 0.25
done
[[ -f "$ARMED_MARKER" ]] || {
  printf 'public-soak: the browser driver never armed\n' >&2
  exit 1
}
ARMED_EPOCH="$(date +%s)"

# Sample MemoryCurrent and the capacity-permit gauge every 30 seconds while
# the soak socket is held. Every line is `epoch value`; a failed or empty read
# writes a SAMPLE_READ_FAILURE value the analyzer refuses -- a sampler that
# cannot see the service must fail the soak, not thin the evidence. The
# done marker stops sampling BEFORE the driver closes its browser, so the
# teardown itself can never contribute a zero connection sample.
(
  while [[ ! -f "$DONE_MARKER" ]]; do
    memory_value="$(systemctl show bcsp.service --property=MemoryCurrent --value 2>/dev/null)" \
      || memory_value=""
    [[ -n "$memory_value" ]] || memory_value="SAMPLE_READ_FAILURE"
    printf '%s %s\n' "$(date +%s)" "$memory_value" >> "$MEMORY_SAMPLES"
    connection_value="$(curl --silent --connect-timeout 2 --max-time 5 --header 'Host: planner.test' \
      http://127.0.0.1:8080/metrics 2>/dev/null |
      awk '$1 == "bcsp_websocket_admitted_connections" { print $2 }')" || connection_value=""
    [[ -n "$connection_value" ]] || connection_value="SAMPLE_READ_FAILURE"
    printf '%s %s\n' "$(date +%s)" "$connection_value" >> "$CONNECTION_SAMPLES"
    for _ in {1..120}; do
      [[ -f "$DONE_MARKER" ]] && break
      sleep 0.25
    done
  done
) &
SAMPLER_PID="$!"

sleep "$(( SOAK_DURATION / 2 ))"
# --force: Caddy skips applying a byte-identical config since 2.4, and a
# skipped reload proves nothing -- the gate needs the real reload path.
caddy_soak reload --force --config "$SOAK_CADDYFILE" --adapter caddyfile
[[ "$(systemctl show --property MainPID --value bcsp.service)" == "$SERVICE_PID" ]] || {
  printf 'public-soak: MainPID changed across the caddy reload\n' >&2
  exit 1
}
printf '%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$RELOAD_MARKER"

DRIVER_STATUS=0
wait "$DRIVER_PID" || DRIVER_STATUS="$?"
DRIVER_PID=""
# The judged window ends when the DRIVER's soak ended (the done marker),
# not when its browser finished closing -- a slow teardown must not
# stretch the window past what the sampler was told to cover.
if [[ -f "$DONE_MARKER" ]]; then
  SOAK_END_EPOCH="$(stat -c %Y "$DONE_MARKER")"
else
  SOAK_END_EPOCH="$(date +%s)"
fi
kill "$SAMPLER_PID" 2>/dev/null || true
wait "$SAMPLER_PID" 2>/dev/null || true
SAMPLER_PID=""
[[ "$DRIVER_STATUS" -eq 0 ]] || {
  printf 'public-soak: the browser driver failed (exit %s)\n' "$DRIVER_STATUS" >&2
  exit 1
}

[[ "$(systemctl show --property MainPID --value bcsp.service)" == "$SERVICE_PID" ]] || {
  printf 'public-soak: MainPID changed across the soak\n' >&2
  exit 1
}
[[ "$(systemctl show --property InvocationID --value bcsp.service)" == "$SERVICE_INVOCATION" ]] || {
  printf 'public-soak: the service invocation changed across the soak\n' >&2
  exit 1
}
# Same invocation, same counter: the delta is what THIS service accepted
# while THIS browser held the socket.
ACK_FINAL="$(read_public_metric bcsp_websocket_heartbeat_acks_accepted)"
ADMISSIONS_FINAL="$(read_public_metric bcsp_websocket_admissions_granted)"
# The driver counts an ACK when it SENDS one; the server logs (and only
# logs) a frame it rejects. Zero rejections in THIS invocation's journal is
# what upgrades "an ACK-shaped frame was sent" into "the server accepted
# every ACK" -- and an unreadable OR unseeing journal proves nothing, so
# both fail the soak: journalctl must exit cleanly AND the transcript must
# carry the service's own startup anchor. An empty result (invocation-id
# tagging broken in a container, rate-limit suppression, runtime-journal
# rotation) is an acquittal from silence, and silence does not acquit.
if ! SOAK_JOURNAL="$(journalctl _SYSTEMD_INVOCATION_ID="$SERVICE_INVOCATION" --no-pager 2>&1)"; then
  printf 'public-soak: journalctl failed; ACK acceptance cannot be proven (fail closed)\n' >&2
  printf '%s\n' "$SOAK_JOURNAL" >&2
  exit 1
fi
if ! grep -q 'PUBLIC_RUNTIME_STARTED' <<< "$SOAK_JOURNAL"; then
  printf 'public-soak: the invocation-bound journal does not contain the service startup anchor; the transcript cannot vouch for this run (fail closed)\n' >&2
  exit 1
fi
if grep -Eq 'rejected (malformed|invalid) watch WebSocket' <<< "$SOAK_JOURNAL"; then
  printf 'public-soak: the service rejected watch frames during the soak; the ACKs did not land\n' >&2
  exit 1
fi
"$SOAK_NODE" "$SOAK_DRIVER" --analyze-memory "$MEMORY_SAMPLES" \
  --window-start "$ARMED_EPOCH" --window-end "$SOAK_END_EPOCH" --interval-seconds 30
"$SOAK_NODE" "$SOAK_DRIVER" --analyze-connections "$CONNECTION_SAMPLES" \
  --window-start "$ARMED_EPOCH" --window-end "$SOAK_END_EPOCH" --interval-seconds 30
# The positive half of the ACK evidence: the server's accepted count moved by
# what the browser actually sent. "The page called send()" and "the journal
# holds no rejection" are both compatible with a service that silently drops
# every valid acknowledgement; this is not.
"$SOAK_NODE" "$SOAK_DRIVER" --analyze-acks --ack-report "$ACK_REPORT" \
  --ack-baseline "$ACK_BASELINE" --ack-final "$ACK_FINAL" \
  --admitted-before "$ADMITTED_BEFORE" \
  --admissions-baseline "$ADMISSIONS_BASELINE" \
  --admissions-final "$ADMISSIONS_FINAL" \
  --expected-pings "$SOAK_EXPECTED_PINGS"
bash "$OPS_ROOT/verify.sh"

COMPOSITION_RAN=0
if [[ -n "${BCSP_SOAK_COMPOSITION_SCRIPT:-}" ]]; then
  # Separately authorized tier: real upstream, real data, the assembled
  # composition gate. The drop-in comes off, so this half must not be run
  # without BCSP_SOAK_ALLOW_RUTGERS=YES (checked at startup).
  rm -f -- "$DROP_IN_ROOT/90-public-soak-network.conf"
  systemctl daemon-reload
  systemctl restart bcsp.service
  bash "$OPS_ROOT/verify.sh"
  env HOME="$BROWSER_HOME" "$SOAK_NODE" "$BCSP_SOAK_COMPOSITION_SCRIPT" \
    --base-url https://planner.test \
    --metrics-url http://127.0.0.1:8080/metrics \
    --playwright-root "$BCSP_PLAYWRIGHT_ROOT"
  COMPOSITION_RAN=1
fi

# Naming is the contract (R1): the full H9 marker exists ONLY when the
# composition tier actually ran and passed. The core soak alone -- however
# green -- reports CORE_PASS with the composition explicitly pending, and
# a sub-600s debug run is never evidence of anything.
if [[ "$SOAK_DURATION" -ge 600 && "$SOAK_EXPECTED_PINGS" -ge 50 ]]; then
  if [[ "$COMPOSITION_RAN" -eq 1 ]]; then
    printf 'P2_H9_PUBLIC_SOAK_PASS duration=%s expected_pings=%s reload=1 composition=1\n' \
      "$SOAK_DURATION" "$SOAK_EXPECTED_PINGS"
  else
    printf 'P2_H9_PUBLIC_SOAK_CORE_PASS duration=%s expected_pings=%s reload=1 composition=PENDING_EXTERNAL_AUTHORIZATION\n' \
      "$SOAK_DURATION" "$SOAK_EXPECTED_PINGS"
  fi
else
  printf 'P2_H9_PUBLIC_SOAK_DEBUG duration=%s expected_pings=%s reload=1 composition_ran=%s (below the frozen gate; NOT H9 evidence)\n' \
    "$SOAK_DURATION" "$SOAK_EXPECTED_PINGS" "$COMPOSITION_RAN"
fi
