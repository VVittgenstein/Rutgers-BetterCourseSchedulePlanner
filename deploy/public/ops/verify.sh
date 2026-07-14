#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

main() {
  local opt_root releases_root current config_root state_root operations_root backup_root environment_file unit_path
  local release_path database support_file passwd_entry passwd_home passwd_shell

  [[ "$#" -eq 0 ]] || bcsp_die "usage: verify.sh"
  bcsp_require_privilege
  opt_root="$(bcsp_opt_root)"
  releases_root="$(bcsp_releases_root)"
  current="$(bcsp_current_link)"
  config_root="$(bcsp_config_root)"
  state_root="$(bcsp_state_root)"
  operations_root="$(bcsp_operations_root)"
  backup_root="$(bcsp_backup_root)"
  environment_file="$(bcsp_environment_file)"
  unit_path="$(bcsp_unit_path)"
  database="$(bcsp_database_path)"

  [[ -d "$opt_root" && -d "$releases_root" && -d "$config_root" ]] || \
    bcsp_die "required installation directories are absent"
  [[ -d "$state_root" && -d "$operations_root" && -d "$backup_root" ]] || \
    bcsp_die "state, operations, or backup directory is absent"
  [[ -L "$current" ]] || bcsp_die "current release symlink is absent"
  release_path="$(bcsp_current_release_path)"
  case "$release_path" in
    "$releases_root"/*) ;;
    *) bcsp_die "current release points outside the releases directory" ;;
  esac
  [[ -x "$release_path/bin/bcsp-server" ]] || bcsp_die "current bcsp-server is not executable"
  [[ -f "$unit_path" && -f "$environment_file" ]] || bcsp_die "service configuration is incomplete"
  for support_file in share systemd caddy config ops docs; do
    [[ -d "$release_path/$support_file" ]] || bcsp_die "current release is missing $support_file/"
    bcsp_assert_mode 755 "$release_path/$support_file"
    bcsp_assert_owner root:root "$release_path/$support_file"
  done
  [[ -d "$release_path/share/bcsp" ]] || bcsp_die "current release is missing share/bcsp/"

  bcsp_assert_mode 755 "$opt_root"
  bcsp_assert_mode 755 "$releases_root"
  bcsp_assert_mode 755 "$release_path"
  bcsp_assert_mode 755 "$release_path/bin"
  bcsp_assert_mode 755 "$release_path/bin/bcsp-server"
  bcsp_assert_mode 750 "$config_root"
  bcsp_assert_mode 750 "$state_root"
  bcsp_assert_mode 700 "$operations_root"
  bcsp_assert_mode 700 "$backup_root"
  bcsp_assert_mode 640 "$environment_file"
  bcsp_assert_mode 644 "$unit_path"
  bcsp_assert_owner root:root "$opt_root"
  bcsp_assert_owner root:root "$releases_root"
  bcsp_assert_owner root:root "$release_path"
  bcsp_assert_owner root:root "$release_path/bin/bcsp-server"
  bcsp_assert_owner "root:$BCSP_SERVICE_GROUP" "$config_root"
  bcsp_assert_owner "root:$BCSP_SERVICE_GROUP" "$environment_file"
  bcsp_assert_owner "$BCSP_SERVICE_USER:$BCSP_SERVICE_GROUP" "$state_root"
  bcsp_assert_owner root:root "$operations_root"
  bcsp_assert_owner root:root "$backup_root"
  bcsp_assert_owner root:root "$unit_path"
  if [[ "${BCSP_SKIP_USER_MANAGEMENT:-0}" != "1" ]]; then
    bcsp_require_command getent
    passwd_entry="$(getent passwd "$BCSP_SERVICE_USER")" || bcsp_die "service user is absent"
    IFS=: read -r _ _ _ _ _ passwd_home passwd_shell <<< "$passwd_entry"
    [[ "$passwd_home" == "/nonexistent" ]] || bcsp_die "service user has an unexpected home directory"
    [[ "$passwd_shell" == "/usr/sbin/nologin" ]] || bcsp_die "service user has an unexpected login shell"
  fi
  for support_file in Caddyfile.example bcsp.env.example bcsp.env.schema.json; do
    bcsp_assert_mode 644 "$config_root/$support_file"
    bcsp_assert_owner root:root "$config_root/$support_file"
  done
  if [[ -f "$(bcsp_last_upgrade_file)" ]]; then
    bcsp_assert_mode 600 "$(bcsp_last_upgrade_file)"
    bcsp_assert_owner root:root "$(bcsp_last_upgrade_file)"
  fi
  bcsp_origin_authority >/dev/null

  bcsp_require_command "$BCSP_SYSTEMCTL"
  bcsp_systemctl is-active --quiet "$BCSP_SERVICE_NAME" || bcsp_die "service is not active"
  bcsp_wait_for_liveness || bcsp_die "service is not live"

  if [[ -f "$database" ]]; then
    bcsp_assert_mode 640 "$database"
    bcsp_assert_owner "$BCSP_SERVICE_USER:$BCSP_SERVICE_GROUP" "$database"
    bcsp_sqlite_integrity "$database"
  fi
  bcsp_log "installation, service, liveness, and SQLite checks passed"
}

main "$@"
