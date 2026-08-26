#!/usr/bin/env bash

# Deterministic checks for ops/preflight.sh (H8, tightened by STAGE-3-R1).
# Every host fact the preflight inspects -- DNS, firewall, sshd (root
# connection tuples), failed units, Caddy (adapted config), the service --
# comes from a recording stub, so the script's judgment can be proven on
# any platform: healthy hosts must PASS, and each hardened fact, broken
# alone, must FAIL. Root-free (BCSP_ROOT redirect), symlink-free,
# network-free. The Caddy structural cases evaluate the ADAPTED JSON and
# therefore need a real jq; where jq is absent they SKIP loudly (CI's
# Linux runner always has jq, so they run on every push).

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

# --- healthy-host stubs; negatives swap individual ones below --------------

make_stub getent-ok 'printf "140.82.9.50      planner.example.test\n"'
make_stub getent-none 'exit 2'
make_stub ufw-ok 'printf "Status: active\n\nTo                         Action      From\n--                         ------      ----\n22/tcp                     ALLOW       Anywhere\n80/tcp                     ALLOW       Anywhere\n443                        ALLOW       Anywhere\n"'
make_stub ufw-inactive 'printf "Status: inactive\n"'
make_stub ufw-no-https 'printf "Status: active\n22/tcp                     ALLOW       Anywhere\n80/tcp                     ALLOW       Anywhere\n"'
make_stub systemctl-ok 'case "$1" in --failed) exit 0 ;; is-active) exit 0 ;; *) exit 0 ;; esac'
make_stub curl-ok 'printf "200"'

# sshd stubs answer BOTH invocation shapes: a plain `-T` (port discovery,
# answered with `port 22`) and the ROOT CONNECTION TUPLE
# (`-T -C user=root,...,addr=A,laddr=L,lport=P`). The conditional stubs
# give one tuple dimension a safe answer and the rest an unsafe one -- the
# divergence itself must fail. The local-address stub keys on the address
# the getent-ok stub resolves, exactly the laddr real public connections
# arrive at.
make_stub sshd-ok 'case "$*" in *" -C "*) printf "permitrootlogin prohibit-password\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-root-password 'case "$*" in *" -C "*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-keys-only-root 'case "$*" in *" -C "*) printf "permitrootlogin yes\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-no-tuple 'case "$*" in *" -C "*) exit 1 ;; *) printf "port 22\npermitrootlogin no\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; esac'
make_stub sshd-address-conditional 'case "$*" in *addr=203.0.113.1*) printf "permitrootlogin prohibit-password\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *" -C "*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication yes\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-local-address-conditional 'case "$*" in *laddr=140.82.9.50*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *" -C "*) printf "permitrootlogin prohibit-password\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-missing-keyword 'case "$*" in *" -C "*) printf "permitrootlogin no\npasswordauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-no-port 'case "$*" in *" -C "*) printf "permitrootlogin no\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "usepam yes\n" ;; esac'

# Caddy stubs answer `adapt` with a fixed ADAPTED JSON; `validate` accepts.
ADAPT_OK_JSON="$TEST_TMP/adapt-ok.json"
cat > "$ADAPT_OK_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
ADAPT_MISSING_JSON="$TEST_TMP/adapt-missing.json"
cat > "$ADAPT_MISSING_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}]}]}]}]}]}}}}}
JSON
ADAPT_ELSEWHERE_JSON="$TEST_TMP/adapt-elsewhere.json"
cat > "$ADAPT_ELSEWHERE_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}]}]},{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:9999"}],"stream_close_delay":14400000000000}]}]}}}}}
JSON
ADAPT_NO_PROXY_JSON="$TEST_TMP/adapt-no-proxy.json"
cat > "$ADAPT_NO_PROXY_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:9999"}],"stream_close_delay":14400000000000}]}]}}}}}
JSON
# The all() tripwire: TWO public proxies, only the first protected. A
# regression from all() to any() passes every other fixture but not this.
ADAPT_SECOND_8080_JSON="$TEST_TMP/adapt-second-8080.json"
cat > "$ADAPT_SECOND_8080_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]},"srv1":{"listen":[":8443"],"routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}]}]}]}}}}}
JSON
make_stub caddy-adapt-ok "case \"\$1\" in adapt) cat '$ADAPT_OK_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-missing "case \"\$1\" in adapt) cat '$ADAPT_MISSING_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-elsewhere "case \"\$1\" in adapt) cat '$ADAPT_ELSEWHERE_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-no-proxy "case \"\$1\" in adapt) cat '$ADAPT_NO_PROXY_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-fails "case \"\$1\" in adapt) exit 1 ;; *) exit 0 ;; esac"
make_stub caddy-adapt-second-8080 "case \"\$1\" in adapt) cat '$ADAPT_SECOND_8080_JSON' ;; *) exit 0 ;; esac"

