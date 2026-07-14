#!/usr/bin/env bash

# Shared Linux operations for the public RBCSP service.
# shellcheck shell=bash

set -Eeuo pipefail
shopt -s inherit_errexit

readonly BCSP_SERVICE_NAME="${BCSP_SERVICE_NAME:-bcsp.service}"
readonly BCSP_SERVICE_USER="${BCSP_SERVICE_USER:-bcsp}"
readonly BCSP_SERVICE_GROUP="${BCSP_SERVICE_GROUP:-bcsp}"
readonly BCSP_ROOT="${BCSP_ROOT:-}"
readonly BCSP_SYSTEMCTL="${BCSP_SYSTEMCTL:-systemctl}"
readonly BCSP_CURL="${BCSP_CURL:-curl}"
readonly BCSP_SQLITE3="${BCSP_SQLITE3:-sqlite3}"
readonly BCSP_SHA256SUM="${BCSP_SHA256SUM:-sha256sum}"
readonly BCSP_FLOCK="${BCSP_FLOCK:-flock}"
readonly BCSP_HEALTH_URL="${BCSP_HEALTH_URL:-http://127.0.0.1:8080/health/live}"
readonly BCSP_HEALTH_ATTEMPTS="${BCSP_HEALTH_ATTEMPTS:-30}"
readonly BCSP_HEALTH_DELAY_SECONDS="${BCSP_HEALTH_DELAY_SECONDS:-1}"

bcsp_log() {
  printf 'bcsp-ops: %s\n' "$*" >&2
}

bcsp_die() {
  bcsp_log "ERROR: $*"
  exit 1
}

