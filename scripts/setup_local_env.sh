#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

DEFAULT_DB_PATH="$ROOT_DIR/data/local.db"
DB_PATH="$DEFAULT_DB_PATH"
FETCH_CONFIG="$ROOT_DIR/configs/fetch_pipeline.local.json"
MAIL_CONFIG="$ROOT_DIR/configs/mail_sender.local.json"
TERMS="12024"
CAMPUSES="NB"
SUBJECTS=""
FETCH_MODE="full-init"
MAX_WORKERS=""
SKIP_FETCH=0
SKIP_FRONTEND_INSTALL=0
DB_OVERRIDDEN=0

usage() {
  cat <<'EOF'
Bootstrap local dev environment (deps + migrations + initial data fetch).

Usage: scripts/setup_local_env.sh [options]

Options:
  --db <path>             SQLite path for migrations/fetch (default: data/local.db)
  --terms <list>          Terms to fetch (comma-separated, default: 12024)
  --campuses <list>       Campuses to fetch (comma-separated, default: NB)
  --subjects <list>       Optional subject filter (comma-separated)
  --mode <full-init|incremental>
                          Fetch mode override (default: full-init)
  --fetch-config <path>   Path to fetch pipeline config (default: configs/fetch_pipeline.local.json)
  --max-workers <n>       Max workers override passed to fetcher
  --skip-fetch            Install + migrate only, do not pull data
  --skip-frontend-install Skip npm install inside frontend/
  --help                  Show this help
EOF
}

log() {
  echo "[setup] $*"
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

write_default_fetch_config() {
  local target="$1"
  local db="$2"
  local default_term="$3"
  local default_campus="$4"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOF
{
  "\$schema": "./fetch_pipeline.schema.json",
  "runLabel": "local-setup",
  "defaultMode": "incremental",
  "sqliteFile": "$db",
  "stagingDir": "data/staging",
  "logDir": "logs/fetch_runs",
  "rateLimitProfile": "docs/soc_rate_limit.latest.json",
  "concurrency": {
    "maxCourseWorkers": 3,
    "courseRequestIntervalMs": 600,
    "maxOpenSectionsWorkers": 10,
    "openSectionsIntervalMs": 250,
    "maxParallelCampuses": 2,
    "maxSubjectsPerCampus": 4
  },
  "retryPolicy": {
    "maxAttempts": 4,
    "backoffMs": [0, 3000, 7000, 15000],
    "jitter": 0.3,
    "downgradedProfile": {
      "maxCourseWorkers": 1,
      "courseRequestIntervalMs": 1200,
      "maxOpenSectionsWorkers": 5,
      "openSectionsIntervalMs": 500
    },
    "retryableStatus": [408, 429, 500, 502, 503, 504]
  },
  "targets": [
    {
      "term": "$default_term",
      "mode": "full-init",
      "campuses": [{ "code": "$default_campus", "subjects": ["ALL"] }]
    }
  ],
  "incremental": {
    "resumeQueueFile": "data/refresh_queue.json",
    "subjectRecencyMinutes": 60,
    "deferNightlyStartLocalTime": "02:00",
    "maxSubjectRetries": 3
  },
  "fullInit": {
    "prerunMigrations": true,
    "truncateTables": [
      "courses",
      "sections",
      "section_meetings",
      "section_instructors",
      "section_populations",
      "section_crosslistings"
    ],
    "rebuildFts": true
  },
  "summary": {
    "writeJson": "logs/fetch_runs/summary_latest.json",
    "writeText": "logs/fetch_runs/summary_latest.log",
    "emitMetrics": true
  },
  "safety": {
    "dryRun": false,
    "requireCleanWorktree": false
  }
}
EOF
  log "Wrote default fetch config to $target (sqliteFile=$db)"
}

copy_if_missing() {
  local target="$1"
  local template="$2"
  if [ -f "$target" ]; then
    return
  fi
  if [ ! -f "$template" ]; then
    echo "Template not found: $template" >&2
    exit 1
  fi
  cp "$template" "$target"
  log "Copied $(basename "$template") -> $target (please edit secrets/IDs)"
}

create_env_helpers() {
  local api_env="$ROOT_DIR/.env.local"
  if [ ! -f "$api_env" ]; then
    cat >"$api_env" <<EOF
APP_PORT=3333
APP_HOST=127.0.0.1
SQLITE_FILE=$DB_PATH
LOG_LEVEL=info
EOF
    log "Created $api_env"
  fi

  local fe_env="$ROOT_DIR/frontend/.env.local"
  if [ ! -f "$fe_env" ]; then
    cat >"$fe_env" <<'EOF'
VITE_API_PROXY_TARGET=http://localhost:3333
VITE_API_BASE_URL=/api
EOF
    log "Created $fe_env"
  fi
}

parse_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --db)
        DB_PATH="$2"
        DB_OVERRIDDEN=1
        shift 2
        ;;
      --terms)
        TERMS="$2"
        shift 2
        ;;
      --campuses)
        CAMPUSES="$2"
        shift 2
        ;;
      --subjects)
        SUBJECTS="$2"
        shift 2
        ;;
      --mode)
        FETCH_MODE="$2"
        shift 2
        ;;
      --fetch-config)
        FETCH_CONFIG="$2"
        shift 2
        ;;
      --max-workers)
        MAX_WORKERS="$2"
        shift 2
        ;;
      --skip-fetch)
        SKIP_FETCH=1
        shift
        ;;
      --skip-frontend-install)
        SKIP_FRONTEND_INSTALL=1
        shift
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

