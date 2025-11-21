# Fresh install log — ST-20251113-act-006-03-fresh-run

Environment: WSL2 (Ubuntu), Node.js 24.11.1 (downloaded tarball to `/tmp/node-v24.11.1`), npm 11.6.2, python3 available, SQLite CLI not installed. Fresh DB path: `data/fresh_local.db`.

## Final successful run (2025-11-21)
Command: `PATH="/tmp/node-v24.11.1/bin:$PATH" ./scripts/setup_local_env.sh --db data/fresh_local.db --terms 12024 --campuses NB`
- Root npm install: ~0.4s (warm cache)
- Frontend npm install: ~0.5s
- Migrations: applied 001–003 on a fresh DB (~1s)
- `npm run data:fetch` full-init: ~26s, duplicate-course warnings emitted (now skipped), first `openSections` probe failed but retry succeeded.
- Outputs: `logs/fetch_runs/summary_latest.{log,json}` (courses inserted: 4506; sections inserted: 11467), `.env.local` files, `configs/*.local.json`, `data/fresh_local.db`.

## Issues encountered + fixes
- Missing Node / `invalid ELF header` for better-sqlite3 after switching Node versions → downloaded Node 24.11.1, removed `node_modules/ frontend/node_modules/`, reran `npm install`.
- `UNIQUE constraint failed` on courses during fetch → added a dedup/skip guard in `scripts/fetch_soc_data.ts` to ignore duplicate course numbers from `courses.json`.
- Occasional `openSections ... fetch failed` network errors → added 3x retry with backoff; rerun `npm run data:fetch` if retries still fail.
- One full-init hang while reusing an old DB → deleting `data/fresh_local.db*` and rerunning setup cleared the lock; starting from a fresh `--db` path works reliably.