bcsp_path() {
  local absolute_path="$1"

  [[ "$absolute_path" == /* ]] || bcsp_die "internal path is not absolute: $absolute_path"
  if [[ -z "$BCSP_ROOT" || "$BCSP_ROOT" == "/" ]]; then
    printf '%s\n' "$absolute_path"
  else
    [[ "$BCSP_ROOT" == /* ]] || bcsp_die "BCSP_ROOT must be an absolute path"
    printf '%s%s\n' "${BCSP_ROOT%/}" "$absolute_path"
  fi
}

bcsp_opt_root() { bcsp_path /opt/bcsp; }
bcsp_releases_root() { bcsp_path /opt/bcsp/releases; }
bcsp_current_link() { bcsp_path /opt/bcsp/current; }
bcsp_config_root() { bcsp_path /etc/bcsp; }
bcsp_environment_file() { bcsp_path /etc/bcsp/bcsp.env; }
bcsp_state_root() { bcsp_path /var/lib/bcsp; }
bcsp_database_path() { bcsp_path /var/lib/bcsp/rbcsp.sqlite; }
bcsp_operations_root() { bcsp_path /var/backups/bcsp/.ops; }
bcsp_last_upgrade_file() { bcsp_path /var/backups/bcsp/.ops/last-upgrade; }
bcsp_backup_root() { bcsp_path /var/backups/bcsp; }
bcsp_unit_path() { bcsp_path /etc/systemd/system/bcsp.service; }

bcsp_require_command() {
  command -v "$1" >/dev/null 2>&1 || bcsp_die "required command is unavailable: $1"
}

bcsp_require_privilege() {
  if [[ -z "$BCSP_ROOT" || "$BCSP_ROOT" == "/" ]]; then
    [[ "$EUID" -eq 0 ]] || bcsp_die "run as root"
  fi
}

bcsp_validate_release_id() {
  local release_id="$1"

  [[ "$release_id" != "." && "$release_id" != ".." ]] || return 1
  [[ "$release_id" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]]
}

bcsp_require_release_id() {
  local release_id="${1:-}"

  bcsp_validate_release_id "$release_id" || \
    bcsp_die "release id must be 1-128 safe filename characters"
}

bcsp_ensure_service_user() {
  local passwd_entry passwd_home passwd_shell primary_group

  if [[ "${BCSP_SKIP_USER_MANAGEMENT:-0}" == "1" ]]; then
    return
  fi

  bcsp_require_command id
  bcsp_require_command getent
  bcsp_require_command useradd
  if id -u "$BCSP_SERVICE_USER" >/dev/null 2>&1; then
    passwd_entry="$(getent passwd "$BCSP_SERVICE_USER")" || \
      bcsp_die "existing service user has no passwd entry"
    IFS=: read -r _ _ _ _ _ passwd_home passwd_shell <<< "$passwd_entry"
    primary_group="$(id -gn "$BCSP_SERVICE_USER")"
    [[ "$primary_group" == "$BCSP_SERVICE_GROUP" ]] || \
      bcsp_die "existing service user has an unexpected primary group"
    [[ "$passwd_home" == "/nonexistent" && "$passwd_shell" == "/usr/sbin/nologin" ]] || \
      bcsp_die "existing service user is not the expected non-login system account"
    return
  fi
  if getent group "$BCSP_SERVICE_GROUP" >/dev/null 2>&1; then
    useradd --system --gid "$BCSP_SERVICE_GROUP" --home-dir /nonexistent \
      --no-create-home --shell /usr/sbin/nologin "$BCSP_SERVICE_USER"
  else
    [[ "$BCSP_SERVICE_USER" == "$BCSP_SERVICE_GROUP" ]] || \
      bcsp_die "service group is absent and differs from service user"
    useradd --system --user-group --home-dir /nonexistent \
      --no-create-home --shell /usr/sbin/nologin "$BCSP_SERVICE_USER"
  fi
}

bcsp_chown() {
  if [[ "${BCSP_SKIP_OWNERSHIP:-0}" == "1" ]]; then
    return
  fi
  chown "$@"
}

bcsp_ensure_directories() {
  local opt_root releases_root config_root state_root operations_root backup_root unit_root

  opt_root="$(bcsp_opt_root)"
  releases_root="$(bcsp_releases_root)"
  config_root="$(bcsp_config_root)"
  state_root="$(bcsp_state_root)"
  operations_root="$(bcsp_operations_root)"
  backup_root="$(bcsp_backup_root)"
  unit_root="$(dirname "$(bcsp_unit_path)")"

  install -d -m 0755 "$opt_root" "$releases_root" "$unit_root"
  install -d -m 0750 "$config_root" "$state_root"
  install -d -m 0700 "$backup_root" "$operations_root"
  bcsp_chown root:root "$opt_root" "$releases_root" "$unit_root" "$backup_root" "$operations_root"
  bcsp_chown "root:$BCSP_SERVICE_GROUP" "$config_root"
  bcsp_chown "$BCSP_SERVICE_USER:$BCSP_SERVICE_GROUP" "$state_root"
}

bcsp_acquire_operations_lock() {
  local lock_file

  bcsp_require_command "$BCSP_FLOCK"
  lock_file="$(bcsp_operations_root)/operations.lock"
  exec 9> "$lock_file"
  chmod 0600 "$lock_file"
  bcsp_chown root:root "$lock_file"
  "$BCSP_FLOCK" -n 9 || bcsp_die "another RBCSP operation is already running"
}

bcsp_install_active_support_files() {
  local release_path="$1" config_root

  config_root="$(bcsp_config_root)"
  [[ -f "$release_path/systemd/bcsp.service" ]] || bcsp_die "release is missing its service unit"
  [[ -f "$release_path/caddy/Caddyfile.example" ]] || bcsp_die "release is missing its Caddy example"
  [[ -f "$release_path/config/bcsp.env.example" ]] || bcsp_die "release is missing its environment example"
  [[ -f "$release_path/config/bcsp.env.schema.json" ]] || bcsp_die "release is missing its environment schema"

  install -m 0644 "$release_path/systemd/bcsp.service" "$(bcsp_unit_path)" || return 1
  install -m 0644 "$release_path/caddy/Caddyfile.example" "$config_root/Caddyfile.example" || return 1
  install -m 0644 "$release_path/config/bcsp.env.example" "$config_root/bcsp.env.example" || return 1
  install -m 0644 "$release_path/config/bcsp.env.schema.json" "$config_root/bcsp.env.schema.json" || return 1
  bcsp_chown root:root "$(bcsp_unit_path)" "$config_root/Caddyfile.example" \
    "$config_root/bcsp.env.example" "$config_root/bcsp.env.schema.json" || return 1
}

bcsp_ensure_environment_file() {
  local environment_file source_file

  environment_file="$(bcsp_environment_file)"
  if [[ ! -f "$environment_file" ]]; then
    source_file="${BCSP_ENV_SOURCE:-}"
    [[ -n "$source_file" ]] || bcsp_die \
      "create $environment_file from the example, or set BCSP_ENV_SOURCE"
    [[ -f "$source_file" ]] || bcsp_die "BCSP_ENV_SOURCE is not a regular file"
    install -m 0640 "$source_file" "$environment_file"
  fi
  chmod 0640 "$environment_file"
  bcsp_chown "root:$BCSP_SERVICE_GROUP" "$environment_file"
  bcsp_origin_authority >/dev/null
}

bcsp_origin_authority() {
  local environment_file line value="" count=0

  environment_file="$(bcsp_environment_file)"
  [[ -f "$environment_file" ]] || bcsp_die "environment file is absent: $environment_file"
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line%$'\r'}"
    case "$line" in
      BCSP_PUBLIC_ORIGIN=*)
        value="${line#BCSP_PUBLIC_ORIGIN=}"
        count=$((count + 1))
        ;;
    esac
  done < "$environment_file"
  [[ "$count" -eq 1 ]] || bcsp_die "environment must contain exactly one BCSP_PUBLIC_ORIGIN"
  if [[ "$value" != *"@"* && "$value" =~ ^https://([^/?#[:space:]]+)$ ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
    return
  fi
  bcsp_die "BCSP_PUBLIC_ORIGIN must be an HTTPS origin without credentials, path, query, or fragment"
}

bcsp_package_binary() {
  local package_root="$1"

  [[ -f "$package_root/bin/bcsp-server" ]] || bcsp_die "package is missing bin/bcsp-server"
  printf '%s\n' "$package_root/bin/bcsp-server"
}

bcsp_validate_candidate_root() {
  local package_root="$1" required actual_top_level expected_top_level forbidden

  bcsp_package_binary "$package_root" >/dev/null
  for required in share/bcsp systemd caddy config ops docs; do
    [[ -d "$package_root/$required" ]] || bcsp_die "package is missing $required/"
  done
  expected_top_level=$'bin\ncaddy\nconfig\ndocs\nops\nshare\nsystemd'
  actual_top_level="$(find "$package_root" -mindepth 1 -maxdepth 1 -printf '%f\n' | sort)"
  [[ "$actual_top_level" == "$expected_top_level" ]] || \
    bcsp_die "package top level does not match the public allowlist"
  forbidden="$(find "$package_root" -type l -print -quit)"
  [[ -z "$forbidden" ]] || bcsp_die "package contains a forbidden symbolic link: $forbidden"
  forbidden="$(find "$package_root" ! -type d ! -type f -print -quit)"
  [[ -z "$forbidden" ]] || bcsp_die "package contains a forbidden special file: $forbidden"
}

bcsp_install_release() {
  local release_id="$1" package_root="$2"
  local source_binary releases_root target stage directory

  bcsp_require_release_id "$release_id"
  bcsp_validate_candidate_root "$package_root"
  source_binary="$(bcsp_package_binary "$package_root")"
  releases_root="$(bcsp_releases_root)"
  target="$releases_root/$release_id"
  if [[ -e "$target" ]]; then
    [[ -d "$target" && -x "$target/bin/bcsp-server" ]] || \
      bcsp_die "existing release is incomplete: $target"
    cmp -s "$source_binary" "$target/bin/bcsp-server" || \
      bcsp_die "release id already exists with different binary content: $release_id"
    for directory in share systemd caddy config ops docs; do
      diff -qr -- "$package_root/$directory" "$target/$directory" >/dev/null || \
        bcsp_die "release id already exists with different support content: $release_id"
    done
    printf '%s\n' "$target"
    return
  fi

  stage="$(mktemp -d "$releases_root/.install-${release_id}.XXXXXX")"
  install -d -m 0755 "$stage/bin"
  install -m 0755 "$source_binary" "$stage/bin/bcsp-server"
  for directory in share systemd caddy config ops docs; do
    cp -R -- "$package_root/$directory" "$stage/$directory"
  done
  find "$stage" -type d -exec chmod 0755 {} +
  find "$stage" -type f -exec chmod 0644 {} +
  find "$stage/ops" -type f -name '*.sh' -exec chmod 0755 {} +
  chmod 0755 "$stage/bin/bcsp-server"
  bcsp_chown -R root:root "$stage"
  mv -- "$stage" "$target"
  printf '%s\n' "$target"
}

bcsp_current_release_path() {
  local current

  current="$(bcsp_current_link)"
  [[ -L "$current" ]] || return 1
  readlink "$current"
}

bcsp_current_release_id() {
  local release_path

  release_path="$(bcsp_current_release_path)" || return 1
  basename -- "$release_path"
}

bcsp_activate_release_path() {
  local release_path="$1"
  local releases_root current temporary_link

  releases_root="$(bcsp_releases_root)"
  [[ -d "$release_path" && -x "$release_path/bin/bcsp-server" ]] || \
    bcsp_die "release is not installable: $release_path"
  case "$release_path" in
    "$releases_root"/*) ;;
    *) bcsp_die "release is outside $releases_root" ;;
  esac
  current="$(bcsp_current_link)"
  [[ ! -e "$current" || -L "$current" ]] || bcsp_die "current path is not a symlink"
  temporary_link="${current}.new.$$"
  rm -f -- "$temporary_link" || return 1
  ln -s -- "$release_path" "$temporary_link" || return 1
  mv -Tf -- "$temporary_link" "$current" || return 1
  bcsp_install_active_support_files "$release_path" || return 1
}

bcsp_systemctl() {
  "$BCSP_SYSTEMCTL" "$@"
}

bcsp_reload_enable_service() {
  bcsp_require_command "$BCSP_SYSTEMCTL"
  bcsp_systemctl daemon-reload || return 1
  bcsp_systemctl enable "$BCSP_SERVICE_NAME" || return 1
}

bcsp_restart_service() {
  bcsp_systemctl reset-failed "$BCSP_SERVICE_NAME" >/dev/null 2>&1 || true
  bcsp_systemctl restart "$BCSP_SERVICE_NAME"
}

bcsp_wait_for_liveness() {
  local authority attempt status

  bcsp_require_command "$BCSP_CURL"
  authority="$(bcsp_origin_authority)"
  for ((attempt = 1; attempt <= BCSP_HEALTH_ATTEMPTS; attempt += 1)); do
    status="$({ "$BCSP_CURL" --silent --show-error --output /dev/null \
      --write-out '%{http_code}' --connect-timeout 2 --max-time 5 \
      --header "Host: $authority" "$BCSP_HEALTH_URL"; } 2>/dev/null || true)"
    if [[ "$status" == "200" ]]; then
      return 0
    fi
    if (( attempt < BCSP_HEALTH_ATTEMPTS )); then
      sleep "$BCSP_HEALTH_DELAY_SECONDS"
    fi
  done
  bcsp_log "liveness probe did not return HTTP 200"
  return 1
}

bcsp_sqlite_integrity() {
  local database="$1" result

  bcsp_require_command "$BCSP_SQLITE3"
  [[ -f "$database" ]] || bcsp_die "database is absent: $database"
  result="$("$BCSP_SQLITE3" "$database" 'PRAGMA integrity_check;')"
  [[ "$result" == "ok" ]] || bcsp_die "SQLite integrity check failed"
}

bcsp_sha256() {
  local file="$1" digest remainder

  bcsp_require_command "$BCSP_SHA256SUM"
  read -r digest remainder < <("$BCSP_SHA256SUM" "$file")
  [[ "$digest" =~ ^[0-9a-f]{64}$ ]] || bcsp_die "SHA-256 command returned an invalid digest"
  printf '%s\n' "$digest"
}

bcsp_backup_database() {
  local reason="${1:-manual}" database backup_root stamp backup_id stage final release_id digest manifest

  [[ "$reason" =~ ^[A-Za-z0-9._-]+$ ]] || bcsp_die "backup reason is unsafe"
  database="$(bcsp_database_path)"
  [[ -f "$database" ]] || return 0
  release_id="$(bcsp_current_release_id)" || bcsp_die "cannot back up without an active release"
  bcsp_require_release_id "$release_id"
  backup_root="$(bcsp_backup_root)"
  stamp="$(date -u +%Y%m%dT%H%M%SZ)"
  backup_id="backup-${stamp}-${reason}-$$"
  stage="$backup_root/.${backup_id}.tmp"
  final="$backup_root/$backup_id"
  manifest="$stage/manifest"
  [[ "$stage" != *"'"* ]] || bcsp_die "backup path contains an unsupported quote"
  [[ ! -e "$stage" && ! -e "$final" ]] || bcsp_die "backup destination already exists"

  install -d -m 0700 "$stage"
  "$BCSP_SQLITE3" "$database" '.timeout 10000' ".backup '$stage/rbcsp.sqlite'"
  bcsp_sqlite_integrity "$stage/rbcsp.sqlite"
  digest="$(bcsp_sha256 "$stage/rbcsp.sqlite")"
  {
    printf 'format=RBCSP_SQLITE_BACKUP_V1\n'
    printf 'database=rbcsp.sqlite\n'
    printf 'sha256=%s\n' "$digest"
    printf 'created_utc=%s\n' "$stamp"
    printf 'reason=%s\n' "$reason"
    printf 'source_release=%s\n' "$release_id"
  } > "$manifest"
  chmod 0600 "$stage/rbcsp.sqlite" "$manifest"
  bcsp_chown -R root:root "$stage"
  mv -- "$stage" "$final"
  printf '%s\n' "$final"
}

bcsp_validate_backup() {
  local backup="$1" manifest database_file line key value digest="" database="" format=""
  local created="" reason="" source_release="" seen=0 calculated entry_count

  [[ -d "$backup" && ! -L "$backup" ]] || bcsp_die "backup is not a regular backup directory"
  manifest="$backup/manifest"
  database_file="$backup/rbcsp.sqlite"
  [[ -f "$manifest" && ! -L "$manifest" && -f "$database_file" && ! -L "$database_file" ]] || \
    bcsp_die "backup payload or manifest is absent"
  entry_count="$(find "$backup" -mindepth 1 -maxdepth 1 -printf '.' | wc -c)"
  [[ "$entry_count" -eq 2 ]] || bcsp_die "backup directory contains unexpected files"
  bcsp_assert_mode 700 "$backup"
  bcsp_assert_mode 600 "$manifest"
  bcsp_assert_mode 600 "$database_file"
  bcsp_assert_owner root:root "$backup"
  bcsp_assert_owner root:root "$manifest"
  bcsp_assert_owner root:root "$database_file"

  while IFS='=' read -r key value || [[ -n "$key$value" ]]; do
    case "$key" in
      format) format="$value" ;;
      database) database="$value" ;;
      sha256) digest="$value" ;;
      created_utc) created="$value" ;;
      reason) reason="$value" ;;
      source_release) source_release="$value" ;;
      *) bcsp_die "backup manifest contains an unknown field" ;;
    esac
    seen=$((seen + 1))
  done < "$manifest"
  [[ "$seen" -eq 6 && "$format" == "RBCSP_SQLITE_BACKUP_V1" ]] || bcsp_die "backup manifest format is invalid"
  [[ "$database" == "rbcsp.sqlite" && "$digest" =~ ^[0-9a-f]{64}$ ]] || \
    bcsp_die "backup manifest payload is invalid"
  [[ "$created" =~ ^[0-9]{8}T[0-9]{6}Z$ && "$reason" =~ ^[A-Za-z0-9._-]+$ ]] || \
    bcsp_die "backup manifest metadata is invalid"
  bcsp_require_release_id "$source_release"
  calculated="$(bcsp_sha256 "$database_file")"
  [[ "$calculated" == "$digest" ]] || bcsp_die "backup SHA-256 does not match its manifest"
  bcsp_sqlite_integrity "$database_file"
  printf '%s\n' "$database_file"
}

bcsp_replace_database_file() {
  local source="$1" database state_root temporary

  bcsp_sqlite_integrity "$source" || return 1
  database="$(bcsp_database_path)"
  state_root="$(bcsp_state_root)"
  temporary="$state_root/.restore.$$"
  install -m 0640 "$source" "$temporary" || return 1
  bcsp_chown "$BCSP_SERVICE_USER:$BCSP_SERVICE_GROUP" "$temporary" || return 1
  rm -f -- "${database}-wal" "${database}-shm" "${database}-journal" || return 1
  mv -f -- "$temporary" "$database" || return 1
}

bcsp_restore_backup() {
  local backup="$1" database_file

  database_file="$(bcsp_validate_backup "$backup")" || return 1
  bcsp_replace_database_file "$database_file" || return 1
}

bcsp_write_last_upgrade() {
  local from_release="$1" to_release="$2" backup="$3" status="$4"
  local backup_id file temporary completed

  bcsp_require_release_id "$from_release"
  bcsp_require_release_id "$to_release"
  backup_id="$(basename -- "$backup")"
  [[ "$backup_id" =~ ^backup-[A-Za-z0-9._-]+$ ]] || bcsp_die "upgrade backup id is invalid"
  case "$status" in
    SUCCESS|AUTO_ROLLED_BACK|ROLLED_BACK) ;;
    *) bcsp_die "upgrade status is invalid" ;;
  esac
  completed="$(date -u +%Y%m%dT%H%M%SZ)"
  file="$(bcsp_last_upgrade_file)"
  temporary="${file}.tmp.$$"
  {
    printf 'format=RBCSP_LAST_UPGRADE_V1\n'
    printf 'from_release=%s\n' "$from_release"
    printf 'to_release=%s\n' "$to_release"
    printf 'backup_id=%s\n' "$backup_id"
    printf 'status=%s\n' "$status"
    printf 'completed_utc=%s\n' "$completed"
  } > "$temporary"
  chmod 0600 "$temporary"
  bcsp_chown root:root "$temporary"
  mv -f -- "$temporary" "$file"
}

bcsp_load_last_upgrade() {
  local file line key value seen=0

  file="$(bcsp_last_upgrade_file)"
  [[ -f "$file" && ! -L "$file" ]] || bcsp_die "last-upgrade metadata is absent"
  bcsp_assert_mode 600 "$file"
  bcsp_assert_owner root:root "$file"
  BCSP_LAST_FORMAT=""
  BCSP_LAST_FROM=""
  BCSP_LAST_TO=""
  BCSP_LAST_BACKUP_ID=""
  BCSP_LAST_STATUS=""
  BCSP_LAST_COMPLETED=""
  while IFS='=' read -r key value || [[ -n "$key$value" ]]; do
    case "$key" in
      format) BCSP_LAST_FORMAT="$value" ;;
      from_release) BCSP_LAST_FROM="$value" ;;
      to_release) BCSP_LAST_TO="$value" ;;
      backup_id) BCSP_LAST_BACKUP_ID="$value" ;;
      status) BCSP_LAST_STATUS="$value" ;;
      completed_utc) BCSP_LAST_COMPLETED="$value" ;;
      *) bcsp_die "last-upgrade metadata contains an unknown field" ;;
    esac
    seen=$((seen + 1))
  done < "$file"
  [[ "$seen" -eq 6 && "$BCSP_LAST_FORMAT" == "RBCSP_LAST_UPGRADE_V1" ]] || \
    bcsp_die "last-upgrade metadata format is invalid"
  bcsp_require_release_id "$BCSP_LAST_FROM"
  bcsp_require_release_id "$BCSP_LAST_TO"
  [[ "$BCSP_LAST_BACKUP_ID" =~ ^backup-[A-Za-z0-9._-]+$ ]] || bcsp_die "last-upgrade backup id is invalid"
  [[ "$BCSP_LAST_COMPLETED" =~ ^[0-9]{8}T[0-9]{6}Z$ ]] || bcsp_die "last-upgrade timestamp is invalid"
}

bcsp_assert_mode() {
  local expected="$1" path="$2" actual

  actual="$(stat -c '%a' "$path")"
  [[ "$actual" == "$expected" ]] || bcsp_die "$path mode is $actual, expected $expected"
}

bcsp_assert_owner() {
  local expected="$1" path="$2" actual

  if [[ "${BCSP_SKIP_OWNERSHIP:-0}" == "1" ]]; then
    return
  fi
  actual="$(stat -c '%U:%G' "$path")"
  [[ "$actual" == "$expected" ]] || bcsp_die "$path owner is $actual, expected $expected"
}