parse_args "$@"

if [[ "$FETCH_MODE" != "full-init" && "$FETCH_MODE" != "incremental" ]]; then
  echo "Invalid --mode: $FETCH_MODE" >&2
  exit 1
fi

require_cmd node
require_cmd npm

FETCH_CONFIG="$(abs_path "$FETCH_CONFIG")"
MAIL_CONFIG="$(abs_path "$MAIL_CONFIG")"

if [ -f "$FETCH_CONFIG" ]; then
  config_db="$(node -e "const fs=require('fs');const path=require('path');const p=process.argv[1];try{const data=JSON.parse(fs.readFileSync(p,'utf8'));if(data.sqliteFile){console.log(path.resolve(path.dirname(p), data.sqliteFile));}}catch(e){process.exit(0);} " "$FETCH_CONFIG")"
  if [ -n "$config_db" ] && [ "$DB_OVERRIDDEN" -eq 0 ]; then
    DB_PATH="$config_db"
    log "Using sqliteFile from $FETCH_CONFIG -> $DB_PATH"
  fi
else
  DB_PATH="$(abs_path "$DB_PATH")"
  primary_term="${TERMS%%,*}"
  primary_campus="${CAMPUSES%%,*}"
  write_default_fetch_config "$FETCH_CONFIG" "$DB_PATH" "$primary_term" "$primary_campus"
fi

DB_PATH="$(abs_path "$DB_PATH")"

if [ -f "$FETCH_CONFIG" ] && [ "$DB_OVERRIDDEN" -eq 1 ]; then
  current_db="$(node -e "const fs=require('fs');const path=require('path');const p=process.argv[1];try{const data=JSON.parse(fs.readFileSync(p,'utf8'));if(data.sqliteFile){console.log(path.resolve(path.dirname(p), data.sqliteFile));}}catch(e){process.exit(0);} " "$FETCH_CONFIG")"
  if [ -n "$current_db" ] && [ "$current_db" != "$DB_PATH" ]; then
    log "Syncing sqliteFile in $(basename "$FETCH_CONFIG") to $DB_PATH"
    node -e "const fs=require('fs');const path=require('path');const p=process.argv[1];const db=process.argv[2];const data=JSON.parse(fs.readFileSync(p,'utf8'));data.sqliteFile=db;fs.writeFileSync(p, JSON.stringify(data, null, 2));" "$FETCH_CONFIG" "$DB_PATH"
  fi
fi

copy_if_missing "$MAIL_CONFIG" "$ROOT_DIR/configs/mail_sender.example.json"
create_env_helpers

log "Installing root dependencies..."
(cd "$ROOT_DIR" && npm install)

if [ "$SKIP_FRONTEND_INSTALL" -eq 0 ]; then
  log "Installing frontend dependencies..."
  (cd "$ROOT_DIR/frontend" && npm install)
else
  log "Skipping frontend npm install (requested)"
fi

mkdir -p "$ROOT_DIR/data" "$ROOT_DIR/logs/fetch_runs" "$ROOT_DIR/data/staging"

log "Running migrations on $DB_PATH ..."
(cd "$ROOT_DIR" && npm run db:migrate -- --db "$DB_PATH" --verbose)

if [ "$SKIP_FETCH" -eq 1 ]; then
  log "Skipping data fetch (requested)"
  log "Setup complete."
  exit 0
fi

FETCH_ARGS=(--config "$FETCH_CONFIG" --mode "$FETCH_MODE" --terms "$TERMS" --campuses "$CAMPUSES")
if [ -n "$SUBJECTS" ]; then
  FETCH_ARGS+=(--subjects "$SUBJECTS")
fi
if [ -n "$MAX_WORKERS" ]; then
  FETCH_ARGS+=(--max-workers "$MAX_WORKERS")
fi

log "Starting data fetch (mode=$FETCH_MODE terms=$TERMS campuses=$CAMPUSES)..."
(cd "$ROOT_DIR" && npm run data:fetch -- "${FETCH_ARGS[@]}")

log "Setup complete."
