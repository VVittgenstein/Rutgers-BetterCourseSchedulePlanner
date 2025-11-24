# Deployment playbook

_Last updated: 2025-11-22_

Bring a clean laptop/VM from git clone to a running stack (SQLite + data fetch + API + React UI + optional email notifications). Defaults assume localhost, SQLite at `data/local.db`, API on `:3333`, and Vite dev server on `:5174`.

## Automation shortcuts
- Bootstrap deps + DB + initial fetch:  
  `./scripts/setup_local_env.sh --terms 12024 --campuses NB`  
  Creates `configs/*.local.json` if missing, runs `npm install` (root + frontend), applies migrations to `data/local.db`, then triggers a full-init fetch (use `--skip-fetch` to defer).
- Start local stack with logs under `logs/run_stack/`:  
  `./scripts/run_stack.sh [--terms 12024 --campuses NB]`  
  Starts API + frontend + openSections poller by default; poller uses `--terms auto` (discover from subscriptions) unless you pin terms. Add `--with-mail` (requires `SENDGRID_API_KEY` + mail config) if you want email alerts.

## Pre-flight checklist
| Item | Notes |
| --- | --- |
| Node.js | v22.x recommended. Use the same installation for root + `frontend/`. |
| npm | Comes with Node. If using Windows, run commands in Git Bash/WSL. |
| SQLite CLI | Optional but useful for quick sanity checks (`sqlite3 data/local.db ...`). |
| Python 3 | Optional for validation helpers in `docs/data_load_runbook.md`. |
| Network | Outbound HTTPS to `classes.rutgers.edu` (data fetch/poller) and SendGrid if sending email notifications. |
| Credentials | `SENDGRID_API_KEY` (or SMTP) for mail if you want outbound email. |

## Config to prepare (one time)
- Copy examples and adjust paths/IDs:
  - `cp configs/fetch_pipeline.example.json configs/fetch_pipeline.local.json` → set `sqliteFile` to `data/local.db` and edit `targets` for the term/campus you want.
  - `cp configs/mail_sender.example.json configs/mail_sender.local.json` → customize `defaultFrom`, templates, and turn off `testHooks.dryRun` when ready. Keep provider keys in env vars (`SENDGRID_API_KEY` or `SMTP_PASSWORD`).
- Optional `.env` helpers (not committed):
  ```bash
  cat > .env.local <<'EOF'
  APP_PORT=3333
  APP_HOST=127.0.0.1
  SQLITE_FILE=$(pwd)/data/local.db
  LOG_LEVEL=info
  EOF
  
  cd frontend
  cat > .env.local <<'EOF'
  VITE_API_PROXY_TARGET=http://localhost:3333
  VITE_API_BASE_URL=/api
  EOF
  ```

## Step-by-step bring-up
1) **Install dependencies**
   ```bash
   npm install                 # root (API, workers, scripts)
   cd frontend && npm install  # UI package
   ```
   Expected: node_modules installed without errors.

2) **Migrate the database**
   ```bash
   npm run db:migrate -- --db data/local.db --verbose
   ```
   Expected: `data/local.db` created, `data/migrations.log` appended, no checksum mismatch.

3) **Fetch course data (full init)**
   ```bash
   npm run data:fetch -- --config configs/fetch_pipeline.local.json --mode full-init --terms 12024 --campuses NB
   ```
   Expected: `logs/fetch_runs/summary_latest.{log,json}` created; `data/staging/` may appear; `courses/sections` counts >0 in SQLite.

