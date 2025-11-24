#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

LOG_DIR="${LOG_DIR:-$ROOT_DIR/logs/run_stack}"
DB_PATH="${DB_PATH:-$ROOT_DIR/data/local.db}"
DEFAULT_CAMPUSES="${DEFAULT_CAMPUSES:-NB}"
TERMS="${TERMS:-${TERM_ID:-auto}}"
CAMPUSES="${CAMPUSES:-${CAMPUS:-}}"
TERMS_MODE="auto"
if [[ "${TERMS,,}" != "auto" ]]; then
  TERMS_MODE="explicit"
fi
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
MAIL_CONFIG_DIR="${MAIL_CONFIG_DIR:-$ROOT_DIR/configs}"
MAIL_USER_CONFIG="$MAIL_CONFIG_DIR/mail_sender.user.json"
MAIL_CONFIG_DEFAULT="$MAIL_CONFIG_DIR/mail_sender.local.json"
MAIL_CONFIG_PROVIDED=0
if [ -n "${MAIL_CONFIG+x}" ]; then
  MAIL_CONFIG_PROVIDED=1
fi
MAIL_CONFIG="${MAIL_CONFIG:-$MAIL_CONFIG_DEFAULT}"
MAIL_BATCH=25

declare -A PID_NAME=()
PID_LIST=()

usage() {
  cat <<'EOF'
Run the local stack (API + frontend + poller, with optional mail dispatcher).

Usage: scripts/run_stack.sh [options]

Options:
  --terms <auto|list>        Poller terms; default auto (discover from subscriptions)
  --term <id>                Alias for --terms <id> (pins poller to one term)
  --campuses <list>          Campus codes (comma list). Acts as allowlist in auto; default empty (auto) or NB (explicit).
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
  --with-mail                Start mail dispatcher (mail config may embed apiKey or reference env)
  --mail-config <path>       Mail config path (default: configs/mail_sender.local.json)
  --mail-batch <n>           Mail dispatcher batch size (default: 25)
  --app-base-url <url>       Base URL for links (default: http://localhost:<frontend-port>)
  --help                     Show this help

Notes:
  - If configs/mail_sender.user.json exists with dryRun=false, mail dispatching starts automatically.
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

mail_status_from_config() {
  node "$ROOT_DIR/scripts/mail_templates.js" status "$1"
}

MAIL_DECISION="none"
MAIL_DECISION_DETAIL=""

decide_mail_autostart() {
  if [ "$MAIL_CONFIG_PROVIDED" -eq 1 ]; then
    return
  fi
  if [ ! -f "$MAIL_USER_CONFIG" ]; then
    return
  fi
  local raw status detail
  raw="$(mail_status_from_config "$MAIL_USER_CONFIG")"
  status="${raw%%|*}"
  detail="${raw#*|}"
  MAIL_CONFIG="$MAIL_USER_CONFIG"
  case "$status" in
    start)
      INCLUDE_MAIL=1
      MAIL_DECISION="auto-start"
      MAIL_DECISION_DETAIL="Mail config ready at $MAIL_USER_CONFIG (dryRun=false); enabling mail dispatcher automatically."
      ;;
    dryrun)
      MAIL_DECISION="dryrun"
      MAIL_DECISION_DETAIL="Mail config at $MAIL_USER_CONFIG has dryRun=true; mail dispatcher not started. 去 WebUI 填写邮件设置并关闭 dryRun 后重启。"
      ;;
    missing-templates)
      MAIL_DECISION="missing-templates"
      MAIL_DECISION_DETAIL="Mail templates missing (${detail:-unknown path}); mail dispatcher not started. 补齐 templates/email 下的文件并保持 dryRun=true。"
      ;;
    missing-key)
      MAIL_DECISION="missing-key"
      MAIL_DECISION_DETAIL="Mail config at $MAIL_USER_CONFIG is missing a SendGrid API key${detail:+ (env ${detail} not set)}; mail dispatcher not started. 去 WebUI 填写邮件设置并关闭 dryRun 后重启。"
      ;;
    *)
      MAIL_DECISION="parse-error"
      MAIL_DECISION_DETAIL="Could not read $MAIL_USER_CONFIG: $detail"
      ;;
  esac
}