ENV_DIR="$TEST_TMP/root/etc/bcsp"
install -d -m 0755 "$TEST_TMP/root" "$TEST_TMP/root/etc" "$ENV_DIR"
printf 'BCSP_PUBLIC_ORIGIN=https://planner.example.test\n' > "$ENV_DIR/bcsp.env"

GOOD_CADDYFILE="$TEST_TMP/Caddyfile"
sed 's/planner\.invalid/planner.example.test/' \
  "$DEPLOY_DIR/caddy/Caddyfile.example" > "$GOOD_CADDYFILE"
# The directive appears ONLY inside a comment: the raw text greps green,
# the adapted config does not carry it. This is exactly the raw-text
# evasion R1 exists to close.
COMMENTED_CADDYFILE="$TEST_TMP/Caddyfile-commented"
{
  grep -v 'stream_close_delay' "$GOOD_CADDYFILE"
  printf '# stream_close_delay 4h\n'
} > "$COMMENTED_CADDYFILE"

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

run_preflight() {
  env \
    BCSP_ROOT="$TEST_TMP/root" \
    BCSP_GETENT="${PF_GETENT:-$STUB_DIR/getent-ok}" \
    BCSP_UFW="${PF_UFW:-$STUB_DIR/ufw-ok}" \
    BCSP_SSHD="${PF_SSHD:-$STUB_DIR/sshd-ok}" \
    BCSP_SYSTEMCTL="${PF_SYSTEMCTL:-$STUB_DIR/systemctl-ok}" \
    BCSP_CURL="${PF_CURL:-$STUB_DIR/curl-ok}" \
    BCSP_CADDY="${PF_CADDY:-$STUB_DIR/caddy-adapt-ok}" \
    BCSP_JQ="${PF_JQ:-jq}" \
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
  PASS_COUNT=$((PASS_COUNT + 1))
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
  FAIL_COUNT=$((FAIL_COUNT + 1))
}

# --- cases that need no jq (run everywhere) --------------------------------

expect_pass 'healthy host, no caddyfile'

# The placeholder origin is refused.
printf 'BCSP_PUBLIC_ORIGIN=https://planner.invalid\n' > "$ENV_DIR/bcsp.env"
expect_fail 'placeholder origin' 'placeholder'

# A duplicated origin line is a NAMED preflight failure, not a stray lib
# error that lets the later probes blame a healthy service.
printf '%s\n%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.example.test' \
  'BCSP_PUBLIC_ORIGIN=https://other.example.test' > "$ENV_DIR/bcsp.env"
expect_fail 'duplicated origin' 'missing, duplicated, or malformed'
printf 'BCSP_PUBLIC_ORIGIN=https://planner.example.test\n' > "$ENV_DIR/bcsp.env"

# An out-of-range per-client override is refused before it can refuse the
# service its startup.
printf '%s\n%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.example.test' \
  'BCSP_PUBLIC_WS_PER_CLIENT_LIMIT=0' > "$ENV_DIR/bcsp.env"
expect_fail 'per-client limit 0' 'must be an integer in 1..1024'
printf '%s\n%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.example.test' \
  'BCSP_PUBLIC_WS_PER_CLIENT_LIMIT=64' > "$ENV_DIR/bcsp.env"
expect_pass 'per-client limit 64'
printf 'BCSP_PUBLIC_ORIGIN=https://planner.example.test\n' > "$ENV_DIR/bcsp.env"

# Unresolvable DNS is a hard failure.
PF_GETENT="$STUB_DIR/getent-none" expect_fail 'unresolvable domain' 'DNS does not resolve'

# The firewall must be active AND open on both public ports.
PF_UFW="$STUB_DIR/ufw-inactive" expect_fail 'inactive firewall' 'ufw is not active'
PF_UFW="$STUB_DIR/ufw-no-https" expect_fail 'closed 443' 'does not allow 443/tcp'

