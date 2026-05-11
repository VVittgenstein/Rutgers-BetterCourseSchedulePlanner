<!-- ngagent-doc-version: 0.8.0 -->
# Main Agent Playbook

> Operational guide for the ngagent Main Orchestrator Agent.
> Complements the root prompt (AGENTS.md/CLAUDE.md) with detailed
> decision procedures.

## Core Loop (9-bucket model)

1. `ngagent plan` → parse the 9-bucket JSON output.
2. **Merge first**: process `ready_to_merge` before anything else.
   Merges unblock downstream and prevent context-amnesia.
3. From `ready`, dispatch this cycle (6-agent cap, disjoint write scopes).
4. For `awaiting_review`, dispatch a Sub-agent to run `ngagent review`.
5. For `needs_retry_decision`, apply §3 retry framework.
6. For `needs_escalation`, investigate; escalate Tier 4 only.
7. Prepare per-task context (upstream reports → distilled `--context`).
8. Dispatch Sub-agents with `--context` / `--feedback` strings.
9. Wait (5-min check interval). Loop to step 1.

## 1. The 9 Buckets

| Bucket                 | TaskStatus         | Main Agent action                       |
|------------------------|--------------------|-----------------------------------------|
| `ready`                | PENDING, deps met  | dispatch this cycle                     |
| `blocked`              | PENDING, deps miss | wait for deps                           |
| `running`              | IN_PROGRESS or live attempt | be patient, do not poll          |
| `assigned_idle`        | ASSIGNED, no live attempt | re-dispatch or retry              |
| `awaiting_review`      | COMPLETED          | dispatch Sub-agent for `ngagent review` |
| `ready_to_merge`       | REVIEWED           | `ngagent merge <task-id>` (you)         |
| `needs_retry_decision` | FAILED             | apply §3 retry framework                |
| `needs_escalation`     | BLOCKED            | investigate; escalate if Tier 4         |
| `completed`            | MERGED             | terminal; counts toward progress        |

`ngagent plan --include-merge-order` returns the topological merge order.

## 2. Context Propagation (your core value-add)

Sub-agents and executors see only their own task prompt. You have
global visibility. Read upstream completion reports, extract what
matters, pass distilled insight downstream via `--context`.

Examples: task A added a DB column → pass name/type to task B that
consumes it; task A discovered a test flake → warn task B to use a
different test ID range; task A renamed helper `foo` to `bar` →
mention it so task B does not re-grep.

Read via `ngagent task show <task-id>` (shows the CompletionReport).
Distill to **≤3 sentences**. Pass as
`ngagent spawn <task-id> --context "..."`. Do not pass full diffs.


## 2A. Execution Provider Selection

Default dispatch remains Claude Code headless:

```bash
ngagent spawn <task-id>
```

When a task benefits from interactive supervision, tell the Sub-agent which execution layer to use:

```bash
ngagent spawn <task-id> --executor claude-code --transport tmux
ngagent spawn <task-id> --executor codex-cli --transport tmux
```

Use `ngagent execution providers` to inspect capabilities. Use `ngagent session` only through the Sub-agent when manual follow-up prompts, provider commands, or approval-state inspection are required. Provider defaults use auto-review permission modes; Codex linked-worktree git index writes use `on-request` approvals routed to `auto_review` because `.git` and resolved gitdirs remain protected under workspace-write. Review and merge gates pin session status, turn count, transcript hash, and reviewed branch head.

## 3. Retry Decision Framework

After a FAILED task, categorise and pick one action:

| Category     | Signal                                 | Action                                                            |
|--------------|----------------------------------------|-------------------------------------------------------------------|
| Mechanical   | test flake, intermittent tool error    | `task retry` + `spawn --feedback "retry, tool was flaky"`         |
| Spec         | ambiguous AC, AC conflict              | edit spec, `task retry`, `spawn --feedback "spec clarified: ..."` |
| Architecture | design assumption wrong                | escalate (Tier 4); do not retry                                   |
| Environment  | missing dep, broken venv, service down | fix environment first (Tier 3), then `task retry`                 |

**Cap: 3 dispatches per task.** After the 3rd failure, stop auto-
retrying and escalate. Sub-agents have their own 2-internal-relaunch
budget on top of this.

## 4. Priority Ordering

When multiple tasks are `ready`:

1. **Fan-out first** — dispatch tasks that unblock the most downstream
   work.
2. **Short-job first** — if fan-out is equal, pick shorter tasks to
   turn over buckets faster.
3. **Never-failed first** — prefer tasks with 0 prior attempts; retried
   tasks carry a weaker signal.

Never force parallelism onto tasks with overlapping
`allowed_write_paths`.

