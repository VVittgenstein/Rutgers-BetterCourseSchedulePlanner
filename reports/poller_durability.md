# Poller durability and restart behavior

## Scenario
- Goal: confirm long-running openSections polling survives restarts without duplicate events or missed closures.
- Method: synthetic 2-hour timeline (120 one-minute ticks) driven by `npx tsx scripts/poller_resume_sim.ts` against a temp SQLite DB. Restart injected right after a missed heartbeat so closure logic depends on checkpoint restore. Default checkpoint path: `scripts/poller_checkpoint.json` (override with `CHECKPOINT_FILE`).

## Results
- Event flow: open at t=30m, immediate close at t=81m **after restart** (miss counter restored from checkpoint), reopen at t=100m. No duplicate open events across the restart.
- Totals from the run: opened=2, closed=1, events=3, notifications=2 (one per open edge). Final checkpoint recorded last snapshot hash `a21bd...` with zero pending miss counters.
- Checkpoint hydration log confirms restoration: `[NB] restored checkpoint at ... (hash=..., misses=1)` prior to the restart phase, proving the pending miss was carried over.

## Checkpoint design
- After every successful poll the worker writes `{term, campus, lastPollAt, lastSnapshotHash, openIndexes, misses}` to `scripts/poller_checkpoint.json` (path configurable via `--checkpoint`).
- On startup the worker reloads matching entries (same term/campus), seeds `missCounters`, and primes metrics' `lastOpenCount`. Corrupt/missing files fall back to a clean slate with a warning.
- The `misses` map persists in-flight close debounces, so a process restart between heartbeats still triggers the close on the next poll instead of waiting an extra cycle.

## Runbook
- Reproduce the durability simulation: `npx tsx scripts/poller_resume_sim.ts` (set `CHECKPOINT_FILE` to keep production checkpoints untouched).
- Operate the real poller with checkpoints: `tsx workers/open_sections_poller.ts --term 12024 --campuses NB,NK --checkpoint data/poller_checkpoint.json ...`. On startup expect a log like `[NB] restored checkpoint ...`.
- Observe health: `/metrics` exposes `poller_*` counters/gauges; `open_events` plus `open_event_notifications` should show one close flanked by open edges in the scenario above.
- Recovery: if the checkpoint file is lost, the poller simply rebuilds state; worst case is one extra debounce interval before marking a section closed.
