#!/usr/bin/env bash

# Deterministic checks for ops/preflight.sh (H8). Every host fact the
# preflight inspects -- DNS, firewall, sshd, failed units, Caddy, the
# service -- comes from a recording stub, so the script's judgment can be
# proven on any platform: one fully-healthy host must PASS, and each
# hardened fact, broken alone, must FAIL. Root-free (BCSP_ROOT redirect),
# symlink-free, network-free.

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
OPS_DIR="$(cd -- "$SCRIPT_DIR/../ops" && pwd)"
DEPLOY_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"

TEST_TMP="$(mktemp -d)"
cleanup() {
  rm -rf -- "$TEST_TMP"
}
trap cleanup EXIT

STUB_DIR="$TEST_TMP/stubs"
install -d -m 0755 "$STUB_DIR"

make_stub() {
  local name="$1" body="$2"
  printf '#!/usr/bin/env bash\n%s\n' "$body" > "$STUB_DIR/$name"
  chmod 0755 "$STUB_DIR/$name"
}

# Healthy-host stubs. Negatives swap individual ones below.
make_stub getent-ok 'printf "140.82.9.50      planner.example.test\n"'
make_stub getent-none 'exit 2'
make_stub ufw-ok 'printf "Status: active\n\nTo                         Action      From\n--                         ------      ----\n22/tcp                     ALLOW       Anywhere\n80/tcp                     ALLOW       Anywhere\n443                        ALLOW       Anywhere\n"'
make_stub ufw-inactive 'printf "Status: inactive\n"'
make_stub ufw-no-https 'printf "Status: active\n22/tcp                     ALLOW       Anywhere\n80/tcp                     ALLOW       Anywhere\n"'
make_stub sshd-ok 'printf "permitrootlogin prohibit-password\npasswordauthentication yes\n"'
make_stub sshd-root-password 'printf "permitrootlogin yes\n"'
make_stub systemctl-ok 'case "$1" in --failed) exit 0 ;; is-active) exit 0 ;; *) exit 0 ;; esac'
make_stub curl-ok 'printf "200"'
make_stub caddy-ok 'exit 0'

ENV_DIR="$TEST_TMP/root/etc/bcsp"
install -d -m 0755 "$TEST_TMP/root" "$TEST_TMP/root/etc" "$ENV_DIR"
printf 'BCSP_PUBLIC_ORIGIN=https://planner.example.test\n' > "$ENV_DIR/bcsp.env"

GOOD_CADDYFILE="$TEST_TMP/Caddyfile"
sed 's/planner\.invalid/planner.example.test/' \
  "$DEPLOY_DIR/caddy/Caddyfile.example" > "$GOOD_CADDYFILE"

run_preflight() {
  # Callers override individual BCSP_* stubs in the environment.
  env \
    BCSP_ROOT="$TEST_TMP/root" \
    BCSP_GETENT="${PF_GETENT:-$STUB_DIR/getent-ok}" \
    BCSP_UFW="${PF_UFW:-$STUB_DIR/ufw-ok}" \
    BCSP_SSHD="${PF_SSHD:-$STUB_DIR/sshd-ok}" \
    BCSP_SYSTEMCTL="${PF_SYSTEMCTL:-$STUB_DIR/systemctl-ok}" \
    BCSP_CURL="${PF_CURL:-$STUB_DIR/curl-ok}" \
    BCSP_CADDY="${PF_CADDY:-$STUB_DIR/caddy-ok}" \
    BCSP_HEALTH_ATTEMPTS=2 \
    BCSP_HEALTH_DELAY_SECONDS=0 \
    bash "$OPS_DIR/preflight.sh" "$@"
}

expect_pass() {
  local label="$1"; shift
  local output
  if ! output="$(run_preflight "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    printf 'preflight-check: %s: expected PASS\n' "$label" >&2
    exit 1
  fi
  if ! grep -q 'RESULT PASS' <<< "$output"; then
    printf '%s\n' "$output" >&2
    printf 'preflight-check: %s: missing RESULT PASS\n' "$label" >&2
    exit 1
  fi
}

expect_fail() {
  local label="$1" needle="$2"; shift 2
  local output
  if output="$(run_preflight "$@" 2>&1)"; then
    printf '%s\n' "$output" >&2
    printf 'preflight-check: %s: expected FAIL\n' "$label" >&2
    exit 1
  fi
  if ! grep -q "$needle" <<< "$output"; then
    printf '%s\n' "$output" >&2
    printf 'preflight-check: %s: missing message: %s\n' "$label" "$needle" >&2
    exit 1
  fi
}

# A healthy host passes, with and without the Caddy config supplied.
expect_pass 'healthy host' --caddyfile "$GOOD_CADDYFILE"
expect_pass 'healthy host, no caddyfile'

# The placeholder origin is refused.
printf 'BCSP_PUBLIC_ORIGIN=https://planner.invalid\n' > "$ENV_DIR/bcsp.env"
expect_fail 'placeholder origin' 'placeholder'
printf 'BCSP_PUBLIC_ORIGIN=https://planner.example.test\n' > "$ENV_DIR/bcsp.env"

# An out-of-range per-client override is refused before it can refuse the
# service its startup.
printf '%s\n%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.example.test' \
  'BCSP_PUBLIC_WS_PER_CLIENT_LIMIT=0' > "$ENV_DIR/bcsp.env"
expect_fail 'per-client limit 0' 'must be an integer in 1..1024'
printf '%s\n%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.example.test' \
  'BCSP_PUBLIC_WS_PER_CLIENT_LIMIT=64' > "$ENV_DIR/bcsp.env"
expect_pass 'per-client limit 64' --caddyfile "$GOOD_CADDYFILE"
printf 'BCSP_PUBLIC_ORIGIN=https://planner.example.test\n' > "$ENV_DIR/bcsp.env"

# Unresolvable DNS is a hard failure.
PF_GETENT="$STUB_DIR/getent-none" expect_fail 'unresolvable domain' 'DNS does not resolve'

# The firewall must be active AND open on both public ports.
PF_UFW="$STUB_DIR/ufw-inactive" expect_fail 'inactive firewall' 'ufw is not active'
PF_UFW="$STUB_DIR/ufw-no-https" expect_fail 'closed 443' 'does not allow 443/tcp'

# Root SSH password login must be off.
PF_SSHD="$STUB_DIR/sshd-root-password" expect_fail 'root password login' 'root SSH password login is not disabled'

# The operator Caddy config must carry the H2 drain directive.
NO_DELAY_CADDYFILE="$TEST_TMP/Caddyfile-no-delay"
grep -v 'stream_close_delay' "$GOOD_CADDYFILE" > "$NO_DELAY_CADDYFILE"
expect_fail 'missing stream_close_delay' 'stream_close_delay' --caddyfile "$NO_DELAY_CADDYFILE"

# A Caddy config still naming the placeholder is refused.
expect_fail 'placeholder caddyfile' 'planner.invalid placeholder' \
  --caddyfile "$DEPLOY_DIR/caddy/Caddyfile.example"

printf 'preflight-check: PASS (one healthy pass, seven hardened refusals)\n'