## 5. Deadlock Detection

If `ngagent plan` shows the **same `blocked` list for 3 consecutive
cycles** AND `running` is empty AND `awaiting_review` is empty: the
project is deadlocked. Investigate circular dependencies, stuck FAILED
tasks with no retry decision, or missing `depends_on` targets. If you
cannot resolve it, escalate.

## 6. Integration Checklist (at merge time)

Before running `ngagent merge <task-id>`:

- [ ] `ngagent task show <task-id>` shows
      `ReviewArtifact.decision == "pass"`.
- [ ] Check `CompletionReport.dependencies_added` — any new pip package?
- [ ] If new deps: verify they are in `pyproject.toml` (not just
      `pip install`ed in a venv); record in
      `.orchestrator/context_manifest.md`.
- [ ] `ngagent merge <task-id>` (auto-rebases downstream unless
      `--no-rebase-downstream`).
- [ ] Re-run `ngagent plan` — newly unblocked tasks should move to
      `ready`. If not, investigate before dispatching the next batch.
- [ ] `ngagent report` at milestone boundaries for Human summaries.

## 8. Context Governance (v0.8.0)

`ngagent` now supports three prompt-assembly modes; default is
`legacy_full` (v0.7.3.1 behavior). Opt in with
`ngagent context set-mode curated`.

- `--context` and `--feedback` are rendered **verbatim** at the top of
  the prompt in every mode. Use `--context` for facts the downstream
  agent must see word-for-word. Use `--feedback` for retry corrections.
- In `curated` mode, a Curator (Claude CLI subprocess) builds a
  `ContextPack` from the project's architecture / manifest / memory +
  dependency outputs. Every claim cites on-disk evidence; every
  required obligation is enumerated deterministically.
- Stale-context protection (post-audit simplification): merge refuses
  when the latest on-disk ContextPack no longer hashes to the value the
  reviewer stamped — re-run `ngagent review`. The pinned-text sha is
  embedded into `obligation_id` so changing `--context` text invalidates
  the cached pack automatically.
- **Durable decisions still belong in `.orchestrator/context_manifest.md`**
  until `ngagent context add` + structured context events ship in
  v0.7.3.3. The Curator reads the manifest as an optional source, so
  entries there flow into the curated prompt via evidence citations.
- `curated_strict` mode is reserved for v0.7.3.3 (Auditor not shipped);
  `ngagent context set-mode curated_strict` is rejected with an
  explicit error.

Relevant commands:

```
ngagent context build <task-id> [--purpose spawn|retry|subagent|eval] [--refresh]
ngagent context show <task-id> [--what catalog|ledger|pack|all] [--purpose …]
ngagent context status
ngagent context set-mode <legacy_full|curated|pinned_only>
ngagent spawn <task-id> --context-mode curated [--refresh-context]
ngagent prompt subagent <task-id> --context "..." --context-mode curated   # preview, no worktree required
```

### Decision Procedure — Main Agent rules

1. Pin must-see facts via `--context` / `--feedback`; rendered verbatim, never mutated.
2. Do not assume Curator will discover facts you already know. If it matters, pin it.
3. Default `legacy_full`; switch to `curated` when `ngagent doctor` reports `architecture.md` or `context_manifest.md` over budget.
4. **Never run `ngagent context build` before a task worktree exists.** Sub-agent runs `ngagent worktree create <task-id>` first, then `ngagent spawn` builds the ContextPack inside the worktree.
5. Curated build failure → fix it, or explicitly dispatch with `--context-mode legacy_full` and note the tradeoff in your report. Never silently fall back.
6. On stale-curated at merge → re-run `ngagent review`. Never force merge.
7. Record durable decisions in `context_manifest.md` until context events ship (v0.7.3.3).
8. `--context` / `--feedback` are implementation-prompt inputs. They are NOT durable review/eval/merge contracts in v0.8.0 — TaskAttempt stores only hashes (`prompt_hash`, `context_pack_hash`), not the pinned text itself. To enforce a pinned fact across review/eval/merge, additionally encode it in TaskSpec AC, tests, or `architecture.md` / `context_manifest.md`. v0.7.3.3 adds a durable pinned-inputs artifact.

### Dispatch Self-Check

Before every Sub-agent dispatch with `--context` or `--feedback`, verify the rendered mission prompt:

- The Pinned Context section contains the exact text supplied.
- The spawn command block forwards that exact text (Python form uses Jinja's `tojson` filter; shell form uses a single-quoted heredoc delimiter so `$`, backticks, and backslashes survive).
- In curated mode, the mission notes that `ngagent spawn` will build the ContextPack after `ngagent worktree create`. No `ngagent context build` call appears before worktree creation.
