#!/usr/bin/env bash

# Public go-live preflight (P2 hardening H8, repository-owned half).
#
# READ-ONLY: this script inspects the host and reports; it never edits DNS,
# firewall rules, SSH configuration, units, or Caddy. The host operations it
# checks for remain separately authorized manual steps -- see the runbook's
# "Public go-live preflight" section.
#
# Usage: preflight.sh --admin-source SPEC [--admin-source SPEC ...]
#                     [--caddyfile PATH]
#   --admin-source SPEC  the sshd -C connection spec of a REAL administrative
#                        root connection: addr=A,host=H,laddr=L,lport=P.
#                        Repeat it once per administrative path (each admin
#                        network, each jump host, IPv4 and IPv6 separately).
#                        Required: the root SSH policy is judged for these
#                        connections and for nothing else.
#   --caddyfile PATH     also validate the operator-managed Caddy config and
#                        require the H2 stream_close_delay directive on the
#                        public host's own adapted route.

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

BCSP_UFW="${BCSP_UFW:-ufw}"
BCSP_SSHD="${BCSP_SSHD:-sshd}"
BCSP_GETENT="${BCSP_GETENT:-getent}"
BCSP_CADDY="${BCSP_CADDY:-caddy}"
BCSP_JQ="${BCSP_JQ:-jq}"

# H8 (R2): the root SSH connections the OPERATOR declares they actually
# administer this host through, as sshd -C connection specs. R1 probed two
# fixed RFC 5737 documentation addresses, which evaluates a connection nobody
# ever makes: a `Match Address <the real admin network>` block that reopens
# root password login was invisible to it, and so was every `Match Host` and
# every second listener. There is no default here and no synthetic
# substitute -- an undeclared, unparseable, or unevaluable connection is a
# failure, never an advisory.
PREFLIGHT_ADMIN_SOURCES=()

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
  local authority host port limit_line limit_value limit_count=0

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
  # `${authority%%:*}` is only a host for an ordinary name:port authority. A
  # bracketed IPv6 literal leaves "[" behind, and calling that a real-looking
  # authority would send every later check off to judge a host that does not
  # exist.
  if ! [[ "$host" =~ ^[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$ ]]; then
    pf_fail "BCSP_PUBLIC_ORIGIN's authority is not an ordinary DNS name ($authority); this preflight can only judge a named host"
    return 0
  fi
  if [[ "$authority" == *:* ]]; then
    port="${authority##*:}"
    if ! [[ "$port" =~ ^[0-9]+$ ]]; then
      pf_fail "BCSP_PUBLIC_ORIGIN names a port that is not a number ($authority)"
      return 0
    fi
  else
    port=443
  fi
  pf_pass "BCSP_PUBLIC_ORIGIN names a real-looking authority: $authority (host $host, port $port)"

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
  PREFLIGHT_PORT="$port"
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

preflight_is_loopback() {
  [[ "$1" == 127.* || "$1" == "::1" ]]
}

# An IPv6 literal in the plain form sshd accepts: hex groups of 1..4 digits,
# at most one "::", eight groups without it and at most seven with it.
# IPv4-mapped forms are deliberately out: declare the IPv4 address instead.
preflight_valid_ipv6() {
  local address="$1" marks group count=0
  local -a groups=()

  [[ "$address" =~ ^[0-9A-Fa-f:]+$ ]] || return 1
  [[ "$address" == *:::* ]] && return 1
  marks="${address//::/|}"
  marks="${marks//[^|]/}"
  [[ "${#marks}" -le 1 ]] || return 1
  IFS=':' read -r -a groups <<< "$address"
  for group in ${groups[@]+"${groups[@]}"}; do
    [[ -n "$group" ]] || continue
    [[ "$group" =~ ^[0-9A-Fa-f]{1,4}$ ]] || return 1
    count=$((count + 1))
  done
  if [[ "${#marks}" -eq 0 ]]; then
    [[ "$count" -eq 8 ]] || return 1
  else
    [[ "$count" -le 7 ]] || return 1
  fi
  return 0
}

# A syntactically real, connectable address literal. The documentation ranges
# are refused BY CONSTRUCTION: they were R1's probe addresses, they name no
# host anyone administers, and they must not be smuggled back in through the
# new flag as go-live evidence.
preflight_valid_admin_address() {
  local address="$1" role="$2" octet
  local -a octets=()

  if [[ "$address" == *:* ]]; then
    # sshd SKIPS every Match Address block when it cannot parse the address
    # it was given, so a loose check here would produce a confident PASS
    # about a policy that was never evaluated. Groups, group count, and the
    # single "::" are all checked.
    if ! preflight_valid_ipv6 "$address"; then
      printf '%s is not an address sshd can evaluate: %s\n' "$role" "$address"
      return 1
    fi
    case "$address" in
      2001:[Dd][Bb]8:*|2001:0[Dd][Bb]8:*)
        printf '%s is an RFC 3849 documentation address and is not go-live evidence: %s\n' \
          "$role" "$address"
        return 1
        ;;
    esac
    return 0
  fi
  [[ "$address" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    printf '%s is not an IPv4/IPv6 literal: %s\n' "$role" "$address"
    return 1
  }
  IFS='.' read -r -a octets <<< "$address"
  for octet in "${octets[@]}"; do
    # 10# forces base ten: a padded octet ("010") is not octal here.
    (( 10#$octet <= 255 )) || {
      printf '%s has an out-of-range octet: %s\n' "$role" "$address"
      return 1
    }
  done
  case "$address" in
    192.0.2.*|198.51.100.*|203.0.113.*)
      printf '%s is an RFC 5737 documentation address and is not go-live evidence: %s\n' \
        "$role" "$address"
      return 1
      ;;
    0.0.0.0|255.255.255.255)
      printf '%s is not a connectable address: %s\n' "$role" "$address"
      return 1
      ;;
  esac
  return 0
}

