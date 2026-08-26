#!/usr/bin/env bash

# Deterministic checks for ops/preflight.sh (H8, tightened by STAGE-3-R1 and
# STAGE-3-R2). Every host fact the preflight inspects -- DNS, firewall, sshd
# (the OPERATOR-DECLARED root connections), failed units, Caddy (the adapted
# config, bound to the public host), the service -- comes from a recording
# stub, so the script's judgment can be proven on any platform: healthy hosts
# must PASS, and each hardened fact, broken alone, must FAIL. Root-free
# (BCSP_ROOT redirect), symlink-free, network-free. No stub address is a real
# host's: the declared administrative connections use RFC 1918 space, which is
# also what the preflight demands (documentation ranges are refused outright).
# The Caddy structural cases evaluate the ADAPTED JSON and therefore need a
# real jq; where jq is absent they SKIP loudly (CI's Linux runner always has
# jq, so they run on every push).

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

# The administrative root connection every case declares unless it overrides
# it. RFC 1918 on both ends, a .test hostname: a real-SHAPED connection that
# belongs to nobody.
PF_ADMIN_SOURCE_DEFAULT='addr=10.30.4.7,host=admin.example.test,laddr=10.30.9.2,lport=22'

# sshd stubs answer BOTH invocation shapes: a plain `-T` (port discovery,
# answered with `port 22`) and the DECLARED ROOT CONNECTION
# (`-T -C user=root,host=H,addr=A,laddr=L,lport=P`). The conditional stubs
# each reopen root password login for exactly ONE dimension of the DECLARED
# connection and answer safely for everything else -- which is the shape R1
# could not see, because R1 never asked about the connection the operator
# actually uses. The `,key=` anchors matter: `addr=` is a substring of
# `laddr=`.
make_stub sshd-ok 'case "$*" in *" -C "*) printf "permitrootlogin prohibit-password\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-root-password 'case "$*" in *" -C "*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-keys-only-root 'case "$*" in *" -C "*) printf "permitrootlogin yes\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-no-tuple 'case "$*" in *" -C "*) exit 1 ;; *) printf "port 22\npermitrootlogin no\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; esac'
make_stub sshd-address-conditional 'case "$*" in *,addr=10.30.4.7,*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication yes\n" ;; *" -C "*) printf "permitrootlogin prohibit-password\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-local-address-conditional 'case "$*" in *,laddr=10.30.9.2,*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *" -C "*) printf "permitrootlogin prohibit-password\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-host-conditional 'case "$*" in *,host=admin.example.test,*) printf "permitrootlogin yes\npasswordauthentication no\nkbdinteractiveauthentication yes\n" ;; *" -C "*) printf "permitrootlogin no\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-second-listener 'case "$*" in *,lport=2222*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *" -C "*) printf "permitrootlogin no\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\nport 2222\n" ;; esac'
# UseDNS off is the OpenSSH default, and then sshd never resolves the
# client: host= is the address, so a declaration naming anything else
# describes a Match Host block that will never be evaluated.
make_stub sshd-usedns-no 'case "$*" in *" -C "*) printf "permitrootlogin no\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\nusedns no\n" ;; esac'
# The admin path is hardened and root password login is open to everyone
# else. Judging only the declared connection cannot see that.
make_stub sshd-admin-only-carveout 'case "$*" in *,addr=10.30.4.7,*) printf "permitrootlogin prohibit-password\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *" -C "*) printf "permitrootlogin yes\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *) printf "port 22\n" ;; esac'
# An admin listener that exists only as ListenAddress host:port; a truthful
# declaration of it must be accepted.
make_stub sshd-listenaddress-port 'case "$*" in *" -C "*) printf "permitrootlogin prohibit-password\npasswordauthentication yes\nkbdinteractiveauthentication no\n" ;; *) printf "listenaddress 0.0.0.0:2222\n" ;; esac'
# OpenSSH 8.6 and earlier print the pre-rename spelling and never the new one.
make_stub sshd-legacy-keyword 'case "$*" in *" -C "*) printf "permitrootlogin yes\npasswordauthentication no\nchallengeresponseauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-missing-keyword 'case "$*" in *" -C "*) printf "permitrootlogin no\npasswordauthentication no\n" ;; *) printf "port 22\n" ;; esac'
make_stub sshd-no-port 'case "$*" in *" -C "*) printf "permitrootlogin no\npasswordauthentication no\nkbdinteractiveauthentication no\n" ;; *) printf "usepam yes\n" ;; esac'

