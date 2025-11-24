# Quickstart (local dev)

Tested on WSL2 (Ubuntu) with Node.js 24.11.1 + npm 11.6.x. Python 3 is available for sanity checks; SQLite CLI is optional.

## Prerequisites
- Node.js 22+ (match one version for root + `frontend/`). If you switch Node majors, delete `node_modules/ frontend/node_modules/` and reinstall to rebuild native deps (better-sqlite3).
- Git + bash; outbound HTTPS to `classes.rutgers.edu` and any notification providers you plan to use.
- Optional: `python3` for quick DB sanity checks, SendGrid credentials if you want email alerts.

## Bootstrap everything
```bash
./scripts/setup_local_env.sh --db data/local.db --terms 12024 --campuses NB
```
What happens: copies `configs/*.example.json` to `*.local.json`, writes `.env.local` files (API + frontend), runs `npm install` in root and `frontend/`, applies migrations, then runs a full-init fetch. Expected runtime on a warm cache: deps <1 minute, migrations ~2s, fetch for 12024/NB ~30s. Logs land in `logs/fetch_runs/summary_latest.{log,json}`.

Notes:
- If a previous DB makes full-init hang, remove `data/local.db*` (or point `--db` to a fresh path) and rerun.
- Network flakes on `openSections` are retried automatically (3 attempts). Rerun the fetch if retries still fail.
- Duplicate-course warnings from `courses.json` are handled and can be ignored.

## Run the stack
```bash
./scripts/run_stack.sh --term 12024 --campuses NB \
  [--with-mail --mail-config configs/mail_sender.local.json]
```
Defaults start API + frontend + openSections poller. The API binds to `127.0.0.1`; change `APP_HOST` only if you explicitly want remote access. Logs stream to `logs/run_stack/`. Use `--no-frontend`, `--no-poller`, or `--poller-once` to slim down the process list.

## Verify quickly
- Check the fetch summary: `cat logs/fetch_runs/summary_latest.log` (expect inserts >0).
- Spot-check counts:
  ```bash
  python3 - <<'PY'
  import sqlite3; conn=sqlite3.connect('data/local.db')
  cur=conn.cursor()
  for t in ['courses','sections']: cur.execute(f'SELECT COUNT(*) FROM {t}'); print(t, cur.fetchone()[0])
  conn.close()
  PY
  ```
- API ready probe: `curl http://localhost:3333/api/ready`.
- UI: browse to `http://localhost:5174` (proxying to API on :3333).

## Troubleshooting
- `Missing required command: node` or `invalid ELF header` for better-sqlite3 → install Node 22+ and rerun `npm install` after deleting `node_modules/ frontend/node_modules/`.
- Full-init stuck on deletes → wipe `data/local.db*` or switch to `--db data/fresh_<date>.db`, then rerun setup.
- Persistent `openSections` failures → retry `npm run data:fetch -- --config configs/fetch_pipeline.local.json --mode full-init --terms 12024 --campuses NB` after a short pause; network issues will surface with request IDs.
