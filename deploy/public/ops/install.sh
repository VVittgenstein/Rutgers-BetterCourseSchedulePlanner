#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$SCRIPT_DIR/lib.sh"

main() {
  local release_id="${1:-}" package_root="${2:-$(cd -- "$SCRIPT_DIR/.." && pwd)}"
  local release_path previous=""

  [[ "$#" -le 2 ]] || bcsp_die "usage: install.sh RELEASE_ID [PACKAGE_ROOT]"
  bcsp_require_release_id "$release_id"
  package_root="$(cd -- "$package_root" && pwd)"
  bcsp_require_privilege
  bcsp_ensure_service_user
  bcsp_ensure_directories
  bcsp_acquire_operations_lock
  release_path="$(bcsp_install_release "$release_id" "$package_root")"

  if previous="$(bcsp_current_release_path)"; then
    if [[ "$previous" != "$release_path" ]]; then
      bcsp_die "a different release is active; use upgrade.sh"
    fi
  fi

  bcsp_activate_release_path "$release_path"
  bcsp_ensure_environment_file
  bcsp_reload_enable_service
  if ! bcsp_restart_service || ! bcsp_wait_for_liveness; then
    bcsp_systemctl stop "$BCSP_SERVICE_NAME" || true
    if [[ -z "$previous" ]]; then
      bcsp_systemctl disable "$BCSP_SERVICE_NAME" || true
      rm -f -- "$(bcsp_current_link)"
    fi
    bcsp_die "installation did not become live"
  fi
  bcsp_log "release $release_id is installed and live"
}

main "$@"
