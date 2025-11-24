#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

LOG_DIR="${LOG_DIR:-$ROOT_DIR/logs/run_stack}"
DB_PATH="${DB_PATH:-$ROOT_DIR/data/local.db}"
TERM_ID="12024"
CAMPUSES="NB"
POLL_INTERVAL=20
POLL_INTERVAL_MS=""
POLL_CHECKPOINT="$ROOT_DIR/data/poller_checkpoint.json"
START_API=1
START_FRONTEND=1
START_POLLER=1
INCLUDE_MAIL=0
POLLER_ONCE=0
API_PORT="${API_PORT:-3333}"
FRONTEND_PORT="${FRONTEND_PORT:-5174}"
API_ORIGIN=""
APP_BASE_URL=""
MAIL_CONFIG="${MAIL_CONFIG:-$ROOT_DIR/configs/mail_sender.local.json}"
MAIL_BATCH=25

declare -A PID_NAME=()
PID_LIST=()

usage() {
  cat <<'EOF'
Run the local stack (API + frontend + poller, with optional mail dispatcher).

Usage: scripts/run_stack.sh [options]

Options:
  --term <id>                Term for poller (default: 12024)
  --campuses <list>          Campus codes for poller (comma list, default: NB)
  --db <path>                SQLite path (default: data/local.db)
  --api-port <port>          API port (default: 3333)
  --api-origin <url>         API origin for frontend proxy (default: http://localhost:<api-port>)
  --frontend-port <port>     Frontend dev server port (default: 5174)
  --poll-interval <sec>      Poller interval seconds (default: 20)
  --poll-interval-ms <ms>    Poller interval milliseconds (overrides seconds)
  --checkpoint <path>        Poller checkpoint file (default: data/poller_checkpoint.json)
  --no-api                   Do not start API server
  --no-frontend              Do not start frontend
  --no-poller                Do not start openSections poller
  --poller-once              Run one poll per campus then exit
  --with-mail                Start mail dispatcher (requires SENDGRID_API_KEY and mail config)
  --mail-config <path>       Mail config path (default: configs/mail_sender.local.json)
  --mail-batch <n>           Mail dispatcher batch size (default: 25)
  --app-base-url <url>       Base URL for links (default: http://localhost:<frontend-port>)
  --help                     Show this help
EOF
}

log() {
  echo "[run] $*"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

abs_path() {
  node -e "const path = require('path'); console.log(path.resolve(process.argv[1]));" "$1"
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --term)
        TERM_ID="$2"
        shift 2
        ;;
      --campuses)
        CAMPUSES="$2"
        shift 2
        ;;
      --db)
        DB_PATH="$2"
        shift 2
        ;;
      --api-port)
        API_PORT="$2"
        shift 2
        ;;
      --api-origin)
        API_ORIGIN="$2"
        shift 2
        ;;
      --frontend-port)
        FRONTEND_PORT="$2"
        shift 2
        ;;
      --poll-interval)
        POLL_INTERVAL="$2"
        shift 2
        ;;
      --poll-interval-ms)
        POLL_INTERVAL_MS="$2"
        shift 2
        ;;
      --checkpoint)
        POLL_CHECKPOINT="$2"
        shift 2
        ;;
      --no-api)
        START_API=0
        shift
        ;;
      --no-frontend)
        START_FRONTEND=0
        shift
        ;;
      --no-poller)
        START_POLLER=0
        shift
        ;;
      --poller-once)
        POLLER_ONCE=1
        shift
        ;;
      --with-mail)
        INCLUDE_MAIL=1
        shift
        ;;
      --mail-config)
        MAIL_CONFIG="$2"
        shift 2
        ;;
      --mail-batch)
        MAIL_BATCH="$2"
        shift 2
        ;;
      --app-base-url)
        APP_BASE_URL="$2"
        shift 2
        ;;
      --help)
        usage
        exit 0
        ;;
      *)
        echo "Unknown option: $1" >&2
        usage
        exit 1
        ;;
    esac
  done
}

start_component() {
  local name="$1"
  local logfile="$LOG_DIR/$name.log"
  shift
  mkdir -p "$LOG_DIR"
  log "Starting $name (log: $logfile)"
  ("$@") >>"$logfile" 2>&1 &
  local pid=$!
  PID_NAME["$pid"]="$name"
  PID_LIST+=("$pid")
}

