<!-- ngagent-doc-version: 0.8.0 -->
# Eval and Handoff Subsystems

## Evaluation

Model-based evaluator gated behind `EvalConfig.enabled=True` (default
**disabled**).

- `ngagent eval <task-id>` — standalone; writes `EvaluationReport` to
  `.git/ngagent/tasks/<task-id>/eval-NNN.json`.
- `ngagent review <task-id>` — full pipeline; runs eval after
  tests/lint/typecheck when enabled. `--skip-eval` bypasses.

Requires `ANTHROPIC_API_KEY`, `EvalConfig.enabled=True`, and
`uv pip install ngagent[eval]`.

**Hard gate:** when enabled and `EvaluationError` is raised (missing
key, network, rate limit), review **fails closed** — status `blocked`,
decision `blocked`. In CI without API access: disable eval or always
pass `--skip-eval`.

## Handoff

`ngagent handoff write/show/list` persists session context. Since
v0.6.2, `spawn` **auto-loads** the latest handoff into the executor
prompt.

- `handoff write <task-id> [--note "..."]` — capture state.
- `handoff show <task-id>` — print latest handoff.
- `handoff list <task-id>` — list all handoffs.
- `spawn <task-id> --no-handoff` — clean re-spawn ignoring prior
  session context.

Use case: Sub-agent was mid-task when interrupted; on resume, `spawn`
auto-injects the last handoff so executor resumes from that state.

## JSON Output Contract

All ngagent stdout is JSON except `ngagent report` (markdown). Errors
go to stderr; exit code non-zero on failure.

**`ngagent plan`** — 9 mutually-exclusive buckets plus execution session metadata:

```json
{
  "ready": ["task-004"],
  "blocked": [{"task_id": "task-005", "missing": ["task-004"]}],
  "running": ["task-003"],
  "assigned_idle": [],
  "awaiting_review": ["task-002"],
  "ready_to_merge": [],
  "needs_retry_decision": ["task-006"],
  "needs_escalation": [],
  "completed": ["task-001"],
  "execution_sessions": {}
}
```

**`ngagent status`** (zero args) — `{project, project_status,
task_counts, total_tasks, merged_tasks, reviewed_unmerged, running_sessions, stale_sessions, warnings,
milestone_summary, blockers, next_actions}`.

**`ngagent task show <task-id>`** — full Task record with latest
TaskSpec, CompletionReport, ReviewArtifact, attempt history. Use this
to read upstream results for context propagation.

**`ngagent merge <task-id>`** — `{task_id, status:
"merged|conflict|error", commit_sha, conflicting_files, rebase_results,
error}`. Non-`merged` exits code 1.

**`ngagent report`** — markdown, not JSON. Milestone summaries.

Schemas grounded in `ngagent.models`: `TaskSpec`, `CompletionReport`,
`ReviewArtifact`, `MergeArtifact`, `EvaluationReport`.

## Context Mode Resolution (v0.8.0)

`ngagent review --eval` resolves its context mode from the **active
attempt**'s `context_mode` first; only if that is empty does it fall
back to `record.config.context.mode`. This means a per-invocation
`--context-mode curated` on spawn propagates to eval automatically.

When mode is `curated` / `curated_strict`, Evaluator uses a
`purpose=eval` ContextPack (built on demand) instead of raw
architecture / context_manifest injection.