# Caddy stubs answer `adapt` with a fixed ADAPTED JSON; `validate` accepts.
# The public host is planner.example.test throughout (the env file below).
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
# R2 discriminators. Each of the next four passes the R1 whole-document scan
# (SOME proxy to 127.0.0.1:8080 exists and every literal one is protected)
# while the host the public actually reaches is unprotected or absent.
ADAPT_FOREIGN_HOST_JSON="$TEST_TMP/adapt-foreign-host.json"
cat > "$ADAPT_FOREIGN_HOST_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:9090"}]}]}]}]},{"match":[{"host":["staging.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
ADAPT_HOST_STATIC_JSON="$TEST_TMP/adapt-host-static.json"
cat > "$ADAPT_HOST_STATIC_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"static_response","status_code":503,"body":"maintenance"}]}]}]},{"match":[{"host":["internal.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
ADAPT_HOST_ALIAS_JSON="$TEST_TMP/adapt-host-alias.json"
cat > "$ADAPT_HOST_ALIAS_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"match":[{"path":["/api/v1/watch"]}],"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"localhost:8080"}]}]},{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
ADAPT_WILDCARD_MISS_JSON="$TEST_TMP/adapt-wildcard-miss.json"
cat > "$ADAPT_WILDCARD_MISS_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"static_response","status_code":200}]}]}]},{"match":[{"host":["*.staging.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
# The public host is protected, but a SIBLING vhost proxies to the same
# service under an alias spelling and is not. The severed sockets would be
# the service's either way, so the whole-document obligation has to see the
# aliases too.
ADAPT_FOREIGN_ALIAS_JSON="$TEST_TMP/adapt-foreign-alias.json"
cat > "$ADAPT_FOREIGN_ALIAS_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]},{"match":[{"host":["staging.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"localhost:8080"}]}]}]}]}]}}}}}
JSON
# A matcher set that EXCLUDES the public host is not a catch-all, and this
# check does not interpret negated host matchers: the protected proxy here
# serves every host except the public one, which gets the static response.
ADAPT_NOT_HOST_JSON="$TEST_TMP/adapt-not-host.json"
cat > "$ADAPT_NOT_HOST_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"not":[{"host":["planner.example.test"]}]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]},{"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"static_response","status_code":503}]}]}]}]}}}}}
JSON
# Caddy tries routes in order and a matching terminal route ends routing:
# the public host gets the maintenance page and never reaches the protected
# proxy in the catch-all block that follows it.
ADAPT_TERMINAL_SHADOW_JSON="$TEST_TMP/adapt-terminal-shadow.json"
cat > "$ADAPT_TERMINAL_SHADOW_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"static_response","status_code":503,"body":"maintenance"}]}]}],"terminal":true},{"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
# The same shadowing one level down: an unconditional maintenance response
# ahead of the proxy INSIDE the public host's own site block.
ADAPT_SUBROUTE_SHADOW_JSON="$TEST_TMP/adapt-subroute-shadow.json"
cat > "$ADAPT_SUBROUTE_SHADOW_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"static_response","status_code":503,"body":"maintenance"}]},{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}],"terminal":true}]}}}}}
JSON
# The protected proxy is on a listener the public origin never reaches.
ADAPT_OTHER_LISTENER_JSON="$TEST_TMP/adapt-other-listener.json"
cat > "$ADAPT_OTHER_LISTENER_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":8443"],"routes":[{"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
# handle_errors routes run on an error; they cannot prove that live traffic
# reaches the service.
ADAPT_ERRORS_ONLY_JSON="$TEST_TMP/adapt-errors-only.json"
cat > "$ADAPT_ERRORS_ONLY_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"static_response","status_code":503}]}]}],"terminal":true}],"errors":{"routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}}}}}}
JSON
# Fail-closed on a structure this check does not model: an unknown handler
# nesting the routes that carry the protected proxy.
ADAPT_OPAQUE_JSON="$TEST_TMP/adapt-opaque.json"
cat > "$ADAPT_OPAQUE_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["planner.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"some_future_handler","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}]}]}}}}}
JSON
# Shapes the host binding must NOT over-refuse: a covering wildcard, a
# multi-name site block, mixed case with a trailing dot, and the nested
# subroute with sibling handlers a real `caddy adapt` of the example emits.
ADAPT_WILDCARD_OK_JSON="$TEST_TMP/adapt-wildcard-ok.json"
cat > "$ADAPT_WILDCARD_OK_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["*.example.test"]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}}}}}
JSON
# Modelled on what a real `caddy adapt` of caddy/Caddyfile.example emits:
# a terminal host route, a `not` matcher on a PATH (from `@dynamic not path
# /assets/*`), a CONDITIONAL static response for /metrics ahead of the
# proxy, and nested subroutes. Every one of those shapes must be walked
# rather than refused or read as shadowing.
ADAPT_NESTED_OK_JSON="$TEST_TMP/adapt-nested-ok.json"
cat > "$ADAPT_NESTED_OK_JSON" <<'JSON'
{"apps":{"http":{"servers":{"srv0":{"listen":[":443"],"routes":[{"match":[{"host":["www.example.test","Planner.Example.Test."]}],"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"headers","response":{"set":{"X-Content-Type-Options":["nosniff"]}}}]},{"match":[{"not":[{"path":["/assets/*"]}]}],"handle":[{"handler":"headers","response":{"set":{"Cache-Control":["no-store"]}}}]},{"match":[{"path":["/metrics","/metrics/*"]}],"handle":[{"handler":"static_response","status_code":404}]},{"handle":[{"handler":"subroute","routes":[{"handle":[{"handler":"reverse_proxy","upstreams":[{"dial":"127.0.0.1:8080"}],"stream_close_delay":14400000000000}]}]}]}]}],"terminal":true}]}}}}}
JSON
make_stub caddy-adapt-ok "case \"\$1\" in adapt) cat '$ADAPT_OK_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-missing "case \"\$1\" in adapt) cat '$ADAPT_MISSING_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-elsewhere "case \"\$1\" in adapt) cat '$ADAPT_ELSEWHERE_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-no-proxy "case \"\$1\" in adapt) cat '$ADAPT_NO_PROXY_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-fails "case \"\$1\" in adapt) exit 1 ;; *) exit 0 ;; esac"
make_stub caddy-adapt-second-8080 "case \"\$1\" in adapt) cat '$ADAPT_SECOND_8080_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-foreign-host "case \"\$1\" in adapt) cat '$ADAPT_FOREIGN_HOST_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-host-static "case \"\$1\" in adapt) cat '$ADAPT_HOST_STATIC_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-host-alias "case \"\$1\" in adapt) cat '$ADAPT_HOST_ALIAS_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-wildcard-miss "case \"\$1\" in adapt) cat '$ADAPT_WILDCARD_MISS_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-foreign-alias "case \"\$1\" in adapt) cat '$ADAPT_FOREIGN_ALIAS_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-not-host "case \"\$1\" in adapt) cat '$ADAPT_NOT_HOST_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-terminal-shadow "case \"\$1\" in adapt) cat '$ADAPT_TERMINAL_SHADOW_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-subroute-shadow "case \"\$1\" in adapt) cat '$ADAPT_SUBROUTE_SHADOW_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-other-listener "case \"\$1\" in adapt) cat '$ADAPT_OTHER_LISTENER_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-errors-only "case \"\$1\" in adapt) cat '$ADAPT_ERRORS_ONLY_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-opaque "case \"\$1\" in adapt) cat '$ADAPT_OPAQUE_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-wildcard-ok "case \"\$1\" in adapt) cat '$ADAPT_WILDCARD_OK_JSON' ;; *) exit 0 ;; esac"
make_stub caddy-adapt-nested-ok "case \"\$1\" in adapt) cat '$ADAPT_NESTED_OK_JSON' ;; *) exit 0 ;; esac"

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

# Every run declares an administrative root connection, because the preflight
# now refuses to judge root SSH without one. PF_ADMIN_SOURCES overrides it: a
# newline-separated list of specs, or the empty string to declare none.
run_preflight() {
  local -a admin=()
  local line
  if [[ "${PF_ADMIN_SOURCES-unset}" == "unset" ]]; then
    admin=(--admin-source "$PF_ADMIN_SOURCE_DEFAULT")
  else
    while IFS= read -r line; do
      [[ -n "$line" ]] || continue
      admin+=(--admin-source "$line")
    done <<< "$PF_ADMIN_SOURCES"
  fi
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
    bash "$OPS_DIR/preflight.sh" ${admin[@]+"${admin[@]}"} "$@"
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

# The adapted-Caddy judgment runs on real JSON and so needs a real jq. These
# two wrappers keep the skip accounting honest: a case that cannot run is
# counted where it is written, so the summary can never claim coverage the
# platform did not provide.
JQ_AVAILABLE=0
if command -v jq >/dev/null 2>&1; then
  JQ_AVAILABLE=1
fi

jq_expect_pass() {
  if [[ "$JQ_AVAILABLE" -eq 1 ]]; then
    expect_pass "$@"
  else
    SKIP_COUNT=$((SKIP_COUNT + 1))
  fi
}

jq_expect_fail() {
  if [[ "$JQ_AVAILABLE" -eq 1 ]]; then
    expect_fail "$@"
  else
    SKIP_COUNT=$((SKIP_COUNT + 1))
  fi
}

# --- cases that need no jq (run everywhere) --------------------------------

expect_pass 'healthy host, no caddyfile'

# The placeholder origin is refused.
printf 'BCSP_PUBLIC_ORIGIN=https://planner.invalid\n' > "$ENV_DIR/bcsp.env"
expect_fail 'placeholder origin' 'placeholder'
# ... and with no usable origin there is no host to bind the Caddy judgment
# to, so it refuses to vouch for any proxy rather than blessing whichever one
# it finds.
expect_fail 'unbindable Caddy judgment' 'cannot bind the adapted Caddy judgment' \
  --caddyfile "$GOOD_CADDYFILE"

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

# SSH judgments are made for the DECLARED administrative connection (R2).
# Without a declaration there is no evidence, and no evidence is a failure:
PF_ADMIN_SOURCES='' expect_fail 'no declared admin connection' \
  'no administrative root connection is declared'
# ... an sshd that is not there cannot answer for it (R1 warned and PASSED):
PF_SSHD="$STUB_DIR/definitely-not-sshd" expect_fail 'sshd unavailable' \
  'sshd is unavailable; the declared'
# ... and every part of the declaration must be usable, or it is not a
# statement about a real connection:
PF_ADMIN_SOURCES='addr=10.30.4.7,host=admin.example.test' \
  expect_fail 'incomplete declaration' 'all four of addr, host, laddr, lport are required'
PF_ADMIN_SOURCES='addr=203.0.113.1,host=admin.example.test,laddr=10.30.9.2,lport=22' \
  expect_fail 'documentation source address' 'RFC 5737 documentation address'
PF_ADMIN_SOURCES='addr=10.30.4.7,host=preflight-probe.invalid,laddr=10.30.9.2,lport=22' \
  expect_fail 'reserved probe hostname' 'is a reserved name'
PF_ADMIN_SOURCES='addr=10.30.4.7,host=admin.example.test,laddr=127.0.0.1,lport=22' \
  expect_fail 'impossible loopback tuple' 'that connection never occurs'
# A loopback SOURCE is the retired documentation address wearing the new
# flag: coherent, non-documentation, and still a connection nobody
# administers through.
PF_ADMIN_SOURCES='addr=127.0.0.1,host=localhost,laddr=127.0.0.1,lport=22' \
  expect_fail 'loopback source declaration' 'declare the address administrative connections really come FROM'
PF_ADMIN_SOURCES='addr=10.30.4.7,host=admin.example.test,laddr=10.30.9.2,lport=22,rdomain=x' \
  expect_fail 'unknown declaration key' 'unknown key rdomain'
PF_ADMIN_SOURCES='addr=10.30.4.7,host=admin.example.test,laddr=10.30.9.2,lport=2222' \
  expect_fail 'declared port sshd does not listen on' 'is not a port sshd listens on'

# The three conditional carve-outs R1 could not see, because R1 asked about
# two documentation addresses instead of the connection the operator uses.
# Each stub is safe for every OTHER tuple, so only a judgment bound to the
# declaration catches it.
PF_SSHD="$STUB_DIR/sshd-address-conditional" expect_fail 'source-conditional carve-out' \
  'root SSH password login is reachable'
PF_SSHD="$STUB_DIR/sshd-local-address-conditional" expect_fail 'local-address carve-out' \
  'root SSH password login is reachable'
PF_SSHD="$STUB_DIR/sshd-host-conditional" expect_fail 'host-conditional carve-out' \
  'root SSH password login is reachable'
# A carve-out on a SECOND listener: sshd's port discovery must see every
# port, and the declared one must be among them.
PF_SSHD="$STUB_DIR/sshd-second-listener" \
  PF_ADMIN_SOURCES='addr=10.30.4.7,host=admin.example.test,laddr=10.30.9.2,lport=2222' \
  expect_fail 'second-listener carve-out' 'root SSH password login is reachable'
# Several declared paths are judged one by one, so a safe path cannot vouch
# for an unsafe one.
PF_SSHD="$STUB_DIR/sshd-address-conditional" \
  PF_ADMIN_SOURCES=$'addr=10.31.4.7,host=admin.example.test,laddr=10.30.9.2,lport=22\naddr=10.30.4.7,host=admin.example.test,laddr=10.30.9.2,lport=22' \
  expect_fail 'one unsafe path among several' 'root SSH password login is reachable'

# The declaration has to describe the connection sshd will actually see:
# with UseDNS off, host= IS the address.
PF_SSHD="$STUB_DIR/sshd-usedns-no" expect_fail 'host that sshd will never resolve' \
  'UseDNS no'
PF_SSHD="$STUB_DIR/sshd-usedns-no" \
  PF_ADMIN_SOURCES='addr=10.30.4.7,host=10.30.4.7,laddr=10.30.9.2,lport=22' \
  expect_pass 'host declared as the address under UseDNS no'
# ... and an address sshd cannot parse makes it skip every Match Address
# block while a looser check here reported a confident PASS.
PF_ADMIN_SOURCES='addr=fd00:1::zz,host=fd00:1::zz,laddr=fd00:1::9,lport=22' \
  expect_fail 'unparseable IPv6 declaration' 'not an address sshd can evaluate'
PF_ADMIN_SOURCES='addr=fd00:1::5,host=fd00:1::5,laddr=fd00:1::9,lport=22' \
  expect_pass 'IPv6 administrative path'
# An address is an address wherever it appears.
PF_ADMIN_SOURCES='addr=10.30.4.7,host=203.0.113.9,laddr=10.30.9.2,lport=22' \
  expect_fail 'documentation address in host' 'RFC 5737 documentation address'
# A listener that exists only as ListenAddress host:port is still a listener.
PF_SSHD="$STUB_DIR/sshd-listenaddress-port" \
  PF_ADMIN_SOURCES='addr=10.30.4.7,host=admin.example.test,laddr=10.30.9.2,lport=2222' \
  expect_pass 'port discovered from ListenAddress'
# The pre-8.7 spelling of the same setting is still an answer.
PF_SSHD="$STUB_DIR/sshd-legacy-keyword" expect_pass 'legacy challenge-response keyword'
# Hardening the admin path does not close root password login to everyone
# else, and R1's cross-tuple rule was the only thing that used to say so.
PF_SSHD="$STUB_DIR/sshd-admin-only-carveout" \
  expect_fail 'root open to the internet, closed to the admin' \
  'reachable from an arbitrary internet source'
# A bad declaration is reported and the next one is still judged.
PF_ADMIN_SOURCES=$'addr=10.30.4.7,host=admin.example.test\naddr=10.31.4.7,host=admin.example.test,laddr=10.30.9.2,lport=22' \
  expect_fail 'first declaration unusable, second still judged' \
  'all four of addr, host, laddr, lport are required'

# The R1 refusals that survive unchanged:
PF_SSHD="$STUB_DIR/sshd-root-password" expect_fail 'root password login' 'root SSH password login is reachable'
PF_SSHD="$STUB_DIR/sshd-no-tuple" expect_fail 'tuple evaluation unavailable' 'cannot evaluate the effective root policy'
PF_SSHD="$STUB_DIR/sshd-missing-keyword" expect_fail 'incomplete effective answer' 'cannot evaluate the effective root policy'
PF_SSHD="$STUB_DIR/sshd-no-port" expect_fail 'undiscoverable sshd port' 'cannot discover the sshd port'
PF_SSHD="$STUB_DIR/sshd-keys-only-root" expect_pass 'keys-only declared connection'

# Without jq the adapted-config judgment must fail closed, never guess.
PF_JQ="$STUB_DIR/definitely-not-jq" expect_fail 'missing jq' 'jq is unavailable' \
  --caddyfile "$GOOD_CADDYFILE"

# --- adapted-Caddy cases (need a real jq; SKIP loudly without one) ---------

jq_expect_pass 'healthy host with caddyfile' --caddyfile "$GOOD_CADDYFILE"

# Each refusal below names the obligation it broke, so no case can pass for
# a reason other than its own.
#
# The old raw-text grep passed a commented-out directive; the adapted
# config must refuse it.
PF_CADDY="$STUB_DIR/caddy-adapt-missing" jq_expect_fail 'comment-only directive' \
  'carries no stream_close_delay 4h' --caddyfile "$COMMENTED_CADDYFILE"

# A delay on some OTHER proxy does not protect the public one.
PF_CADDY="$STUB_DIR/caddy-adapt-elsewhere" jq_expect_fail 'delay on another proxy' \
  'carries no stream_close_delay 4h' --caddyfile "$GOOD_CADDYFILE"

# No public proxy at all cannot pass H2.
PF_CADDY="$STUB_DIR/caddy-adapt-no-proxy" jq_expect_fail 'no public proxy' \
  'no reverse_proxy anywhere reaches' --caddyfile "$GOOD_CADDYFILE"

# EVERY public proxy must carry the delay: a second unprotected 8080
# handler in another server fails, pinning the all()-not-any() core.
PF_CADDY="$STUB_DIR/caddy-adapt-second-8080" jq_expect_fail 'second unprotected public proxy' \
  'carries no stream_close_delay 4h' --caddyfile "$GOOD_CADDYFILE"

# R2: the protection has to belong to the route the public actually reaches.
# Every one of these PASSES the R1 whole-document scan.
PF_CADDY="$STUB_DIR/caddy-adapt-foreign-host" jq_expect_fail 'protection on another host' \
  'no active route for planner.example.test reaches' --caddyfile "$GOOD_CADDYFILE"
PF_CADDY="$STUB_DIR/caddy-adapt-host-static" jq_expect_fail 'public host serves a static response' \
  'no active route for planner.example.test reaches' --caddyfile "$GOOD_CADDYFILE"
PF_CADDY="$STUB_DIR/caddy-adapt-host-alias" jq_expect_fail 'unprotected alias proxy on the public host' \
  'carries no stream_close_delay 4h' --caddyfile "$GOOD_CADDYFILE"
PF_CADDY="$STUB_DIR/caddy-adapt-wildcard-miss" jq_expect_fail 'wildcard that does not cover the host' \
  'no active route for planner.example.test reaches' --caddyfile "$GOOD_CADDYFILE"
# ... and the whole-document obligation must see the alias spellings, or an
# unprotected sibling vhost severs the same service's sockets unnoticed.
PF_CADDY="$STUB_DIR/caddy-adapt-foreign-alias" jq_expect_fail 'unprotected alias proxy on another host' \
  'carries no stream_close_delay 4h' --caddyfile "$GOOD_CADDYFILE"
# A route that EXCLUDES the public host is not a catch-all that includes it.
PF_CADDY="$STUB_DIR/caddy-adapt-not-host" jq_expect_fail 'negated host matcher' \
  'negated host matcher' --caddyfile "$GOOD_CADDYFILE"
# Routes are tried in order and a matching terminal route ends routing, so a
# protected proxy behind a maintenance page is not protection -- at site
# level and one level down inside the site's own subroute.
PF_CADDY="$STUB_DIR/caddy-adapt-terminal-shadow" jq_expect_fail 'proxy shadowed by a terminal route' \
  'no active route for planner.example.test reaches' --caddyfile "$GOOD_CADDYFILE"
PF_CADDY="$STUB_DIR/caddy-adapt-subroute-shadow" jq_expect_fail 'proxy shadowed inside the site block' \
  'no active route for planner.example.test reaches' --caddyfile "$GOOD_CADDYFILE"
# A protected proxy on a listener the public origin never reaches proves
# nothing about the public origin.
PF_CADDY="$STUB_DIR/caddy-adapt-other-listener" jq_expect_fail 'protection on another listener' \
  'no adapted server listens on port 443' --caddyfile "$GOOD_CADDYFILE"
# handle_errors routes run on an error, not on live traffic.
PF_CADDY="$STUB_DIR/caddy-adapt-errors-only" jq_expect_fail 'proxy only in handle_errors' \
  'no active route for planner.example.test reaches' --caddyfile "$GOOD_CADDYFILE"

# A structure this check does not model is refused, not walked hopefully.
PF_CADDY="$STUB_DIR/caddy-adapt-opaque" jq_expect_fail 'uninterpretable nesting handler' \
  'nests routes this check cannot interpret' --caddyfile "$GOOD_CADDYFILE"

# ... and the host binding must not over-refuse the shapes operators really
# write: a covering wildcard, several names on one site block (mixed case,
# trailing dot), and nested subroutes with sibling handlers.
PF_CADDY="$STUB_DIR/caddy-adapt-wildcard-ok" jq_expect_pass 'covering wildcard host' \
  --caddyfile "$GOOD_CADDYFILE"
PF_CADDY="$STUB_DIR/caddy-adapt-nested-ok" jq_expect_pass 'multi-name host in nested subroutes' \
  --caddyfile "$GOOD_CADDYFILE"

# caddy adapt failing is fail-closed, not fail-open.
PF_CADDY="$STUB_DIR/caddy-adapt-fails" jq_expect_fail 'adapt failure' \
  'cannot evaluate the active proxy semantics' --caddyfile "$GOOD_CADDYFILE"

# A Caddy config still naming the placeholder is refused even when the
# adapted proxy semantics are right.
PLACEHOLDER_CADDYFILE="$TEST_TMP/Caddyfile-placeholder"
cp -- "$DEPLOY_DIR/caddy/Caddyfile.example" "$PLACEHOLDER_CADDYFILE"
jq_expect_fail 'placeholder caddyfile' 'planner.invalid placeholder' \
  --caddyfile "$PLACEHOLDER_CADDYFILE"

if [[ "$SKIP_COUNT" -gt 0 ]]; then
  printf 'preflight-check: SKIPPED %s adapted-Caddy cases (jq unavailable on this platform; CI runs them)\n' \
    "$SKIP_COUNT" >&2
fi

SKIP_NOTE=""
if [[ "$SKIP_COUNT" -gt 0 ]]; then
  SKIP_NOTE=", $SKIP_COUNT skipped without jq"
fi
printf 'preflight-check: PASS (%s healthy passes, %s hardened refusals%s)\n' \
  "$PASS_COUNT" "$FAIL_COUNT" "$SKIP_NOTE"
