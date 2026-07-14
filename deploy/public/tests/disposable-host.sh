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
for required in share/bcsp systemd caddy config ops docs; do
  [[ -d "$CANDIDATE_ROOT/$required" ]] || {
    printf 'disposable-host: candidate is missing %s/\n' "$required" >&2
    exit 1
  }
done
[[ ! -e "$CANDIDATE_ROOT/tests" ]] || {
  printf 'disposable-host: final candidate must not contain tests/\n' >&2
  exit 1
}
expected_top_level=$'bin\ncaddy\nconfig\ndocs\nops\nshare\nsystemd'
actual_top_level="$(find "$CANDIDATE_ROOT" -mindepth 1 -maxdepth 1 -printf '%f\n' | sort)"
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
[[ -z "$(grep -Ev '^(#.*|[[:space:]]*|BCSP_PUBLIC_ORIGIN=https://planner.invalid)$' \
  "$CANDIDATE_ROOT/config/bcsp.env.example")" ]]
jq -e '
  .type == "object" and
  .additionalProperties == false and
  .required == ["BCSP_PUBLIC_ORIGIN"] and
  (.properties | keys) == ["BCSP_PUBLIC_ORIGIN"] and
  .secretVariables == []
' "$CANDIDATE_ROOT/config/bcsp.env.schema.json" >/dev/null
caddy validate --config "$CANDIDATE_ROOT/caddy/Caddyfile.example" --adapter caddyfile >/dev/null

TEST_TMP="$(mktemp -d)"
DROP_IN_ROOT=/etc/systemd/system/bcsp.service.d
cleanup() {
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

install -d -m 0755 "$DROP_IN_ROOT"
cat > "$DROP_IN_ROOT/90-disposable-network.conf" <<'DROPIN'
[Service]
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

live_status="$(curl --silent --output /dev/null --write-out '%{http_code}' \
  --header 'Host: planner.invalid' http://127.0.0.1:8080/health/live)"
ready_status="$(curl --silent --output /dev/null --write-out '%{http_code}' \
  --header 'Host: planner.invalid' http://127.0.0.1:8080/health/ready)"
[[ "$live_status" == "200" ]]
[[ "$ready_status" == "503" ]]

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
     ORDER BY name;")
[[ "${#business_tables[@]}" -gt 0 ]]
for table in "${business_tables[@]}"; do
  [[ "$table" =~ ^[A-Za-z0-9_]+$ ]]
  table_rows="$(sqlite3 "$DATABASE" "SELECT count(*) FROM \"$table\";")"
  [[ "$table_rows" -eq 0 ]]
done

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
for required in share systemd caddy config ops docs; do
  cp -R -- "$CANDIDATE_ROOT/$required" "$BAD_CANDIDATE/$required"
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
printf 'P7_1_014_DISPOSABLE_HOST_PASS\n'