# Validates one --admin-source spec and prints it back in canonical order, or
# prints why it cannot be used and returns non-zero. The grammar IS sshd's own
# -C grammar minus user=, which the caller pins to root so it cannot be
# spoofed away.
preflight_parse_admin_source() {
  local spec="$1" field key value
  local addr="" host="" laddr="" lport=""
  local -a fields=()

  IFS=',' read -r -a fields <<< "$spec"
  for field in "${fields[@]}"; do
    [[ "$field" == *"="* ]] || {
      printf 'field is not key=value: %s\n' "$field"
      return 1
    }
    key="${field%%=*}"
    value="${field#*=}"
    [[ -n "$value" ]] || {
      printf '%s is empty\n' "$key"
      return 1
    }
    case "$key" in
      addr)
        [[ -z "$addr" ]] || { printf 'addr is given more than once\n'; return 1; }
        addr="$value"
        ;;
      host)
        [[ -z "$host" ]] || { printf 'host is given more than once\n'; return 1; }
        host="$value"
        ;;
      laddr)
        [[ -z "$laddr" ]] || { printf 'laddr is given more than once\n'; return 1; }
        laddr="$value"
        ;;
      lport)
        [[ -z "$lport" ]] || { printf 'lport is given more than once\n'; return 1; }
        lport="$value"
        ;;
      *)
        printf 'unknown key %s (expected addr, host, laddr, lport)\n' "$key"
        return 1
        ;;
    esac
  done
  [[ -n "$addr" && -n "$host" && -n "$laddr" && -n "$lport" ]] || {
    printf 'all four of addr, host, laddr, lport are required\n'
    return 1
  }
  preflight_valid_admin_address "$addr" "addr" || return 1
  preflight_valid_admin_address "$laddr" "laddr" || return 1
  # A loopback SOURCE is the old false pass wearing the new flag: it is a
  # connection that never crosses the network, and vouching for it says
  # exactly as much about administrative access as a documentation address
  # did. Declare the address you actually connect FROM.
  if preflight_is_loopback "$addr"; then
    printf 'addr=%s is loopback; declare the address administrative connections really come FROM\n' \
      "$addr"
    return 1
  fi
  # And a packet from a non-loopback source never arrives at 127.0.0.1.
  if preflight_is_loopback "$laddr"; then
    printf 'laddr=%s is loopback but addr=%s is not; that connection never occurs\n' \
      "$laddr" "$addr"
    return 1
  fi
  [[ "$host" =~ ^[A-Za-z0-9._:-]+$ ]] || {
    printf 'host is not a hostname or address literal: %s\n' "$host"
    return 1
  }
  if [[ "$host" =~ ^[0-9]+.[0-9]+.[0-9]+.[0-9]+$ || "$host" == *:* ]]; then
    preflight_valid_admin_address "$host" "host" || return 1
  fi
  case "${host,,}" in
    *.invalid)
      printf 'host %s is a reserved name and is not go-live evidence\n' "$host"
      return 1
      ;;
  esac
  if ! [[ "$lport" =~ ^[0-9]+$ ]] || ! (( 10#$lport >= 1 && 10#$lport <= 65535 )); then
    printf 'lport must be an integer in 1..65535: %s\n' "$lport"
    return 1
  fi
  printf 'host=%s,addr=%s,laddr=%s,lport=%s\n' "$host" "$addr" "$laddr" "$((10#$lport))"
}

# One declared root connection, fully specified so sshd evaluates the Match
# blocks that really apply to it. Prints "permitrootlogin
# passwordauthentication kbdinteractiveauthentication" or fails.
root_ssh_effective_triple() {
  local connection="$1" effective

  effective="$("$BCSP_SSHD" -T -C "user=root,${connection}" 2>/dev/null)" || return 1
  awk '
    $1 == "permitrootlogin" { root = $2 }
    $1 == "passwordauthentication" { password = $2 }
    # OpenSSH renamed this in 8.7; older sshd prints the former spelling
    # and never the latter, so demanding only the new name would make the
    # gate unpassable on those hosts rather than safer.
    $1 == "kbdinteractiveauthentication" { keyboard = $2 }
    $1 == "challengeresponseauthentication" { if (keyboard == "") keyboard = $2 }
    END {
      if (root == "" || password == "" || keyboard == "") exit 1
      print root, password, keyboard
    }
  ' <<< "$effective"
}

check_root_ssh() {
  local spec canonical triple root password keyboard ports lport
  local global_config usedns declared_host declared_addr control_reference=""
  local control_triple control_root control_password control_keyboard
  local control_laddr control_lport

  # H8 (R2): the judgment is made for the connection(s) the operator DECLARES
  # they administer this host through, and for nothing else. Nothing is
  # inferred from DNS, nothing is synthesised, and no declaration means no
  # evidence -- which is a failure, never an advisory.
  if [[ "${#PREFLIGHT_ADMIN_SOURCES[@]}" -eq 0 ]]; then
    pf_fail "no administrative root connection is declared; pass --admin-source addr=A,host=H,laddr=L,lport=P (repeat it once per administrative path) so sshd evaluates the REAL root connection (fail closed)"
    return 0
  fi
  if ! command -v "$BCSP_SSHD" >/dev/null 2>&1; then
    pf_fail "sshd is unavailable; the declared administrative root connection(s) cannot be evaluated (fail closed)"
    return 0
  fi
  # EVERY listening port, not just the first: a second listener is exactly
  # where a Match LocalPort carve-out hides, and a declared lport that sshd
  # does not listen on is a declaration about some other host. `|| true`
  # because this is a bare assignment under `set -e` with inherit_errexit: an
  # sshd that exits non-zero must land on the named failure below, not abort
  # the run before it can print a RESULT line.
  global_config="$("$BCSP_SSHD" -T 2>/dev/null || true)"
  # Port AND ListenAddress: an admin listener declared as host:port appears
  # only in the latter, and refusing a truthful declaration of it would push
  # the operator toward declaring something false.
  ports="$(awk '
    $1 == "port" { print $2 }
    $1 == "listenaddress" {
      listen_port = $2
      sub(/^.*:/, "", listen_port)
      if (listen_port ~ /^[0-9]+$/) print listen_port
    }
  ' <<< "$global_config" | sort -u)"
  usedns="$(awk '$1 == "usedns" { print $2; exit }' <<< "$global_config")"
  if [[ -z "$ports" ]]; then
    pf_fail "cannot discover the sshd port from sshd -T; the declared root connection cannot be evaluated (fail closed)"
    return 0
  fi
  for spec in "${PREFLIGHT_ADMIN_SOURCES[@]}"; do
    # Each declaration is judged on its own merits and reported on its own
    # line: one unusable or unsafe path must not hide the next one.
    if ! canonical="$(preflight_parse_admin_source "$spec")"; then
      pf_fail "--admin-source '$spec' is not a usable root connection spec: $canonical (fail closed)"
      continue
    fi
    declared_host="${canonical#host=}"
    declared_host="${declared_host%%,*}"
    declared_addr="${canonical#*,addr=}"
    declared_addr="${declared_addr%%,*}"
    # With UseDNS off -- the OpenSSH default -- sshd never resolves the
    # client, so host= IS the address. A declaration naming anything else
    # describes a Match Host block that will never be evaluated.
    if [[ "$usedns" == "no" && "$declared_host" != "$declared_addr" ]]; then
      pf_fail "--admin-source '$spec' names host=$declared_host, but this sshd has UseDNS no and will see host=$declared_addr; the declared connection is not the one sshd evaluates (fail closed)"
      continue
    fi
    lport="${canonical##*,lport=}"
    if ! grep -qx -- "$lport" <<< "$ports"; then
      pf_fail "--admin-source '$spec' declares lport=$lport, which is not a port sshd listens on ($(paste -sd ',' - <<< "$ports")); the declared connection is not the real one (fail closed)"
      continue
    fi
    # The control probe below borrows this connection's local end, so it
    # only ever adopts a declaration that survived every check above.
    [[ -n "$control_reference" ]] || control_reference="$canonical"
    if ! triple="$(root_ssh_effective_triple "$canonical")"; then
      pf_fail "sshd -T -C for the declared root connection $canonical failed; cannot evaluate the effective root policy (fail closed)"
      continue
    fi
    read -r root password keyboard <<< "$triple"
    case "$root" in
      no|prohibit-password|without-password|forced-commands-only)
        pf_pass "root SSH password login is disabled for the declared root connection $canonical (PermitRootLogin $root)"
        ;;
      yes)
        # Root login allowed: only safe when no password-shaped method is
        # available to that connection.
        if [[ "$password" == "no" && "$keyboard" == "no" ]]; then
          pf_pass "root SSH allows keys only for the declared root connection $canonical (PermitRootLogin yes, PasswordAuthentication no, KbdInteractiveAuthentication no)"
        else
          pf_fail "root SSH password login is reachable for the declared root connection $canonical (PermitRootLogin yes, PasswordAuthentication $password, KbdInteractiveAuthentication $keyboard); fix sshd_config (separately authorized)"
        fi
        ;;
      *)
        pf_fail "unrecognized PermitRootLogin value '$root' for the declared root connection $canonical (fail closed)"
        ;;
    esac
  done

  # A CONTROL, never evidence. The declared connections above are what this
  # gate vouches for; this one documentation address exists only to produce
  # a refusal that no declaration can excuse, because root password login
  # reachable from an arbitrary internet source is a blocker whether or not
  # the operator's own path is hardened. R1 caught this shape with its
  # cross-tuple divergence rule; judging only declared paths would have lost
  # it. It can fail the run and it can never pass it: with no declaration
  # parsed, there is no reference connection and nothing is probed.
  if [[ -n "$control_reference" ]]; then
    control_laddr="${control_reference#*,laddr=}"
    control_laddr="${control_laddr%%,*}"
    control_lport="${control_reference##*,lport=}"
    if control_triple="$(root_ssh_effective_triple "host=203.0.113.1,addr=203.0.113.1,laddr=${control_laddr},lport=${control_lport}")"; then
      read -r control_root control_password control_keyboard <<< "$control_triple"
      if [[ "$control_root" == "yes" ]] &&
        { [[ "$control_password" != "no" ]] || [[ "$control_keyboard" != "no" ]]; }; then
        pf_fail "root SSH password login is reachable from an arbitrary internet source (PermitRootLogin yes, PasswordAuthentication $control_password, KbdInteractiveAuthentication $control_keyboard); the declared paths being safe does not close that door (fix sshd_config, separately authorized)"
      else
        pf_pass "root SSH password login is not reachable from an arbitrary source either (control probe, not go-live evidence)"
      fi
    else
      pf_warn "the arbitrary-source control probe could not be evaluated; the declared connections above are the evidence"
    fi
  fi
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

