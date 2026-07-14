#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

main() {
  local backup

  [[ "$#" -eq 0 ]] || bcsp_die "usage: backup.sh"
  bcsp_require_privilege
  bcsp_ensure_directories
  bcsp_acquire_operations_lock
  [[ -f "$(bcsp_database_path)" ]] || bcsp_die "database does not exist yet"
  backup="$(bcsp_backup_database manual)"
  bcsp_log "consistent SQLite backup created: $backup"
  printf '%s\n' "$backup"
}

main "$@"
