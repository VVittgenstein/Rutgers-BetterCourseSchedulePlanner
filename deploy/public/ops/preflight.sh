#!/usr/bin/env bash

# Public go-live preflight (P2 hardening H8, repository-owned half).
#
# READ-ONLY: this script inspects the host and reports; it never edits DNS,
# firewall rules, SSH configuration, units, or Caddy. The host operations it
# checks for remain separately authorized manual steps -- see the runbook's
# "Public go-live preflight" section.
#
# Usage: preflight.sh [--caddyfile PATH]
#   --caddyfile PATH   also validate the operator-managed Caddy config and
#                      require the H2 stream_close_delay directive in it.

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

BCSP_UFW="${BCSP_UFW:-ufw}"
BCSP_SSHD="${BCSP_SSHD:-sshd}"
BCSP_GETENT="${BCSP_GETENT:-getent}"
BCSP_CADDY="${BCSP_CADDY:-caddy}"
BCSP_JQ="${BCSP_JQ:-jq}"

# The two documentation addresses (RFC 5737) used as root connection tuples
# for sshd Match evaluation. Two distinct sources, so an address-conditional
# root/password policy reveals itself as a divergence and fails closed.
PREFLIGHT_SSH_PROBE_ADDRESSES=("203.0.113.1" "198.51.100.1")

PREFLIGHT_FAILURES=0
PREFLIGHT_ADVISORIES=0

pf_pass() {
  printf 'preflight: ok      %s\n' "$*"
}

pf_fail() {
  printf 'preflight: FAIL    %s\n' "$*" >&2
  PREFLIGHT_FAILURES=$((PREFLIGHT_FAILURES + 1))
}

pf_warn() {
  printf 'preflight: advise  %s\n' "$*" >&2
  PREFLIGHT_ADVISORIES=$((PREFLIGHT_ADVISORIES + 1))
}

