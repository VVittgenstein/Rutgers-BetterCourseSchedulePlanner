#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

main() {
  local release_id="${1:-}" package_root="${2:-$(cd -- "$SCRIPT_DIR/.." && pwd)}"
  local previous previous_id release_path backup

  [[ "$#" -le 2 ]] || bcsp_die "usage: upgrade.sh RELEASE_ID [PACKAGE_ROOT]"
  bcsp_require_release_id "$release_id"
  package_root="$(cd -- "$package_root" && pwd)"
  bcsp_require_privilege
  previous="$(bcsp_current_release_path)" || bcsp_die "no active release; use install.sh"
  previous_id="$(basename -- "$previous")"
  bcsp_ensure_service_user
  bcsp_ensure_directories
  bcsp_acquire_operations_lock
  bcsp_ensure_environment_file
  release_path="$(bcsp_install_release "$release_id" "$package_root")"

  if [[ "$previous" == "$release_path" ]]; then
    # H3: a same-release upgrade is a liveness check, nothing more. The
    # release content was just proven byte-identical, so there is nothing
    # to reload and nothing to restart -- and a restart here would clear
    # every live watch (watches are not persisted) for zero benefit. The
    # service keeps its MainPID; only the health probe runs.
    bcsp_wait_for_liveness || bcsp_die "active release is not live"
    bcsp_log "release $release_id is already active and live; nothing to restart"
    return
  fi

  [[ -f "$(bcsp_database_path)" ]] || bcsp_die "active service database is absent"
  backup="$(bcsp_backup_database pre-upgrade)"
  if bcsp_activate_release_path "$release_path" && \
    bcsp_reload_enable_service && \
    bcsp_restart_service && \
    bcsp_wait_for_liveness; then
    bcsp_write_last_upgrade "$previous_id" "$release_id" "$backup" SUCCESS
    bcsp_log "release $release_id is live; paired database backup: $backup"
    return
  else
    bcsp_log "release $release_id failed liveness; restoring the previous release and database"
    bcsp_systemctl stop "$BCSP_SERVICE_NAME" || true
    bcsp_activate_release_path "$previous"
    bcsp_restore_backup "$backup"
    bcsp_reload_enable_service
    bcsp_restart_service
    bcsp_wait_for_liveness || bcsp_die "automatic rollback did not become live"
    bcsp_write_last_upgrade "$previous_id" "$release_id" "$backup" AUTO_ROLLED_BACK
    bcsp_die "upgrade failed and the previous binary/database pair was restored"
  fi
}

main "$@"