cleanup() {
  set +e
  for pid in "${PID_LIST[@]}"; do
    if kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
    fi
  done
  set -e
}

monitor_children() {
  while :; do
    for pid in "${PID_LIST[@]}"; do
      if ! kill -0 "$pid" >/dev/null 2>&1; then
        set +e
        wait "$pid"
        status=$?
        set -e
        local name="${PID_NAME[$pid]:-child}"
        log "$name exited with status $status"
        cleanup
        exit "$status"
      fi
    done
    sleep 2
  done
}

start_api() {
  start_component "api" bash -c "cd \"$ROOT_DIR\" && APP_PORT=$API_PORT APP_HOST=\${APP_HOST:-127.0.0.1} SQLITE_FILE=\"$DB_PATH\" npm run api:start"
}

start_frontend() {
  start_component "frontend" bash -c "cd \"$ROOT_DIR/frontend\" && VITE_API_PROXY_TARGET=$API_ORIGIN VITE_API_BASE_URL=/api npm run dev -- --host 127.0.0.1 --port $FRONTEND_PORT"
}

start_poller() {
  local interval_arg=(--interval "$POLL_INTERVAL")
  if [ -n "$POLL_INTERVAL_MS" ]; then
    interval_arg=(--interval-ms "$POLL_INTERVAL_MS")
  fi
  local poll_args=(--term "$TERM_ID" --campuses "$CAMPUSES" --sqlite "$DB_PATH" --checkpoint "$POLL_CHECKPOINT" "${interval_arg[@]}")
  if [ "$POLLER_ONCE" -eq 1 ]; then
    poll_args+=(--once)
  fi
  start_component "open_sections_poller" bash -c "cd \"$ROOT_DIR\" && npx tsx workers/open_sections_poller.ts ${poll_args[*]}"
}

start_mail_dispatcher() {
  start_component "mail_dispatcher" bash -c "cd \"$ROOT_DIR\" && npx tsx workers/mail_dispatcher.ts --sqlite \"$DB_PATH\" --mail-config \"$MAIL_CONFIG\" --batch $MAIL_BATCH --app-base-url \"$APP_BASE_URL\""
}

parse_args "$@"

require_cmd node
require_cmd npm

DB_PATH="$(abs_path "$DB_PATH")"
MAIL_CONFIG="$(abs_path "$MAIL_CONFIG")"
POLL_CHECKPOINT="$(abs_path "$POLL_CHECKPOINT")"

if [ -z "$API_ORIGIN" ]; then
  API_ORIGIN="http://localhost:$API_PORT"
fi

if [ -z "$APP_BASE_URL" ]; then
  APP_BASE_URL="http://localhost:$FRONTEND_PORT"
fi

if [ "$START_POLLER" -eq 1 ] || [ "$INCLUDE_MAIL" -eq 1 ] || [ "$START_API" -eq 1 ]; then
  if [ ! -f "$DB_PATH" ]; then
    echo "SQLite database not found at $DB_PATH. Run scripts/setup_local_env.sh first." >&2
    exit 1
  fi
fi

trap 'log "Stopping components..."; cleanup' EXIT
trap 'log "Interrupted"; cleanup; exit 1' INT TERM

if [ "$START_API" -eq 1 ]; then
  start_api
fi

if [ "$START_FRONTEND" -eq 1 ]; then
  start_frontend
fi

if [ "$START_POLLER" -eq 1 ]; then
  start_poller
fi

if [ "$INCLUDE_MAIL" -eq 1 ]; then
  if [ -z "${SENDGRID_API_KEY:-}" ]; then
    echo "SENDGRID_API_KEY is required to start mail dispatcher." >&2
    cleanup
    exit 1
  fi
  if [ ! -f "$MAIL_CONFIG" ]; then
    echo "Mail config not found at $MAIL_CONFIG" >&2
    cleanup
    exit 1
  fi
  start_mail_dispatcher
fi

if [ "${#PID_LIST[@]}" -eq 0 ]; then
  echo "No components started (all disabled?). Use --help for options." >&2
  exit 1
fi

log "Components running. Tailing logs under $LOG_DIR. Press Ctrl+C to stop."
monitor_children