check_origin() {
  local authority host limit_line limit_value limit_count=0

  # bcsp_origin_authority dies (in the substitution subshell) on a missing,
  # duplicated, or malformed BCSP_PUBLIC_ORIGIN. That must surface as a
  # named preflight failure -- not vanish and let the probe checks blame a
  # healthy service.
  if ! authority="$(bcsp_origin_authority 2>/dev/null)"; then
    pf_fail "BCSP_PUBLIC_ORIGIN is missing, duplicated, or malformed in $(bcsp_environment_file)"
    return 0
  fi
  host="${authority%%:*}"
  case "$host" in
    *.invalid|localhost|127.*)
      pf_fail "BCSP_PUBLIC_ORIGIN still points at the placeholder ($authority); set the real domain"
      return 0
      ;;
  esac
  pf_pass "BCSP_PUBLIC_ORIGIN names a real-looking authority: $authority"

  while IFS= read -r limit_line || [[ -n "$limit_line" ]]; do
    limit_line="${limit_line%$'\r'}"
    case "$limit_line" in
      BCSP_PUBLIC_WS_PER_CLIENT_LIMIT=*)
        limit_value="${limit_line#BCSP_PUBLIC_WS_PER_CLIENT_LIMIT=}"
        limit_count=$((limit_count + 1))
        ;;
    esac
  done < "$(bcsp_environment_file)"
  if [[ "$limit_count" -gt 1 ]]; then
    pf_fail "environment sets BCSP_PUBLIC_WS_PER_CLIENT_LIMIT more than once"
  elif [[ "$limit_count" -eq 1 ]]; then
    # 10# forces base ten: a leading zero ("09") is a legal value to the
    # service's parser and must not crash this arithmetic as octal.
    if [[ "$limit_value" =~ ^[0-9]+$ ]] && (( 10#$limit_value >= 1 && 10#$limit_value <= 1024 )); then
      pf_pass "per-client WebSocket limit override is in range: $limit_value"
    else
      pf_fail "BCSP_PUBLIC_WS_PER_CLIENT_LIMIT must be an integer in 1..1024, got '$limit_value' (the service refuses to start otherwise)"
    fi
  fi

  PREFLIGHT_HOST="$host"
}

check_dns() {
  local host="$1" resolved

  if ! command -v "$BCSP_GETENT" >/dev/null 2>&1; then
    pf_fail "getent is unavailable; cannot confirm $host resolves"
    return 0
  fi
  if resolved="$("$BCSP_GETENT" hosts "$host" 2>/dev/null)" && [[ -n "$resolved" ]]; then
    pf_pass "DNS resolves $host: $(printf '%s' "$resolved" | awk '{print $1}' | paste -sd ',' -)"
    pf_warn "confirm those addresses are THIS host before opening traffic; this script does not compare interfaces"
    # The first resolved address doubles as the local-address dimension of
    # the SSH probe matrix: real public connections arrive AT it.
    PREFLIGHT_RESOLVED_ADDR="$(printf '%s\n' "$resolved" | awk 'NR==1 {print $1}')"
  else
    pf_fail "DNS does not resolve $host; create the record (separately authorized) before go-live"
  fi
}

check_firewall() {
  local status

  if ! command -v "$BCSP_UFW" >/dev/null 2>&1; then
    pf_fail "ufw is unavailable; cannot confirm 80/443 are open (the capture snapshot showed only 22 ever open)"
    return 0
  fi
  # LC_ALL=C: ufw's output is gettext-localized; the patterns below are the
  # English forms.
  status="$(LC_ALL=C "$BCSP_UFW" status 2>/dev/null || true)"
  if ! grep -q '^Status: active' <<< "$status"; then
    pf_fail "ufw is not active"
    return 0
  fi
  local port
  for port in 80 443; do
    if grep -Eq "^${port}(/tcp)?[[:space:]]+ALLOW" <<< "$status"; then
      pf_pass "firewall allows ${port}/tcp"
    else
      pf_fail "firewall does not allow ${port}/tcp; open it with an explicit port rule (e.g. 'ufw allow ${port}/tcp' -- app-profile rules are not recognized here)"
    fi
  done
}

# One root connection tuple, fully specified so sshd can evaluate Match
# blocks for it. Prints "permitrootlogin passwordauthentication
# kbdinteractiveauthentication" or fails.
root_ssh_effective_triple() {
  local address="$1" local_address="$2" local_port="$3" effective

  effective="$("$BCSP_SSHD" -T \
    -C "user=root,host=preflight-probe.invalid,addr=${address},laddr=${local_address},lport=${local_port}" \
    2>/dev/null)" || return 1
  awk '
    $1 == "permitrootlogin" { root = $2 }
    $1 == "passwordauthentication" { password = $2 }
    $1 == "kbdinteractiveauthentication" { keyboard = $2 }
    END {
      if (root == "" || password == "" || keyboard == "") exit 1
      print root, password, keyboard
    }
  ' <<< "$effective"
}

check_root_ssh() {
  local ssh_port first_triple triple address local_address root password keyboard
  local local_addresses=()

  if ! command -v "$BCSP_SSHD" >/dev/null 2>&1; then
    pf_warn "sshd is unavailable; skip only if this host is reached some other way"
    return 0
  fi
  # H8 (R1): the judgment binds explicit root connection tuples, so
  # conditional Match blocks are evaluated instead of read past -- across
  # BOTH the source dimension (two documentation addresses) and the local
  # dimension (loopback plus the host's own resolved public address, at
  # sshd's real port). A policy that differs anywhere in that matrix is an
  # address-conditional carve-out this preflight cannot vouch for.
  ssh_port="$("$BCSP_SSHD" -T 2>/dev/null | awk '$1 == "port" { print $2; exit }')"
  if ! [[ "$ssh_port" =~ ^[0-9]+$ ]]; then
    pf_fail "cannot discover the sshd port from sshd -T; the root tuple cannot be evaluated (fail closed)"
    return 0
  fi
  local_addresses=("127.0.0.1")
  if [[ -n "$PREFLIGHT_RESOLVED_ADDR" ]]; then
    local_addresses+=("$PREFLIGHT_RESOLVED_ADDR")
  else
    pf_warn "no resolved public address available; the SSH matrix covers only a loopback local address"
  fi
  first_triple=""
  for address in "${PREFLIGHT_SSH_PROBE_ADDRESSES[@]}"; do
    for local_address in "${local_addresses[@]}"; do
      if ! triple="$(root_ssh_effective_triple "$address" "$local_address" "$ssh_port")"; then
        pf_fail "sshd -T -C for tuple addr=$address laddr=$local_address lport=$ssh_port failed; cannot evaluate the effective root policy (fail closed)"
        return 0
      fi
      if [[ -z "$first_triple" ]]; then
        first_triple="$triple"
      elif [[ "$triple" != "$first_triple" ]]; then
        pf_fail "the effective root/password policy differs across the probed connection tuples ($first_triple vs $triple at addr=$address laddr=$local_address); a conditional carve-out cannot be vouched for (fail closed)"
        return 0
      fi
    done
  done
  read -r root password keyboard <<< "$first_triple"
  case "$root" in
    no|prohibit-password|without-password|forced-commands-only)
      pf_pass "root SSH password login is disabled for the root tuple (PermitRootLogin $root)"
      ;;
    yes)
      # Root login allowed: only safe when no password-shaped method is
      # available to that tuple.
      if [[ "$password" == "no" && "$keyboard" == "no" ]]; then
        pf_pass "root SSH allows keys only for the root tuple (PermitRootLogin yes, PasswordAuthentication no, KbdInteractiveAuthentication no)"
      else
        pf_fail "root SSH password login is reachable for the root tuple (PermitRootLogin yes, PasswordAuthentication $password, KbdInteractiveAuthentication $keyboard); fix sshd_config (separately authorized)"
      fi
      ;;
    *)
      pf_fail "unrecognized PermitRootLogin value '$root' for the root tuple (fail closed)"
      ;;
  esac
}

