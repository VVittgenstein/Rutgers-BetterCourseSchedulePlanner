# One-click web UI launch (Windows/macOS)

This launcher is meant for people with no coding background: double-click a file to open the course planner locally.

## Prerequisites
- Install Node.js 22+ (includes npm). No other tools are required.
- Keep the repository folder intact; the launcher assumes the files stay together.

## Start the app
1) Windows: double-click `Start-WebUI.bat`. macOS: double-click `Start-WebUI.command` (you may need to allow it to run the first time).
2) Leave the window open. On first run it will install dependencies, migrate/create the SQLite file, and run a full data fetch for the default term/campus configured in `configs/fetch_pipeline.local.json` (seeds data for the UI/poller).
3) Your browser opens to `http://localhost:5174`. Use the web UI to subscribe; the poller runs in auto mode and discovers your term/campus from active subscriptions. If it logs `fetch course data for this term/campus`, run a fetch for that combo before expecting notifications.
4) To stop, press Ctrl+C in the launcher window or close it.

## Pick a different term/campus for the data
- Defaults come from `configs/fetch_pipeline.local.json` (first entry in `targets[]`). Campus codes are the Rutgers SOC short codes (e.g., `NB`). The poller stays in auto mode unless you override it.
- Quick override without editing JSON: set environment variables before launching (`CSP_TERM=12026` and `CSP_CAMPUSES=NB` would target term 12026 for NB and pin the poller; `CSP_TERMS=12024,12026` sets an explicit list).
- If you change term/campus, force a fresh download once by deleting the existing SQLite file (default `data/fresh_local.db`) or launching with `CSP_FORCE_FETCH=1`.

## Optional toggles
- Skip the openSections poller: `CSP_SKIP_POLLER=1`.
- Keep poller in a fixed term/campus: use `CSP_TERMS=<list>` (or `CSP_TERM` + `CSP_CAMPUSES`) instead of the default auto-discovery.
- Ports: `CSP_API_PORT` (default `3333`) and `CSP_FRONTEND_PORT` (default `5174`).
- Custom DB path: `CSP_DB_PATH` (or `CSP_SQLITE_FILE`).