# H2 (R2): and the protection has to belong to the route the PUBLIC host
# actually reaches. R1 asked the whole document whether SOME proxy to
# 127.0.0.1:8080 carried the delay, so a protected proxy on a staging
# vhost answered for a public host that proxied somewhere else entirely --
# or served a static page and never touched the service at all.
#
# Four obligations, evaluated on the adapted JSON:
#   A  at least one reverse_proxy anywhere reaches the local service, and
#   B  every such proxy carries the delay                    (R1's rule, kept
#      because a severed socket belongs to the SERVICE, not to a vhost: an
#      unprotected sibling site still fails. Widened from R1's literal
#      127.0.0.1:8080 comparison to the same alias spellings clause D uses,
#      so the rule and the reason behind it agree)
#   C  a route that applies to the ORIGIN's host actually reaches the
#      service, and
#   D  every proxy on that host reaching the service carries the delay --
#      including the aliases (localhost:8080, [::1]:8080, :8080) that a
#      literal dial comparison never sees.
#
# Route matching is walked by name, never by `..`: a matcher or a nesting
# handler this program cannot interpret calls die(), jq exits non-zero, and
# the gate fails closed rather than guessing which host a proxy serves.
PREFLIGHT_CADDY_JQ='
def die($message): error("H2: " + $message);