4) **Start core services (use separate terminals)**
   - API server:
     ```bash
     APP_PORT=3333 SQLITE_FILE=data/local.db npm run api:start
     ```
     Expected log: `Listening on http://127.0.0.1:3333`.
   - Frontend (runs Vite with proxy to API):
     ```bash
     cd frontend
     VITE_API_PROXY_TARGET=http://localhost:3333 npm run dev -- --host 127.0.0.1 --port 5174
     ```
     Expected output: Vite ready message and UI at http://localhost:5174.
   - openSections poller (queues open-seat events):
     ```bash
     tsx workers/open_sections_poller.ts \
       --terms auto \
       --interval 20 --sqlite data/local.db \
       --checkpoint data/poller_checkpoint.json \
       --metrics-port 9309
     ```
     Pin to one term with `--terms 12024 --campuses NB` if you don't want auto discovery. Expected: lines like `[NB] openSections=XX opened=Y closed=Z events=E notifications=N ...`; missing datasets log `fetch course data` without stopping other targets.
   - Mail dispatcher:
     ```bash
     SENDGRID_API_KEY=... tsx workers/mail_dispatcher.ts \
       --sqlite data/local.db \
       --mail-config configs/mail_sender.local.json \
       --batch 25 --app-base-url http://localhost:5174 --idle-delay 2000
     ```
     Expected: logs showing claimed batches and `status=sent` attempts; `open_event_notifications` rows should drain.

## Verification flows
### A) Data fetch
- `sqlite3 data/local.db "SELECT COUNT(*) FROM courses;"` → should be >0.
- `cat logs/fetch_runs/summary_latest.log | tail -n 5` → ends with `Finished <term>/<campus>` and `errors=[]`.

### B) API + UI queries
- Health/readiness:
  ```bash
  curl http://localhost:3333/api/ready
  ```
  Expected: JSON `{ "status": "ready", ... }`.
- Course search spot-check:
  ```bash
  curl "http://localhost:3333/api/courses?term=12024&campus=NB&limit=3"
  ```
  Expected: payload with `data` array and `meta.total` >0.
- UI: open http://localhost:5174, pick a term/campus, confirm course list populates (no “fallback” warning).

### C) Subscribe + receive mail
1. Pick a test section index:
   ```bash
   sqlite3 data/local.db "SELECT term_id,campus_code,index_number,is_open FROM sections LIMIT 5;"
   ```
2. Create a subscription (email example):
   ```bash
  curl -X POST http://localhost:3333/api/subscribe \
    -H "Content-Type: application/json" \
    -d '{"term":"12024","campus":"NB","sectionIndex":"12345","contactType":"email","contactValue":"you@example.com","locale":"en-US"}'
  ```
3. Force an open event for smoke testing (avoids waiting for a live flip):
  ```bash
  sqlite3 data/local.db "UPDATE sections SET is_open=0, open_status='CLOSED' WHERE index_number='12345';"
  tsx workers/open_sections_poller.ts --terms 12024 --campuses NB --once --sqlite data/local.db --checkpoint data/poller_checkpoint.json
  sqlite3 data/local.db "SELECT fanout_status,COUNT(*) FROM open_event_notifications GROUP BY 1;"
  ```
  Expected: a `pending` count ≥1 for that index.
4. Run the mail dispatcher until the `pending` count drops to 0. Check your inbox for the message; logs should show `status=sent`.
5. Re-run `npm run data:fetch -- --config configs/fetch_pipeline.local.json --mode incremental` to restore the true section status after the forced flip.

If you want a dry-run without live sends, use `npx tsx scripts/mail_e2e_sim.ts` (no provider traffic).

## Troubleshooting
| Symptom | Likely cause | Fix |
| --- | --- | --- |
| `openSections failed` / HTTP 429 in poller or fetcher | SOC throttling | Lower concurrency in fetch config, increase `--interval`/`--max-workers 1`, retry after a pause (see `docs/soc_rate_limit.md`). |
| API returns 503/not ready | SQLite missing or schema incomplete | Re-run `npm run db:migrate -- --db data/local.db`; ensure `SQLITE_FILE` points to the same DB the fetcher wrote. |
| UI shows fallback dictionary / empty lists | API unreachable or proxy mis-set | Set `VITE_API_PROXY_TARGET` to the API URL; verify `curl http://localhost:3333/api/filters` works before reloading UI. |
| No notifications queued | Poller not seeing opens or subscriptions inactive | Ensure subscriptions exist (`SELECT COUNT(*) FROM subscriptions`), drop `--once` so poller keeps watching, or run the forced-open procedure above; delete stale `data/poller_checkpoint.json` if it refers to another DB. |
| Mail dispatcher loops or skips | Missing provider key or still in dry-run | Export `SENDGRID_API_KEY` (or SMTP password), set `testHooks.dryRun=false` in mail config, and inspect `open_event_notifications.error` for details. |
