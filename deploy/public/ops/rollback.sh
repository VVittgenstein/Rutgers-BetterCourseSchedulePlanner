#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

main() {
  local release_id="${1:-}" current current_id target upgrade_backup recovery_backup

  [[ "$#" -eq 1 ]] || bcsp_die "usage: rollback.sh RELEASE_ID"
  bcsp_require_release_id "$release_id"
  bcsp_require_privilege
  bcsp_ensure_directories
  bcsp_acquire_operations_lock
  current="$(bcsp_current_release_path)" || bcsp_die "no active release"
  current_id="$(basename -- "$current")"
  bcsp_load_last_upgrade
  [[ "$BCSP_LAST_STATUS" == "SUCCESS" ]] || bcsp_die "last upgrade is not eligible for paired rollback"
  [[ "$BCSP_LAST_FROM" == "$release_id" && "$BCSP_LAST_TO" == "$current_id" ]] || \
    bcsp_die "requested release is not the binary/database pair from the last successful upgrade"
  target="$(bcsp_releases_root)/$release_id"
  [[ -d "$target" && -x "$target/bin/bcsp-server" ]] || bcsp_die "release is not installed: $release_id"
  upgrade_backup="$(bcsp_backup_root)/$BCSP_LAST_BACKUP_ID"
  bcsp_validate_backup "$upgrade_backup" >/dev/null
  recovery_backup="$(bcsp_backup_database pre-rollback)"

  bcsp_systemctl stop "$BCSP_SERVICE_NAME"
  if bcsp_activate_release_path "$target" && \
    bcsp_restore_backup "$upgrade_backup" && \
    bcsp_reload_enable_service && \
    bcsp_restart_service && \
    bcsp_wait_for_liveness; then
    bcsp_write_last_upgrade "$release_id" "$current_id" "$upgrade_backup" ROLLED_BACK
    bcsp_log "release $release_id and its paired pre-upgrade database are active"
    return
  else
    bcsp_log "rollback pair failed liveness; restoring the newer binary/database pair"
    bcsp_systemctl stop "$BCSP_SERVICE_NAME" || true
    bcsp_activate_release_path "$current"
    bcsp_restore_backup "$recovery_backup"
    bcsp_reload_enable_service
    bcsp_restart_service
    bcsp_wait_for_liveness || bcsp_die "newer binary/database pair could not be restored"
    bcsp_die "rollback failed and the newer binary/database pair was restored"
  fi
}

main "$@"
