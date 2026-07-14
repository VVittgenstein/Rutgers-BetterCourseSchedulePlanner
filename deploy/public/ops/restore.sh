#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

main() {
  local backup="${1:-}" safety_backup

  [[ "$#" -eq 1 ]] || bcsp_die "usage: restore.sh BACKUP_DIRECTORY"
  backup="$(cd -- "$(dirname -- "$backup")" && pwd)/$(basename -- "$backup")"
  [[ -d "$backup" ]] || bcsp_die "backup is not a backup directory"
  bcsp_require_privilege
  bcsp_ensure_directories
  bcsp_acquire_operations_lock
  [[ -L "$(bcsp_current_link)" ]] || bcsp_die "no active release"
  bcsp_validate_backup "$backup" >/dev/null

  bcsp_systemctl stop "$BCSP_SERVICE_NAME"
  safety_backup="$(bcsp_backup_database pre-restore)"
  if [[ -z "$safety_backup" ]]; then
    bcsp_die "current database is absent"
  fi
  bcsp_restore_backup "$backup"
  if ! bcsp_systemctl start "$BCSP_SERVICE_NAME" || ! bcsp_wait_for_liveness; then
    bcsp_log "restored database did not become live; reverting to pre-restore backup"
    bcsp_systemctl stop "$BCSP_SERVICE_NAME" || true
    bcsp_restore_backup "$safety_backup"
    bcsp_systemctl start "$BCSP_SERVICE_NAME" && bcsp_wait_for_liveness || \
      bcsp_die "pre-restore database could not be brought live"
    bcsp_die "restore was reverted"
  fi
  bcsp_log "database restored; pre-restore backup retained at $safety_backup"
}

main "$@"