ensure_mail_ready() {
  local raw status detail
  raw="$(mail_status_from_config "$MAIL_CONFIG")"
  status="${raw%%|*}"
  detail="${raw#*|}"
  case "$status" in
    start|dryrun)
      return 0
      ;;
    missing-templates)
      echo "Missing mail templates: ${detail:-check templates/email paths}. Populate required files before starting mail dispatcher." >&2
      ;;
    missing-key)
      if [ -n "$detail" ]; then
        echo "Missing SendGrid API key env variable: $detail" >&2
      else
        echo 'Missing SendGrid API key: providers.sendgrid.apiKey or apiKeyEnv is required' >&2
      fi
      ;;
    missing)
      echo "Mail config not found at $MAIL_CONFIG" >&2
      ;;
    error)
      echo "$detail" >&2
      ;;
    *)
      echo "Unknown mail config status: $raw" >&2
      ;;
  esac
  return 3
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --term)
        if [ -z "${2-}" ]; then
          echo "Missing value for --term" >&2
          exit 1
        fi
        TERMS="$2"
        TERMS_MODE="explicit"
        shift 2
        ;;
      --terms)
        if [ -z "${2-}" ]; then
          echo "Missing value for --terms" >&2
          exit 1
        fi
        if [[ "${2,,}" == "auto" ]]; then
          TERMS="auto"
          TERMS_MODE="auto"
        else
          TERMS="$2"
          TERMS_MODE="explicit"
        fi
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
        MAIL_CONFIG_PROVIDED=1
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
  if [[ "$TERMS_MODE" == "auto" ]]; then
    local allowlist="${CAMPUSES:-<none>}"
    log "Poller terms=auto (discovering from subscriptions; campus allowlist: ${allowlist}). Missing term/campus data will log \"fetch course data\"; 先 fetch 对应 term/campus 再重试。"
  else
    log "Poller terms=${TERMS} campuses=${CAMPUSES:-$DEFAULT_CAMPUSES}"
  fi
  local poll_args=(--terms "$TERMS" --sqlite "$DB_PATH" --checkpoint "$POLL_CHECKPOINT" "${interval_arg[@]}")
  if [ -n "$CAMPUSES" ]; then
    poll_args+=(--campuses "$CAMPUSES")
  fi
  if [ "$POLLER_ONCE" -eq 1 ]; then
    poll_args+=(--once)
  fi
  start_component "open_sections_poller" bash -c "cd \"$ROOT_DIR\" && npx tsx workers/open_sections_poller.ts ${poll_args[*]}"
}

start_mail_dispatcher() {
  start_component "mail_dispatcher" bash -c "cd \"$ROOT_DIR\" && npx tsx workers/mail_dispatcher.ts --sqlite \"$DB_PATH\" --mail-config \"$MAIL_CONFIG\" --batch $MAIL_BATCH --app-base-url \"$APP_BASE_URL\""
}

parse_args "$@"

if [[ "$TERMS_MODE" == "explicit" ]]; then
  if [ -z "$TERMS" ]; then
    echo "Explicit term mode selected but no terms provided." >&2
    exit 1
  fi
  if [ -z "$CAMPUSES" ]; then
    CAMPUSES="$DEFAULT_CAMPUSES"
    log "No campuses provided; defaulting to $CAMPUSES for explicit term mode."
  fi
else
  TERMS="auto"
fi

require_cmd node
require_cmd npm

MAIL_CONFIG_DIR="$(abs_path "$MAIL_CONFIG_DIR")"
MAIL_USER_CONFIG="$MAIL_CONFIG_DIR/mail_sender.user.json"
MAIL_CONFIG_DEFAULT="$MAIL_CONFIG_DIR/mail_sender.local.json"
if [ "$MAIL_CONFIG_PROVIDED" -eq 0 ]; then
  MAIL_CONFIG="$MAIL_CONFIG_DEFAULT"
fi

decide_mail_autostart

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
  if [ ! -f "$MAIL_CONFIG" ]; then
    echo "Mail config not found at $MAIL_CONFIG" >&2
    cleanup
    exit 1
  fi
  if ! ensure_mail_ready; then
    cleanup
    exit 1
  fi
  if [ "$MAIL_DECISION" = "auto-start" ]; then
    log "$MAIL_DECISION_DETAIL"
  fi
  start_mail_dispatcher
elif [ -n "$MAIL_DECISION_DETAIL" ]; then
  log "$MAIL_DECISION_DETAIL"
else
  log "Mail dispatcher not started. 去 WebUI 填写邮件设置并关闭 dryRun 后重启。"
fi

if [ "${#PID_LIST[@]}" -eq 0 ]; then
  echo "No components started (all disabled?). Use --help for options." >&2
  exit 1
fi

log "Components running. Tailing logs under $LOG_DIR. Press Ctrl+C to stop."
monitor_children
