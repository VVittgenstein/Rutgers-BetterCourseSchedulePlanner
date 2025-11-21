Subtask: ST-20251113-act-003-03-end-to-end-validation

Facts (implemented & validated)
- Added synthetic E2E simulator `scripts/mail_e2e_sim.ts` that seeds SQLite, runs poller applySnapshot + mail dispatcher with a timed stub sender, outputs JSON metrics covering detection→send latency and dedupe behavior.
- New report `reports/mail_worker_latency.md` documents sim setup, results (~0.8s detection→send with 20s poll interval implied avg 10.8s / worst 20.8s within <30s/<60s SLO) and reproducibility steps.
- New runbook `docs/notify_runbook.md` captures mail channel SLOs, recommended poller/dispatcher flags (interval ≤20–25s, lock TTL 120s), monitoring queries (metrics + SQLite checks), and troubleshooting steps for stuck locks, provider errors, duplicates.
- `record.json` marks the subtask status as done and registers new artifacts above.

Interfaces / behavior impact
- Simulation script provides a deterministic harness for queue→mail path and dedupe check; no runtime API changes.
- Runbook specifies operational expectations (poll cadence, retry budgets, dedupe key) for other operators/modules.

Tests
- `npx tsx scripts/mail_e2e_sim.ts` (passes; outputs latency/dedupe JSON).

Risks / TODO
- Latency claims rely on stub SendGrid (~250ms) and 20s poll interval; real provider RTTs and different poll cadences should be rechecked with live credentials.