# Caddy host matchers are case-insensitive; "*." covers exactly one label.
def host_pattern_matches($public):
  (ascii_downcase | sub("\\.$"; "")) as $pattern
  | if ($pattern | length) == 0 then die("empty host matcher")
    elif ($pattern | startswith("*.")) then
      ($pattern | ltrimstr("*.")) as $suffix
      | if ($suffix | length) == 0 or ($suffix | test("[*{}]"))
        then die("uninterpretable host matcher \($pattern)")
        elif ($public | endswith("." + $suffix))
        then ($public | .[0:(($public | length) - ($suffix | length) - 1)] | test("\\.") | not)
        else false
        end
    elif ($pattern | test("[*{}]")) then die("uninterpretable host matcher \($pattern)")
    else $pattern == $public
    end;

# Does one matcher set admit a request for $public?
#
# A set that constrains something other than the host (a path, a method) is
# a catch-all as far as the host is concerned. But three shapes can
# constrain the host in ways this program does not model -- a negated host,
# a CEL expression, a Host header matcher -- and reading those as
# catch-alls is how a route that explicitly EXCLUDES the public host would
# come to answer for it. They are refused.
def matcher_set_applies($public):
  if type != "object" then die("matcher set is not an object")
  else
    (if has("expression")
     then die("a CEL expression matcher may constrain the host; this check cannot interpret it")
     else . end)
    | (if (has("header") and ((.header | type) == "object")
           and (any((.header | keys_unsorted)[]; ascii_downcase == "host")))
       then die("a Host header matcher may constrain the host; this check cannot interpret it")
       else . end)
    | (if has("not") then
         (if (.not | type) != "array" then die("a not matcher is not an array")
          elif any(.not[]; (type == "object") and has("host"))
          then die("a negated host matcher cannot be interpreted; rewrite the site as a host block")
          else . end)
       else . end)
    | (if (has("host") | not) then true
       elif (.host | type) != "array" then die("host matcher is not an array")
       elif (.host | length) == 0 then die("empty host matcher list")
       else any(.host[];
             if type == "string" then host_pattern_matches($public)
             else die("non-string host matcher") end)
       end)
  end;

