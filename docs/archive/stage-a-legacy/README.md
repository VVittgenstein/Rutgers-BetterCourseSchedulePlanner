# Stage A Legacy Archive

> **Purpose.** This directory holds historical AI-workflow evidence that
> Stage A's final baseline classified as evidence-only, not authority.
> Nothing here is the canonical source of truth for current product
> behavior or current planning. The canonical sources are listed in
> `.orchestrator/stage-a/06-final-baseline.md` §4 (source-of-truth
> hierarchy) and `.orchestrator/stage-a/07-cleanup-application.md`
> (the cleanup that produced this archive).

## What is archived here

This archive was created by `task-007` (Stage A apply approved
non-product cleanup and repository organization). The cleanup moved the
listed items from their original repository-root locations into this
directory using `git mv`, preserving file content and (where applicable)
directory structure. The original on-disk content is not deleted; it is
relocated.

| Original path | Archive path | Kind | Reason for archival |
| --- | --- | --- | --- |
| `record.json` | `docs/archive/stage-a-legacy/record.json` | Legacy planning JSON | Pre-ngagent task graph; not ngagent runtime state. Status fields are stale or contradict current code. (`.orchestrator/stage-a/03-record-reconciliation.md` §6.6, §8.2.) |
| `rEmail.json` | `docs/archive/stage-a-legacy/rEmail.json` | Legacy planning JSON | Closest legacy record to ground truth (mail onboarding). Retained as historical evidence; not authoritative. (`03-record-reconciliation.md` §5.1.) |
| `rRevision.json` | `docs/archive/stage-a-legacy/rRevision.json` | Legacy planning JSON | Filter rewrite seed plan; statuses stale. (`03-record-reconciliation.md` §5.2.) |
| `rSubscribe.json` | `docs/archive/stage-a-legacy/rSubscribe.json` | Legacy planning JSON | Auto-term poller seed plan; parent status stale. (`03-record-reconciliation.md` §5.3.) |
| `Compact/` (74 files) | `docs/archive/stage-a-legacy/Compact/` | Historical narrative | Per-subtask narratives + Code-Review trailers authored by an earlier AI workflow. (`03-record-reconciliation.md` §3, §4.11.) |
| `review/` (6 files) | `docs/archive/stage-a-legacy/review/` | Historical narrative | `diff --git a/record.json b/record.json` snapshots from 2025-11-13..17. (`03-record-reconciliation.md` §3, §6.4.) |
| `Rutgers-dr/` (2 files) | `docs/archive/stage-a-legacy/Rutgers-dr/` | Hand-authored DR | Includes `2025-11-11-dr.md` and `Patch-dr-2025-11-14.md`. The patch DR is the immutable narrative that explains the static-frontend → local-DB pivot. (`03-record-reconciliation.md` §3, §6.1.) |
| `read_only.md` | `docs/archive/stage-a-legacy/read_only.md` | DR distillation | The `5b-mb.md` output, v0.1, 2025-11-13. Decisions D1/D3/ACT-001/-002/-003/DEP-004 were superseded by `Patch-dr-2025-11-14.md` and `record.json.decisions`. The root `read_only.md` was replaced with a short forwarding pointer. (`03-record-reconciliation.md` §6.2, §7.) |

## How to use this archive

1. **For Stage B planning:** treat the contents of this directory as Tier
   T2 historical evidence per `.orchestrator/stage-a/06-final-baseline.md`
   §4. Useful for reconstructing what an earlier agent said it did and
   when. Not useful as a description of current code state or current
   task status.
2. **For migrations into ngagent runtime state:** legacy planning JSON
   (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`)
   may be consulted as *spec input* when authoring a new ngagent task,
   but no `status` field from these files may be imported into
   `.git/ngagent/` as if it described current state.
3. **For forensic comparison:** Compact files frequently carry
   Code-Review trailers and are dated, which makes them more reliable
   than `record.json` parents for "what an agent claimed it did." Some
   Compacts describe code that is not present on `dev` today — see
   `.orchestrator/stage-a/03-record-reconciliation.md` §4.11 for the
   `act-004-discord-notify-channel` contradiction.

## What was deliberately not changed

Per `task-007` acceptance criteria and per
`.orchestrator/stage-a/06-final-baseline.md` §7.5:

- No content inside these files was edited during the archive move.
- No file was deleted from disk.
- Status fields in `record.json` and the `r*.json` files were not
  "corrected"; correcting them would destroy the evidence layer the
  Stage A baseline depends on. The reconciled status set lives in
  ngagent runtime state at `.git/ngagent/`, not in this archive.
- Release artifacts (`release/bcsp-20260121.tar.gz`,
  `release/bcsp-20260121.zip`, `bcsp-20260122.zip`) were not moved into
  this archive; release-pack disposition was explicitly deferred per
  `task-007` acceptance criteria.
- The external Obsidian vault under
  `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\` is
  out of scope for any in-repo change.

## See also

- `.orchestrator/stage-a/01-inventory.md` — original inventory that
  surfaced these items.
- `.orchestrator/stage-a/03-record-reconciliation.md` — full record
  reconciliation report.
- `.orchestrator/stage-a/06-final-baseline.md` §3.3, §5.3, §7.3 — the
  Stage A baseline classification and archive recommendation.
- `.orchestrator/stage-a/07-cleanup-application.md` — the cleanup
  application that produced this archive.
- `read_only.md` (repository root) — short forwarding pointer left in
  place so external references continue to resolve.
