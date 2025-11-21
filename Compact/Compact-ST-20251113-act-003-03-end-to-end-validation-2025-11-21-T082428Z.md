Subtask: ST-20251113-act-003-03-end-to-end-validation

Facts (implemented & updated)
- `docs/notify_runbook.md` poller command corrected: `--interval` is seconds (`--interval 20` in example), notes `--interval-ms` for ms override and that jitter uses the built-in default (0.3). Aligns instructions with actual CLI parsing and <60s latency SLO.
- Prior artifacts remain: synthetic E2E simulator `scripts/mail_e2e_sim.ts`, report `reports/mail_worker_latency.md` with ~0.8s detection→send stub timing and dedupe validation, and runbook guidance for SLOs/ops.
- `record.json` already marks the subtask done with these artifacts.

Interfaces / behavior impact
- Documentation only: corrected poller invocation to match CLI flags; no runtime code changes.

Tests
- Not re-run (docs-only edit).

Risks / TODO
- Real provider latency still needs validation with live credentials to confirm SLOs beyond stub timings.

## Code Review - ST-20251113-act-003-03-end-to-end-validation - 2025-11-21T08:26:44Z

---review-start---
docs/notify_runbook.md
Comment on lines +12 to +15


## How to run locally
- **Poller (openSections):**
  - `tsx workers/open_sections_poller.ts --term 12025 --campuses NB --interval 20000 --jitter 0.2 --checkpoint data/poller_checkpoint.json --metrics-port 9309`
  - Interval ≤20s keeps worst-case mail latency under 60s; jitter avoids thundering herd.


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Correct poller CLI flags and units

The runbook’s poller command uses --interval 20000 --jitter 0.2, but workers/open_sections_poller.ts parses --interval as seconds (multiplies by 1000) and has no --jitter flag at all (switch at lines 200–299), so running this command either errors out or configures a 20,000‑second interval—directly contradicting the stated 20s/60s latency SLO and leaving notifications idle for hours.
---review-end---