# match[] is an OR of matcher sets; no match at all is a catch-all.
def route_applies($public):
  if type != "object" then die("route is not an object")
  elif (has("match") | not) then true
  elif (.match | type) != "array" then die("route match is not an array")
  elif (.match | length) == 0 then true
  else any(.match[]; matcher_set_applies($public))
  end;

# A route that matches EVERY request for $public, not merely some of them.
# Only such a route can shadow the routes after it: the example config
# answers /metrics with a static response, and that must not be read as
# shadowing the proxy that follows it.
def matcher_set_is_host_only:
  (keys_unsorted | length) == 0
  or ((keys_unsorted | length) == 1 and (keys_unsorted[0] == "host"));

def route_is_unconditional($public):
  if (has("match") | not) then true
  elif (.match | type) != "array" then false
  elif (.match | length) == 0 then true
  else any(.match[]; matcher_set_is_host_only and matcher_set_applies($public))
  end;

# Handlers that answer the request themselves, so nothing after them runs.
def handler_terminates:
  (.handler? // "")
  | . == "static_response" or . == "file_server" or . == "abort" or . == "error";

# Every reverse_proxy a request for $public can actually REACH in this route
# list, in Caddy order: routes are tried in sequence, and the first
# unconditional route that ends the request (terminal, or a handler that
# answers) is the last one anything after it will ever see. A protected
# proxy sitting behind a maintenance page is not protection.
#
# "subroute" is the one handler the repository-supported configs use to nest
# routes; anything else that nests is refused rather than assumed active.
# handle_errors routes are deliberately NOT walked here: they run on an
# error, so they cannot prove that live traffic reaches the service. The
# whole-document obligation below still sees any proxy inside them.
def proxies_in_routes($public):
  (if type != "array" then die("route list is not an array") else . end)
  | [ to_entries[] | select(.value | route_applies($public)) ] as $applying
  | [ $applying[]
      | select(.value
          | route_is_unconditional($public)
            and ((.terminal? == true) or any((.handle // [])[]; handler_terminates)))
      | .key ] as $stops
  | (if ($stops | length) == 0 then $applying
     else ($stops | min) as $stop | [ $applying[] | select(.key <= $stop) ]
     end)
  | .[].value
  | (.handle // []) as $handlers
  | (if ($handlers | type) != "array" then die("route handle is not an array") else . end)
  | $handlers[]
  | if type != "object" then die("handler entry is not an object")
    elif (.handler? | type) != "string" then die("handler entry has no handler name")
    elif .handler == "reverse_proxy" then
      (if has("handle_response")
       then die("reverse_proxy nests handle_response routes")
       else . end)
    elif .handler == "subroute" then
      ((.routes // []) | proxies_in_routes($public))
    elif (has("routes") or has("handle")) then
      die("handler \(.handler) nests routes this check cannot interpret")
    else empty
    end;

# Only a server listening on the origin port carries the public traffic.
# A server with no listen list at all is unknowable, so it is included
# rather than excluded: over-including can only add obligations.
def serves_public_port($public_port):
  (.listen // []) as $listen
  | if ($listen | type) != "array" then die("server listen is not an array")
    elif ($listen | length) == 0 then true
    else any($listen[];
          if type != "string" then die("listen entry is not a string")
          else (sub("^[a-z0-9]+/"; "") | sub("^.*:"; "")) as $listen_port
            | if ($listen_port | test("^[0-9]+$")) then $listen_port == $public_port
              else die("cannot read a port out of the listen address \(.)")
              end
          end)
    end;

def dials:
  .upstreams as $upstreams
  | if ($upstreams | type) != "array" or ($upstreams | length) == 0
    then die("reverse_proxy without upstreams")
    else $upstreams[]
      | if type == "object" and (.dial? | type) == "string" and (.dial | length) > 0
        then .dial
        else die("upstream without a usable dial")
        end
    end;

# The same local service under any of the spellings Caddy accepts. On the
# public host a dial this cannot classify (a unix socket, an SRV lookup) is a
# refusal: there we must vouch, so we may not shrug.
def reaches_service:
  . as $dial
  | (sub("^.*:"; "")) as $dial_port
  | if ($dial_port | test("^[0-9]+$") | not)
    then die("upstream dial \($dial) has no numeric port")
    elif $dial_port != "8080" then false
    else ($dial | sub(":[0-9]+$"; "") | ascii_downcase
          | . == "localhost" or . == "::1" or . == "[::1]" or . == ""
            or startswith("127."))
    end;

# The same question asked of ANY proxy in the document, where we are only
# looking for violations and an uninterpretable dial elsewhere must not
# refuse the whole config.
def dial_touches_service:
  if type != "string" then false
  else (sub("^.*:"; "")) as $dial_port
    | if $dial_port != "8080" then false
      else (sub(":[0-9]+$"; "") | ascii_downcase
            | . == "localhost" or . == "::1" or . == "[::1]" or . == ""
              or startswith("127."))
      end
  end;

($host | ascii_downcase | sub("\\.$"; "")) as $public
| [ (.apps.http.servers // die("adapted config has no apps.http.servers"))
    | if type != "object" then die("apps.http.servers is not an object") else .[] end
    | if type != "object" then die("server entry is not an object") else . end
    | select(serves_public_port($port)) ] as $public_servers
| [ $public_servers[] | (.routes // []) | proxies_in_routes($public) ] as $host_proxies
| [ .. | objects
    | select(.handler? == "reverse_proxy")
    | select(any((.upstreams // [])[]; .dial? | dial_touches_service)) ] as $service_proxies
| if ($public_servers | length) == 0
  then "no adapted server listens on port \($port), the port of the public origin"
  elif ($service_proxies | length) == 0
  then "no reverse_proxy anywhere reaches 127.0.0.1:8080"
  elif (all($service_proxies[]; .stream_close_delay? == $delay) | not)
  then "a reverse_proxy to 127.0.0.1:8080 carries no stream_close_delay 4h"
  elif (any($host_proxies[]; any(dials; reaches_service)) | not)
  then "no active route for \($public) reaches 127.0.0.1:8080"
  elif (all($host_proxies[];
            if any(dials; reaches_service)
            then (.stream_close_delay? == $delay) else true end) | not)
  then "an active reverse_proxy for \($public) carries no stream_close_delay 4h"
  else "OK"
  end
'

check_caddy() {
  local caddyfile="$1" public_host="${2:-}" public_port="${3:-}" adapted verdict

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
  # Bind first: without a usable public host there is no route to judge, and
  # a verdict about "some proxy somewhere" is exactly the false pass R2
  # exists to remove.
  if [[ -z "$public_host" ]]; then
    pf_fail "cannot bind the adapted Caddy judgment to a public host (BCSP_PUBLIC_ORIGIN is unusable); refusing to vouch for any proxy (fail closed)"
    return 0
  fi
  if ! [[ "$public_host" =~ ^[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$ ]]; then
    pf_fail "the public host derived from BCSP_PUBLIC_ORIGIN is not a plain DNS name ('$public_host'); cannot bind the adapted Caddy judgment (fail closed)"
    return 0
  fi
  if ! [[ "$public_port" =~ ^[0-9]+$ ]]; then
    pf_fail "cannot tell which port BCSP_PUBLIC_ORIGIN is served on; cannot bind the adapted Caddy judgment (fail closed)"
    return 0
  fi
  if ! command -v "$BCSP_JQ" >/dev/null 2>&1; then
    pf_fail "jq is unavailable; cannot evaluate the adapted Caddy config (fail closed)"
    return 0
  fi
  if ! adapted="$("$BCSP_CADDY" adapt --config "$caddyfile" --adapter caddyfile 2>/dev/null)"; then
    pf_fail "caddy adapt refused $caddyfile; cannot evaluate the active proxy semantics (fail closed)"
    return 0
  fi
  # The program answers with the REASON it refused, not merely a boolean, so
  # a failure names the obligation it broke instead of leaving the operator
  # to guess which of four it was. stderr is folded in: a die() from the
  # program names the structure it could not interpret, and a jq that cannot
  # run at all lands in the same failure branch.
  if ! verdict="$("$BCSP_JQ" -r --arg host "$public_host" --arg port "$public_port" \
    --argjson delay "$PREFLIGHT_STREAM_CLOSE_DELAY_NS" \
    "$PREFLIGHT_CADDY_JQ" <<< "$adapted" 2>&1)"; then
    verdict="${verdict:-jq could not evaluate the adapted config}"
  fi
  if [[ "$verdict" == "OK" ]]; then
    pf_pass "the adapted route for $public_host:$public_port reaches 127.0.0.1:8080 and every reverse_proxy to the service carries stream_close_delay 4h (H2)"
  else
    pf_fail "the adapted config does not show $public_host's own active route reaching 127.0.0.1:8080 with stream_close_delay 4h on every reverse_proxy to the service -- $verdict; a reload would sever every monitoring socket (comments and other sites do not count)"
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
  local usage="usage: preflight.sh --admin-source addr=A,host=H,laddr=L,lport=P [--admin-source ...] [--caddyfile PATH]"

  # An unknown FLAG is a usage error; a bad flag VALUE or a missing
  # declaration is a preflight finding, so the run still reports every other
  # fact beside it.
  PREFLIGHT_ADMIN_SOURCES=()
  while [[ "$#" -gt 0 ]]; do
    case "$1" in
      --caddyfile)
        [[ "$#" -ge 2 ]] || bcsp_die "$usage"
        caddyfile="$2"
        shift 2
        ;;
      --admin-source)
        [[ "$#" -ge 2 ]] || bcsp_die "$usage"
        PREFLIGHT_ADMIN_SOURCES+=("$2")
        shift 2
        ;;
      *)
        bcsp_die "$usage"
        ;;
    esac
  done
  bcsp_require_privilege

  PREFLIGHT_HOST=""
  PREFLIGHT_PORT=""
  check_origin
  if [[ -n "$PREFLIGHT_HOST" ]]; then
    check_dns "$PREFLIGHT_HOST"
  fi
  check_firewall
  check_root_ssh
  check_failed_units
  check_caddy "$caddyfile" "$PREFLIGHT_HOST" "$PREFLIGHT_PORT"
  check_service

  if [[ "$PREFLIGHT_FAILURES" -gt 0 ]]; then
    printf 'preflight: RESULT FAIL (%s failure(s), %s advisory(ies))\n' \
      "$PREFLIGHT_FAILURES" "$PREFLIGHT_ADVISORIES" >&2
    exit 1
  fi
  printf 'preflight: RESULT PASS (%s advisory(ies))\n' "$PREFLIGHT_ADVISORIES"
}

main "$@"