check_failed_units() {
  local failed

  failed="$(bcsp_systemctl --failed --no-legend --plain 2>/dev/null || true)"
  if [[ -n "$failed" ]]; then
    pf_warn "failed units present (fwupd leftovers are known cosmetics): $(printf '%s' "$failed" | awk '{print $1}' | paste -sd ',' -)"
  else
    pf_pass "no failed systemd units"
  fi
}

# H2 (R1): the judgment runs on the ADAPTED configuration -- what Caddy
# will actually serve -- not on the raw text. A directive that lives only
# in a comment, another site, or another block never reaches the adapted
# public proxy and must fail. 4h == 14400000000000 nanoseconds in Caddy's
# JSON form.
PREFLIGHT_STREAM_CLOSE_DELAY_NS=14400000000000

check_caddy() {
  local caddyfile="$1" adapted

  if ! command -v "$BCSP_CADDY" >/dev/null 2>&1; then
    pf_fail "caddy is unavailable on this host"
    return 0
  fi
  pf_pass "caddy is installed"
  if [[ -z "$caddyfile" ]]; then
    pf_warn "operator-managed Caddy config not checked; re-run with --caddyfile PATH to validate it"
    return 0
  fi
  [[ -f "$caddyfile" ]] || { pf_fail "--caddyfile does not exist: $caddyfile"; return 0; }
  if ! "$BCSP_CADDY" validate --config "$caddyfile" --adapter caddyfile >/dev/null 2>&1; then
    pf_fail "caddy validate refused $caddyfile"
    return 0
  fi
  pf_pass "caddy validate accepts $caddyfile"
  if ! command -v "$BCSP_JQ" >/dev/null 2>&1; then
    pf_fail "jq is unavailable; cannot evaluate the adapted Caddy config (fail closed)"
    return 0
  fi
  if ! adapted="$("$BCSP_CADDY" adapt --config "$caddyfile" --adapter caddyfile 2>/dev/null)"; then
    pf_fail "caddy adapt refused $caddyfile; cannot evaluate the active proxy semantics (fail closed)"
    return 0
  fi
  if "$BCSP_JQ" -e --argjson delay "$PREFLIGHT_STREAM_CLOSE_DELAY_NS" '
    [.. | objects
      | select(.handler? == "reverse_proxy")
      | select(any((.upstreams // [])[]; .dial == "127.0.0.1:8080"))]
    | (length >= 1) and all(.stream_close_delay? == $delay)
  ' >/dev/null 2>&1 <<< "$adapted"; then
    pf_pass "every adapted reverse_proxy to 127.0.0.1:8080 carries stream_close_delay 4h (H2)"
  else
    pf_fail "the adapted config has no reverse_proxy to 127.0.0.1:8080 with stream_close_delay 4h on it; a reload would sever every monitoring socket (comments and other sites do not count)"
  fi
  if grep -q 'planner\.invalid' "$caddyfile"; then
    pf_fail "$caddyfile still names the planner.invalid placeholder"
  fi
}

check_service() {
  if bcsp_systemctl is-active --quiet "$BCSP_SERVICE_NAME"; then
    pf_pass "service is active"
  else
    pf_fail "service is not active; run ops/install.sh or ops/verify.sh first"
    return 0
  fi
  if bcsp_wait_for_liveness; then
    pf_pass "liveness probe returns 200"
  else
    pf_fail "liveness probe did not return 200"
  fi
}

main() {
  local caddyfile=""

  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --caddyfile)
        [[ "$#" -ge 2 ]] || bcsp_die "usage: preflight.sh [--caddyfile PATH]"
        caddyfile="$2"
        shift 2
        ;;
      *)
        bcsp_die "usage: preflight.sh [--caddyfile PATH]"
        ;;
    esac
  done
  bcsp_require_privilege

  PREFLIGHT_HOST=""
  PREFLIGHT_RESOLVED_ADDR=""
  check_origin
  if [[ -n "$PREFLIGHT_HOST" ]]; then
    check_dns "$PREFLIGHT_HOST"
  fi
  check_firewall
  check_root_ssh
  check_failed_units
  check_caddy "$caddyfile"
  check_service

  if [[ "$PREFLIGHT_FAILURES" -gt 0 ]]; then
    printf 'preflight: RESULT FAIL (%s failure(s), %s advisory(ies))\n' \
      "$PREFLIGHT_FAILURES" "$PREFLIGHT_ADVISORIES" >&2
    exit 1
  fi
  printf 'preflight: RESULT PASS (%s advisory(ies))\n' "$PREFLIGHT_ADVISORIES"
}

main "$@"
