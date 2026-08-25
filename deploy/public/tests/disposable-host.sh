#!/usr/bin/env bash

# Destructive only to a disposable Linux host. This test intentionally uses the
# real FHS paths, system user, systemd service, SQLite CLI, and candidate binary.
set -Eeuo pipefail

usage() {
  printf 'usage: disposable-host.sh --candidate-root PATH\n' >&2
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

[[ "$EUID" -eq 0 ]] || { printf 'disposable-host: run as root\n' >&2; exit 1; }
[[ "${BCSP_DISPOSABLE_HOST_CONFIRM:-}" == "YES" ]] || {
  printf 'disposable-host: set BCSP_DISPOSABLE_HOST_CONFIRM=YES on a disposable host\n' >&2
  exit 1
}
[[ -n "$CANDIDATE_ROOT" ]] || usage
CANDIDATE_ROOT="$(cd -- "$CANDIDATE_ROOT" && pwd)"
[[ -x "$CANDIDATE_ROOT/bin/bcsp-server" ]] || {
  printf 'disposable-host: candidate is missing executable bin/bcsp-server\n' >&2
  exit 1
}
for required in systemd caddy config ops docs; do
  [[ -d "$CANDIDATE_ROOT/$required" ]] || {
    printf 'disposable-host: candidate is missing %s/\n' "$required" >&2
    exit 1
  }
done
for required in BUILD-PROVENANCE.json FRONTEND-CAPABILITIES.json LICENSE \
  MANIFEST.json SBOM.cdx.json SHA256SUMS THIRD-PARTY-NOTICES.txt VERSION; do
  [[ -f "$CANDIDATE_ROOT/$required" ]] || {
    printf 'disposable-host: candidate is missing %s\n' "$required" >&2
    exit 1
  }
done
[[ ! -e "$CANDIDATE_ROOT/tests" ]] || {
  printf 'disposable-host: final candidate must not contain tests/\n' >&2
  exit 1
}
expected_top_level=$'BUILD-PROVENANCE.json\nFRONTEND-CAPABILITIES.json\nLICENSE\nMANIFEST.json\nSBOM.cdx.json\nSHA256SUMS\nTHIRD-PARTY-NOTICES.txt\nVERSION\nbin\ncaddy\nconfig\ndocs\nops\nsystemd'
actual_top_level="$(find "$CANDIDATE_ROOT" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort)"
[[ "$actual_top_level" == "$expected_top_level" ]] || {
  printf 'disposable-host: candidate top-level allowlist mismatch\n' >&2
  exit 1
}
[[ -z "$(find "$CANDIDATE_ROOT" -type l -print -quit)" ]] || {
  printf 'disposable-host: candidate contains a symbolic link\n' >&2
  exit 1
}
[[ -z "$(find "$CANDIDATE_ROOT" ! -type d ! -type f -print -quit)" ]] || {
  printf 'disposable-host: candidate contains a special file\n' >&2
  exit 1
}
forbidden_file="$(find "$CANDIDATE_ROOT" -type f \( \
  -name 'bcsp.env' -o -iname '*.sqlite' -o -iname '*.sqlite3' -o -iname '*.db' -o \
  -iname '*.pem' -o -iname '*.key' -o -iname '*.crt' -o -iname '*.p12' \
  \) -print -quit)"
[[ -z "$forbidden_file" ]] || {
  printf 'disposable-host: candidate contains real configuration/data material: %s\n' "$forbidden_file" >&2
  exit 1
}

for command_name in caddy curl flock jq sha256sum sqlite3 systemctl systemd-analyze useradd userdel; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'disposable-host: required command is absent: %s\n' "$command_name" >&2
    exit 1
  }
done
[[ -d /run/systemd/system ]] || {
  printf 'disposable-host: systemd is not running\n' >&2
  exit 1
}

for path in /opt/bcsp /etc/bcsp /var/lib/bcsp /var/backups/bcsp \
  /etc/systemd/system/bcsp.service /etc/systemd/system/bcsp.service.d; do
  [[ ! -e "$path" ]] || {
    printf 'disposable-host: host is not clean: %s already exists\n' "$path" >&2
    exit 1
  }
done
if id -u bcsp >/dev/null 2>&1; then
  printf 'disposable-host: host is not clean: bcsp user already exists\n' >&2
  exit 1
