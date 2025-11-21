# Mail worker latency & dedupe validation

## Scenario
- Goal: measure end-to-end delay from poller detection (open event created) to email dispatch, and confirm dedupe key prevents repeated sends.
- Method: synthetic pipeline run via `npx tsx scripts/mail_e2e_sim.ts` using a temp SQLite DB seeded with one section and three email subscriptions.
- Config: poller interval `20s` with `±20%` jitter, send stub delay `250ms`, default mail dispatcher delivery policy (`maxAttempts=3`, retry schedule `0/2/7s`), unsubscribe + manage links enabled.

## Results
- Detection → send wall-clock: **~0.8s** (includes queueing + stub sender delay).
- Derived latency (includes poll wait): best `0.8s`, average `10.8s`, worst `20.8s` — all within `<30s avg / <60s max` SLO for closed→open transitions.
- Fan-out: 1 open event → 3 pending notifications → 3 sends, meeting + course context populated in template variables.
- Dedupe: forced close then reopen within 5-minute bucket produced **0 new events / notifications**; `dedupe_key=sha1(term|campus|index|status|bucket)` prevented repeat emails.

## Reproduce / inspect
1) Run `npx tsx scripts/mail_e2e_sim.ts` (no DB or API prerequisites).
2) Inspect JSON output:
   - `detectionToSendMs` and derived `avgCaseMs`/`worstCaseMs`.
   - `openOutcome.notifications` should equal number of active subs; `reopenOutcome.notifications` should stay `0` in the dedupe check.
3) To adjust SLO inputs: change `pollIntervalMs` / `jitter` / stub delay inside the script and re-run; keep poll interval ≤20–25s to preserve `<60s` worst-case.