# SSH judgments bind the root connection tuple (R1):
#  - a password-reachable root tuple is refused;
#  - an sshd that cannot answer for the tuple (no -C support / failure) is
#    refused -- the pre-R1 global `sshd -T` would have passed this host;
#  - an address-conditional policy is refused as unvouchable;
#  - an answer missing one of the three governing keywords is refused;
#  - keys-only root (PermitRootLogin yes + no password-shaped method) passes.
PF_SSHD="$STUB_DIR/sshd-root-password" expect_fail 'root password login' 'root SSH password login is reachable'
PF_SSHD="$STUB_DIR/sshd-no-tuple" expect_fail 'tuple evaluation unavailable' 'cannot evaluate the effective root policy'
PF_SSHD="$STUB_DIR/sshd-address-conditional" expect_fail 'address-conditional policy' 'differs across the probed connection tuples'
PF_SSHD="$STUB_DIR/sshd-local-address-conditional" expect_fail 'local-address carve-out' 'differs across the probed connection tuples'
PF_SSHD="$STUB_DIR/sshd-missing-keyword" expect_fail 'incomplete effective answer' 'cannot evaluate the effective root policy'
PF_SSHD="$STUB_DIR/sshd-no-port" expect_fail 'undiscoverable sshd port' 'cannot discover the sshd port'
PF_SSHD="$STUB_DIR/sshd-keys-only-root" expect_pass 'keys-only root tuple'

# Without jq the adapted-config judgment must fail closed, never guess.
PF_JQ="$STUB_DIR/definitely-not-jq" expect_fail 'missing jq' 'jq is unavailable' \
  --caddyfile "$GOOD_CADDYFILE"

# --- adapted-Caddy cases (need a real jq; SKIP loudly without one) ---------

if command -v jq >/dev/null 2>&1; then
  expect_pass 'healthy host with caddyfile' --caddyfile "$GOOD_CADDYFILE"

  # The old raw-text grep passed a commented-out directive; the adapted
  # config must refuse it.
  PF_CADDY="$STUB_DIR/caddy-adapt-missing" expect_fail 'comment-only directive' \
    'comments and other sites do not count' --caddyfile "$COMMENTED_CADDYFILE"

  # A delay on some OTHER proxy does not protect the public one.
  PF_CADDY="$STUB_DIR/caddy-adapt-elsewhere" expect_fail 'delay on another proxy' \
    'comments and other sites do not count' --caddyfile "$GOOD_CADDYFILE"

  # No public proxy at all cannot pass H2.
  PF_CADDY="$STUB_DIR/caddy-adapt-no-proxy" expect_fail 'no public proxy' \
    'comments and other sites do not count' --caddyfile "$GOOD_CADDYFILE"

  # EVERY public proxy must carry the delay: a second unprotected 8080
  # handler in another server fails, pinning the all()-not-any() core.
  PF_CADDY="$STUB_DIR/caddy-adapt-second-8080" expect_fail 'second unprotected public proxy' \
    'comments and other sites do not count' --caddyfile "$GOOD_CADDYFILE"

  # caddy adapt failing is fail-closed, not fail-open.
  PF_CADDY="$STUB_DIR/caddy-adapt-fails" expect_fail 'adapt failure' \
    'cannot evaluate the active proxy semantics' --caddyfile "$GOOD_CADDYFILE"

  # A Caddy config still naming the placeholder is refused even when the
  # adapted proxy semantics are right.
  PLACEHOLDER_CADDYFILE="$TEST_TMP/Caddyfile-placeholder"
  cp -- "$DEPLOY_DIR/caddy/Caddyfile.example" "$PLACEHOLDER_CADDYFILE"
  expect_fail 'placeholder caddyfile' 'planner.invalid placeholder' \
    --caddyfile "$PLACEHOLDER_CADDYFILE"
else
  SKIP_COUNT=7
  printf 'preflight-check: SKIPPED %s adapted-Caddy cases (jq unavailable on this platform; CI runs them)\n' "$SKIP_COUNT" >&2
fi

SKIP_NOTE=""
if [[ "$SKIP_COUNT" -gt 0 ]]; then
  SKIP_NOTE=", $SKIP_COUNT skipped without jq"
fi
printf 'preflight-check: PASS (%s healthy passes, %s hardened refusals%s)\n' \
  "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_NOTE"