fi

[[ "$(grep -c '^BCSP_PUBLIC_ORIGIN=https://planner.invalid$' \
  "$CANDIDATE_ROOT/config/bcsp.env.example")" -eq 1 ]]
if grep -Evq '^(#.*|[[:space:]]*|BCSP_PUBLIC_ORIGIN=https://planner.invalid)$' \
  "$CANDIDATE_ROOT/config/bcsp.env.example"; then
  printf 'disposable-host: environment example contains an unexpected setting\n' >&2
  exit 1
fi
jq -e '
  .type == "object" and
  .additionalProperties == false and
  .required == ["BCSP_PUBLIC_ORIGIN"] and
  (.properties | keys) == ["BCSP_PUBLIC_ORIGIN", "BCSP_PUBLIC_WS_PER_CLIENT_LIMIT"] and
  .secretVariables == []
' "$CANDIDATE_ROOT/config/bcsp.env.schema.json" >/dev/null
caddy validate --config "$CANDIDATE_ROOT/caddy/Caddyfile.example" --adapter caddyfile >/dev/null

TEST_TMP="$(mktemp -d)"
DROP_IN_ROOT=/etc/systemd/system/bcsp.service.d
CADDY_ACCEPTANCE_STARTED=0
ACCEPTANCE_CA_PATH=""
ACCEPTANCE_HOSTS_ADDED=0
CADDY_DATA_ROOT="$TEST_TMP/caddy-data"
CADDY_CONFIG_ROOT="$TEST_TMP/caddy-config"

caddy_acceptance() {
  env XDG_DATA_HOME="$CADDY_DATA_ROOT" XDG_CONFIG_HOME="$CADDY_CONFIG_ROOT" \
    caddy "$@"
}

