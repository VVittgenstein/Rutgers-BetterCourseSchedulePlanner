# BetterCourseSchedulePlanner
A modern course filtering and sniping tool for Rutgers University SOC data.

## Quick start
1) Install Node.js 22+ (tested with 24.11.1) + npm. Python 3 is handy for sanity checks.
2) Bootstrap everything (creates `.env.local`, copies config examples, installs deps, migrates, runs a full-init fetch):
   ```bash
   ./scripts/setup_local_env.sh --db data/local.db --terms 12024 --campuses NB
   ```
   Fetch summary: `logs/fetch_runs/summary_latest.log`. If a previous DB causes a hang, delete `data/local.db*` or point `--db` to a fresh path.
3) Start the stack:
   ```bash
   ./scripts/run_stack.sh --term 12024 --campuses NB \
     [--with-mail --mail-config configs/mail_sender.local.json]
   ```
   Logs go to `logs/run_stack/`; Ctrl+C stops all children. The API binds to `127.0.0.1` by default for local-only use.

## One-click launch (Windows/macOS)
- Windows: double-click `Start-WebUI.bat`. macOS: double-click `Start-WebUI.command`.
- The launcher installs deps if missing, migrates/creates the SQLite DB, runs a fetch for the default term/campus, starts API + web UI, and opens `http://localhost:5174`.
- Change term/campus or ports via the env vars described in `docs/oneclick.md`.

## Useful scripts
- Database migrations (SQLite in `data/migrations`):
  ```bash
  npm run db:migrate                     # default data/local.db
  npm run db:migrate -- --db /tmp/csp.db # custom path
  ```
  Flags: `--migrations <dir>`, `--log-file <path>`, `--dry-run`, `--verbose`.
- Fetch data manually (if you want to retry without the setup wrapper):
  ```bash
  npm run data:fetch -- --config configs/fetch_pipeline.local.json --mode full-init --terms 12024 --campuses NB
  ```
- Start pieces individually: `npm run api:start` (API) and `npm run dev` inside `frontend/` (UI) if you prefer separate terminals.

## Docs
- docs/quickstart.md (step-by-step local bring-up + troubleshooting)
- docs/oneclick.md (no-code one-click launcher guide for Windows/macOS)
- docs/deployment_playbook.md (local checklist, validation, and notification flows)