cleanup() {
  if [[ "$CADDY_ACCEPTANCE_STARTED" -eq 1 ]]; then
    caddy_acceptance stop >/dev/null 2>&1 || true
  fi
  if [[ -n "$ACCEPTANCE_CA_PATH" && -e "$ACCEPTANCE_CA_PATH" ]]; then
    rm -f -- "$ACCEPTANCE_CA_PATH"
    update-ca-certificates >/dev/null 2>&1 || true
  fi
  if [[ "$ACCEPTANCE_HOSTS_ADDED" -eq 1 ]]; then
    sed -i '\|^127\.0\.0\.1 planner\.test # bcsp-final-candidate$|d' /etc/hosts || true
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

UNEXPECTED_WEB_ROOT="$TEST_TMP/unexpected-external-web"
cp -a -- "$CANDIDATE_ROOT" "$UNEXPECTED_WEB_ROOT"
mkdir -p "$UNEXPECTED_WEB_ROOT/share/bcsp"
if bash -c 'source "$1"; bcsp_validate_candidate_root "$2"' _ \
  "$CANDIDATE_ROOT/ops/lib.sh" "$UNEXPECTED_WEB_ROOT" >/dev/null 2>&1; then
  printf 'disposable-host: candidate validator accepted external web assets\n' >&2
  exit 1
fi

install -d -m 0755 "$DROP_IN_ROOT"
cat > "$DROP_IN_ROOT/90-disposable-network.conf" <<'DROPIN'
[Service]
Environment=BCSP_CI_NO_RUTGERS=1
IPAddressDeny=any
IPAddressAllow=127.0.0.0/8
IPAddressAllow=::1/128
DROPIN
chmod 0644 "$DROP_IN_ROOT/90-disposable-network.conf"

ENV_SOURCE="$TEST_TMP/bcsp.env"
printf '%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.invalid' > "$ENV_SOURCE"
export BCSP_ENV_SOURCE="$ENV_SOURCE"
export BCSP_HEALTH_ATTEMPTS=8
export BCSP_HEALTH_DELAY_SECONDS=1
OPS_ROOT="$CANDIDATE_ROOT/ops"

bash "$OPS_ROOT/install.sh" ci-v1 "$CANDIDATE_ROOT"
systemd-analyze verify /etc/systemd/system/bcsp.service
systemd-analyze security --no-pager bcsp.service > "$TEST_TMP/systemd-security.txt"
bash "$OPS_ROOT/verify.sh"
for metadata in BUILD-PROVENANCE.json FRONTEND-CAPABILITIES.json LICENSE \
  MANIFEST.json SBOM.cdx.json SHA256SUMS THIRD-PARTY-NOTICES.txt VERSION; do
  cmp -s -- "$CANDIDATE_ROOT/$metadata" "/opt/bcsp/releases/ci-v1/$metadata"
done
[[ ! -e /opt/bcsp/releases/ci-v1/share ]]

live_status="$(curl --silent --output /dev/null --write-out '%{http_code}' \
  --header 'Host: planner.invalid' http://127.0.0.1:8080/health/live)"
ready_status="$(curl --silent --output /dev/null --write-out '%{http_code}' \
  --header 'Host: planner.invalid' http://127.0.0.1:8080/health/ready)"
[[ "$live_status" == "200" ]]
[[ "$ready_status" == "503" ]]

DOCUMENT_BODY="$TEST_TMP/public.html"
document_status="$(curl --silent --show-error --output "$DOCUMENT_BODY" \
  --write-out '%{http_code}' --header 'Host: planner.invalid' http://127.0.0.1:8080/)"
[[ "$document_status" == "200" ]]
grep -q 'id="root"' "$DOCUMENT_BODY"
grep -q 'id="bcsp-bootstrap"' "$DOCUMENT_BODY"
asset_path="$(grep -m 1 -oE 'src="(\./|/)?assets/[A-Za-z0-9._-]+\.js"' \
  "$DOCUMENT_BODY" | cut -d '"' -f 2)"
asset_path="${asset_path#./}"
asset_path="${asset_path#/}"
[[ "$asset_path" == assets/*.js ]]
ASSET_BODY="$TEST_TMP/public.js"
ASSET_HEADERS="$TEST_TMP/public-js.headers"
asset_status="$(curl --silent --show-error --dump-header "$ASSET_HEADERS" \
  --output "$ASSET_BODY" --write-out '%{http_code}' --header 'Host: planner.invalid' \
  "http://127.0.0.1:8080/$asset_path")"
[[ "$asset_status" == "200" && -s "$ASSET_BODY" ]]
grep -Eqi '^content-type: (application|text)/(javascript|x-javascript)' "$ASSET_HEADERS"

DATABASE=/var/lib/bcsp/rbcsp.sqlite
[[ -f "$DATABASE" ]]
schema_tables="$(sqlite3 "$DATABASE" \
  "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';")"
[[ "$schema_tables" -gt 0 ]]
mapfile -t business_tables < <(sqlite3 "$DATABASE" \
  "SELECT name FROM sqlite_master
     WHERE type='table'
       AND name NOT LIKE 'sqlite_%'
       AND name <> 'bcsp_operational_migrations'
       AND name <> 'catalog_discovery_state'
       AND (name = 'catalog_course_fts' OR name NOT LIKE 'catalog_course_fts_%')
     ORDER BY name;")
[[ "${#business_tables[@]}" -gt 0 ]]
for table in "${business_tables[@]}"; do
  [[ "$table" =~ ^[A-Za-z0-9_]+$ ]]
  table_rows="$(sqlite3 "$DATABASE" "SELECT count(*) FROM \"$table\";")"
  if [[ "$table_rows" -ne 0 ]]; then
    printf 'disposable-host: first-start table %s contains %s unexpected row(s)\n' \
      "$table" "$table_rows" >&2
    exit 1
  fi
done
discovery_sentinel="$(sqlite3 "$DATABASE" \
  'SELECT count(*) FROM catalog_discovery_state
    WHERE singleton = 1
      AND content_version = 0
      AND semantic_sha256 IS NULL
      AND last_attempt_observation_id IS NULL
      AND last_success_observation_id IS NULL
      AND last_published_observation_id IS NULL
      AND last_nonempty_observation_id IS NULL;')"
if [[ "$discovery_sentinel" -ne 1 ]]; then
  printf 'disposable-host: initial discovery state is not the empty sentinel\n' >&2
  exit 1
fi

# H6: crash-loop recovery. StartLimitIntervalSec=0 means systemd never
# stops retrying a failed service; six SIGKILLs in a row would trip the old
# 5-per-minute start limit on the sixth kill, so surviving all six is the
# discriminating proof, not a formality.
for kill_attempt in 1 2 3 4 5 6; do
  crashed_pid="$(systemctl show bcsp.service --property=MainPID --value)"
  if ! [[ "$crashed_pid" =~ ^[0-9]+$ && "$crashed_pid" -gt 0 ]]; then
    printf 'disposable-host: no MainPID before SIGKILL %s of 6\n' "$kill_attempt" >&2
    exit 1
  fi
  kill -9 "$crashed_pid"
  recovered=0
  for _ in {1..60}; do
    sleep 1
    revived_pid="$(systemctl show bcsp.service --property=MainPID --value)"
    if [[ "$revived_pid" =~ ^[0-9]+$ && "$revived_pid" -gt 0 && \
      "$revived_pid" != "$crashed_pid" ]] && systemctl is-active --quiet bcsp.service; then
      recovered=1
      break
    fi
  done
  if [[ "$recovered" -ne 1 ]]; then
    printf 'disposable-host: service did not recover after SIGKILL %s of 6\n' \
      "$kill_attempt" >&2
    exit 1
  fi
done
kill_live_status=""
for _ in {1..30}; do
  kill_live_status="$(curl --silent --output /dev/null --write-out '%{http_code}' \
    --header 'Host: planner.invalid' http://127.0.0.1:8080/health/live || true)"
  [[ "$kill_live_status" == "200" ]] && break
  sleep 1
done
if [[ "$kill_live_status" != "200" ]]; then
  printf 'disposable-host: liveness did not return after the crash-loop drill\n' >&2
  exit 1
fi

sqlite3 "$DATABASE" \
  'CREATE TABLE ops_disposable_proof(value TEXT NOT NULL); INSERT INTO ops_disposable_proof VALUES ("baseline");'
MANUAL_BACKUP="$(bash "$OPS_ROOT/backup.sh")"
[[ -d "$MANUAL_BACKUP" && -f "$MANUAL_BACKUP/manifest" ]]
expected="$(awk -F= '$1 == "sha256" { print $2 }' "$MANUAL_BACKUP/manifest")"
actual="$(sha256sum "$MANUAL_BACKUP/rbcsp.sqlite" | awk '{ print $1 }')"
[[ "$expected" == "$actual" ]]
sqlite3 "$DATABASE" 'UPDATE ops_disposable_proof SET value = "changed";'
printf 'stale rollback journal' > "${DATABASE}-journal"
bash "$OPS_ROOT/restore.sh" "$MANUAL_BACKUP"
[[ "$(sqlite3 "$DATABASE" 'SELECT value FROM ops_disposable_proof;')" == "baseline" ]]
[[ ! -e "${DATABASE}-journal" ]]

bash "$OPS_ROOT/upgrade.sh" ci-v2 "$CANDIDATE_ROOT"
[[ "$(basename -- "$(readlink /opt/bcsp/current)")" == "ci-v2" ]]
sqlite3 "$DATABASE" 'UPDATE ops_disposable_proof SET value = "after-v2";'
bash "$OPS_ROOT/rollback.sh" ci-v1
[[ "$(basename -- "$(readlink /opt/bcsp/current)")" == "ci-v1" ]]
[[ "$(sqlite3 "$DATABASE" 'SELECT value FROM ops_disposable_proof;')" == "baseline" ]]

bash "$OPS_ROOT/upgrade.sh" ci-v2 "$CANDIDATE_ROOT"
sqlite3 "$DATABASE" 'UPDATE ops_disposable_proof SET value = "before-bad";'
BAD_CANDIDATE="$TEST_TMP/bad-candidate"
mkdir -p "$BAD_CANDIDATE/bin"
for required in systemd caddy config ops docs; do
  cp -R -- "$CANDIDATE_ROOT/$required" "$BAD_CANDIDATE/$required"
done
for required in BUILD-PROVENANCE.json FRONTEND-CAPABILITIES.json LICENSE \
  MANIFEST.json SBOM.cdx.json SHA256SUMS THIRD-PARTY-NOTICES.txt VERSION; do
  cp -- "$CANDIDATE_ROOT/$required" "$BAD_CANDIDATE/$required"
done
cat > "$BAD_CANDIDATE/bin/bcsp-server" <<'BAD_BINARY'
#!/usr/bin/env bash
sqlite3 /var/lib/bcsp/rbcsp.sqlite \
  'UPDATE ops_disposable_proof SET value = "bad-migration";'
exit 1
BAD_BINARY
chmod 0755 "$BAD_CANDIDATE/bin/bcsp-server"
if bash "$OPS_ROOT/upgrade.sh" bad "$BAD_CANDIDATE"; then
  printf 'disposable-host: failing release unexpectedly passed\n' >&2
  exit 1
fi
[[ "$(basename -- "$(readlink /opt/bcsp/current)")" == "ci-v2" ]]
[[ "$(sqlite3 "$DATABASE" 'SELECT value FROM ops_disposable_proof;')" == "before-bad" ]]
grep -q '^status=AUTO_ROLLED_BACK$' /var/backups/bcsp/.ops/last-upgrade

bash "$OPS_ROOT/verify.sh"
[[ "$(stat -c '%U:%G %a' /var/backups/bcsp)" == "root:root 700" ]]
[[ "$(stat -c '%U:%G %a' /var/backups/bcsp/.ops/last-upgrade)" == "root:root 600" ]]
[[ ! -e /etc/caddy/Caddyfile.d/rbcsp ]]

if [[ -n "${BCSP_BROWSER_ACCEPTANCE_SCRIPT:-}" ]]; then
  [[ -f "$BCSP_BROWSER_ACCEPTANCE_SCRIPT" && ! -L "$BCSP_BROWSER_ACCEPTANCE_SCRIPT" ]] || {
    printf 'disposable-host: browser acceptance script is not a regular file\n' >&2
    exit 1
  }
  [[ -n "${BCSP_PLAYWRIGHT_ROOT:-}" && -f "$BCSP_PLAYWRIGHT_ROOT/package.json" ]] || {
    printf 'disposable-host: BCSP_PLAYWRIGHT_ROOT is not a frontend package root\n' >&2
    exit 1
  }
  BROWSER_NODE="${BCSP_NODE_BIN:-$(command -v node || true)}"
  [[ -n "$BROWSER_NODE" ]] || {
    printf 'disposable-host: Node.js is required for browser acceptance\n' >&2
    exit 1
  }
  for command_name in awk certutil getent install sed update-ca-certificates; do
    command -v "$command_name" >/dev/null 2>&1 || {
      printf 'disposable-host: browser acceptance command is absent: %s\n' "$command_name" >&2
      exit 1
    }
  done
  if getent hosts planner.test >/dev/null 2>&1; then
    printf 'disposable-host: planner.test already resolves on the disposable host\n' >&2
    exit 1
  fi

  printf '%s\n' '127.0.0.1 planner.test # bcsp-final-candidate' >> /etc/hosts
  ACCEPTANCE_HOSTS_ADDED=1
  NO_PROXY="${NO_PROXY:+$NO_PROXY,}planner.test,127.0.0.1,localhost"
  no_proxy="$NO_PROXY"
  export NO_PROXY no_proxy
  printf '%s\n' 'BCSP_PUBLIC_ORIGIN=https://planner.test' > /etc/bcsp/bcsp.env
  chmod 0640 /etc/bcsp/bcsp.env
  chown root:bcsp /etc/bcsp/bcsp.env
  systemctl restart bcsp.service
  bash "$OPS_ROOT/verify.sh"

  CADDY_CONFIG="$TEST_TMP/Caddyfile.final-candidate"
  awk '
    /^planner\.invalid[[:space:]]*\{$/ {
      replacements += 1
      print "planner.test {"
      print "\ttls internal"
      next
    }
    { print }
    END { if (replacements != 1) exit 42 }
  ' "$CANDIDATE_ROOT/caddy/Caddyfile.example" > "$CADDY_CONFIG" || {
    printf 'disposable-host: Caddy example did not contain one planner.invalid site\n' >&2
    exit 1
  }
  caddy validate --config "$CADDY_CONFIG" --adapter caddyfile >/dev/null
  caddy_acceptance start --config "$CADDY_CONFIG" --adapter caddyfile
  CADDY_ACCEPTANCE_STARTED=1

  CADDY_CA_SOURCE="$CADDY_DATA_ROOT/caddy/pki/authorities/local/root.crt"
  for _ in {1..40}; do
    [[ -f "$CADDY_CA_SOURCE" ]] && break
    sleep 0.25
  done
  [[ -f "$CADDY_CA_SOURCE" ]] || {
    printf 'disposable-host: Caddy test CA was not generated\n' >&2
    exit 1
  }
  ACCEPTANCE_CA_PATH=/usr/local/share/ca-certificates/bcsp-final-candidate.crt
  install -m 0644 "$CADDY_CA_SOURCE" "$ACCEPTANCE_CA_PATH"
  update-ca-certificates >/dev/null
  BROWSER_HOME="$TEST_TMP/browser-home"
  BROWSER_NSSDB="$BROWSER_HOME/.pki/nssdb"
  install -d -m 0700 "$BROWSER_NSSDB"
  certutil -N --empty-password -d "sql:$BROWSER_NSSDB"
  certutil -A -d "sql:$BROWSER_NSSDB" -n bcsp-final-candidate -t 'C,,' \
    -i "$CADDY_CA_SOURCE"
  for _ in {1..40}; do
    if curl --fail --silent --show-error --connect-timeout 2 --max-time 5 \
      https://planner.test/ >/dev/null 2>&1; then
      break
    fi
    sleep 0.25
  done
  curl --fail --silent --show-error --connect-timeout 2 --max-time 5 \
    https://planner.test/ >/dev/null

  env HOME="$BROWSER_HOME" "$BROWSER_NODE" "$BCSP_BROWSER_ACCEPTANCE_SCRIPT" \
    --mode public \
    --base-url https://planner.test \
    --playwright-root "$BCSP_PLAYWRIGHT_ROOT"

  sqlite3 "$DATABASE" '.timeout 10000' \
    'CREATE TABLE caddy_cycle_proof(value TEXT NOT NULL); INSERT INTO caddy_cycle_proof VALUES ("preserve");'
  SERVICE_PID="$(systemctl show --property MainPID --value bcsp.service)"
  [[ "$SERVICE_PID" =~ ^[1-9][0-9]*$ ]] || {
    printf 'disposable-host: bcsp.service has no stable MainPID\n' >&2
    exit 1
  }

  caddy_acceptance stop
  CADDY_ACCEPTANCE_STARTED=0
  if curl --fail --silent --connect-timeout 1 --max-time 2 \
    https://planner.test/ >/dev/null 2>&1; then
    printf 'disposable-host: HTTPS remained available after Caddy stopped\n' >&2
    exit 1
  fi
  [[ "$(systemctl show --property MainPID --value bcsp.service)" == "$SERVICE_PID" ]]
  [[ "$(curl --silent --output /dev/null --write-out '%{http_code}' \
    --header 'Host: planner.test' http://127.0.0.1:8080/health/live)" == "200" ]]
  [[ "$(sqlite3 "$DATABASE" '.timeout 10000' 'SELECT value FROM caddy_cycle_proof;')" == "preserve" ]]
  [[ "$(sqlite3 "$DATABASE" '.timeout 10000' 'PRAGMA integrity_check;')" == "ok" ]]

  caddy_acceptance start --config "$CADDY_CONFIG" --adapter caddyfile
  CADDY_ACCEPTANCE_STARTED=1
  for _ in {1..40}; do
    if curl --fail --silent --show-error --connect-timeout 2 --max-time 5 \
      https://planner.test/ >/dev/null 2>&1; then
      break
    fi
    sleep 0.25
  done
  curl --fail --silent --show-error --connect-timeout 2 --max-time 5 \
    https://planner.test/ >/dev/null
  [[ "$(systemctl show --property MainPID --value bcsp.service)" == "$SERVICE_PID" ]]
  [[ "$(sqlite3 "$DATABASE" '.timeout 10000' 'SELECT value FROM caddy_cycle_proof;')" == "preserve" ]]
  [[ "$(sqlite3 "$DATABASE" '.timeout 10000' 'PRAGMA integrity_check;')" == "ok" ]]
  env HOME="$BROWSER_HOME" "$BROWSER_NODE" "$BCSP_BROWSER_ACCEPTANCE_SCRIPT" \
    --mode public \
    --base-url https://planner.test \
    --playwright-root "$BCSP_PLAYWRIGHT_ROOT"
  bash "$OPS_ROOT/verify.sh"
fi

printf 'P7_1_014_DISPOSABLE_HOST_PASS\n'
