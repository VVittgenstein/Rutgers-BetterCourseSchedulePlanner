<!-- ngagent-doc-version: 0.8.0 -->
# Sub-agent Runbook

> Lifecycle, retry protocol, and error recovery for ngagent Sub-agents.
> The actual mission prompt a Sub-agent receives is rendered by:
>
>     ngagent prompt subagent <task-id> [--context "..."] [--feedback "..."]
>
> This document is the *reference* explaining what the Sub-agent does
> and why. Main Agent reads it once, then relies on the CLI renderer
> for every dispatch.

## 1. Execution Steps

A Sub-agent manages one task end-to-end: worktree → spawn → review →
inspect → report. Run these shell commands imperatively — do not
describe, do not delegate.

```bash
# Zero — verify tool availability
ngagent --help
# If not found: uv pip install -e . (from project root)

# Step 1 — create the isolated worktree
ngagent worktree create task-001

# Step 2 — launch the configured executor (BLOCKS 10-30+ minutes, NORMAL)
ngagent spawn task-001 \
    --expected-duration 1800 \
    [--context "..."] [--feedback "..."] \
    [--model claude-opus-4-7] [--effort xhigh]
# --effort = low<medium<high<xhigh<max (Opus 4.7, 5 tiers ascending; default: xhigh)
# --context/--feedback are OPTIONAL on first attempt
# DO NOT add timeouts. DO NOT kill this process.

# Step 3 — check the task record
ngagent task show task-001

# Step 4 — run validation (tests + lint + typecheck + optional eval)
ngagent review task-001       # --skip-eval to bypass model-based eval

# Step 5 — inspect delivered artifacts
git -C <worktree_path> log --oneline
git -C <worktree_path> diff dev..HEAD --stat
ngagent events task-001

# Step 6 — file structured report to Main Agent (see §8)
```


Execution Plane v2 also allows explicit execution selection:

```bash
ngagent spawn task-001 --executor claude-code --transport headless
ngagent spawn task-001 --executor claude-code --transport tmux
ngagent spawn task-001 --executor codex-cli --transport tmux
```

For interactive supervision, use `ngagent session start|send|command|completion-request|approval-status|capture|wait|finalize|stop`. Do not drive raw tmux sessions directly. `session start` sends the initial task prompt through the provider-safe path: Claude uses the supervised composer path, and Codex uses the interactive CLI PROMPT argument while still recording an initial execution turn.

Canonical supervised tmux lifecycle:

```bash
ngagent session start task-001 --executor claude-code --transport tmux
ngagent session send task-001 --text "<follow-up prompt when needed>"
ngagent session completion-request task-001
ngagent session wait task-001 --until completion-report
# If wait stalls or returns no report, inspect approval state and escalate when needed.
ngagent session approval-status task-001
ngagent session finalize task-001
ngagent review task-001
```

`session wait --until completion-report` binds the parsed CompletionReport to the active execution session and updates the attempt state. Provider permission defaults use auto review: Claude starts in auto mode, and Codex uses workspace-write plus on-request approvals routed to auto_review. If an approval prompt remains visible, run `session approval-status`; denied, timed-out, stopped, or unresolved requests must be raised to Main Agent. The `completion-request` prompt deliberately describes the sentinel protocol without embedding a parseable report example, so echoed user input cannot be mistaken for executor completion. `session finalize` writes the final transcript hash and closes the review-ready provenance gate. Finalize without `--keep-session` before review; a finalized but still-live tmux session is rejected by the review gate.

## 2. What Happens Inside `ngagent spawn`

`ngagent spawn <task-id>` builds the mission prompt from the latest
TaskSpec (acceptance criteria with AC-NNN IDs, interface contracts,
relevant files, allowed_write_paths), pre-freezes an `attempt-{N}.json`
with provenance hashes (base_sha, prompt_hash, architecture_hash,
context_manifest_hash), then launches executor as a subprocess:

    ngagent spawn <task-id> --executor claude-code --transport headless

It blocks until the executor exits, then finalizes the attempt with
`finished_at`, `status`, and `completion_ref`.

What the executor does internally is not your concern:
- It MAY spawn lightweight **sub-agents** via the Task tool to parallelise
  independent work inside its session.
- It MAY form **Agent Teams** of independent executor instances that
  share a task list and direct-message each other.
- It runs a **validation-driven validation loop** (test → fix → retest) up to
  10 cycles before emitting a structured `failure_diagnosis`.

You do not pass deprecated provider flags. Use `ngagent spawn` or `ngagent session` so provider flags are constructed by ngagent.

On timeout or no-parseable-report with no commits, the spawner
transitions the task to FAILED — never leaves it stuck IN_PROGRESS.

## 3. Collaborative Retry Protocol

the executor has a 10-cycle internal retry cap. When hit, it emits a
`failure_diagnosis` recording the approaches it tried, the errors that
kept recurring, a root-cause hypothesis, and suggested next steps. Your
job is to analyse and re-launch with corrected guidance.

`failure_diagnosis` JSON shape (persisted inside CompletionReport,
must match the `FailureDiagnosis` Pydantic model):

```json
{
  "root_cause_hypothesis": "string",
  "approaches_tried": ["string"],
  "persistent_errors": ["string"],
  "suggested_next_steps": ["string"]
}
```

Decision tree on receiving a FAILED/BLOCKED result:

1. **You can adjust the approach** → `ngagent task retry task-001`
   (transitions back to IN_PROGRESS), then
   `ngagent spawn task-001 --feedback "..." [--context "..."]`.
   The feedback and latest `review-*.json` findings are auto-injected
   into the next prompt. Use `--force` ONLY if a stale attempt is still
   recorded as running.
2. **Issue is beyond your scope** (spec conflict, architectural choice,
   external blocker) → escalate to Main Agent with your analysis and
   executor's `failure_diagnosis`.

**Budget:** re-launch the configured executor at most **2 times** after the first
attempt (3 launches × 10 cycles ≈ 30 validation cycles total). After
the 3rd launch, escalate unconditionally.

## 4. Error Recovery

**No-parseable-completion-report fallback:** if `ngagent spawn` exits
with status `error` and message "no parseable completion report":

1. `git -C <worktree_path> log --oneline` — if commits exist, work was
   done but the report protocol failed. The spawner's finalize step
   builds a degraded CompletionReport from commit metadata automatically.
   Inspect the diff and report what you see.
2. If there are no commits, the executor produced nothing. Check whether
   it was blocked (missing tool, ambiguous spec, environment). Report
   diagnostics to Main Agent.

**Do not crash on missing reports.** Always produce *some* report back
to the Main Agent, even if the only content is "executor exited
with no commits, see events.jsonl".

## 5. Sub-agent Hard Constraints (7 MUST NOT rules)

- **MUST NOT** skip the review step. `ngagent merge` gates on a
  ReviewArtifact with `decision="pass"`; skipping guarantees merge
  failure.
- **MUST NOT** manually call `git merge`, `git rebase`, or
  `git cherry-pick`. Merging is Main Agent's job via `ngagent merge`.
- **MUST NOT** delete the worktree before reporting back. Main Agent
  may need it for inspection; deletion is handled during merge.
- **MUST NOT** edit files outside `allowed_write_paths` declared in
  the TaskSpec. Write-scope violations fail the review policy check.
- **MUST NOT** modify `.orchestrator/*.md` planning docs
  (goals.md, architecture.md, context_manifest.md). Those are Main
  Agent's architect-layer files.
- **MUST NOT** pass deprecated provider flags directly. Let ngagent construct provider launch commands.
- **MUST NOT** suppress or mask provider exit codes. Non-zero exit
  is a real signal and must propagate into the CompletionReport.

## 6. Validation Protocol

The validation loop inside executor is validation-driven, not step-counted:

```
while (not acceptance_criteria_all_met):
    run_tests()
    if failing: fix_the_failing_thing()
    if cycle_count >= 10: emit failure_diagnosis and stop
```

At the Sub-agent layer, run `ngagent review` once after spawn completes.
Review runs mechanical checks (tests, lint, typecheck), then the
model-based evaluator when `EvalConfig.enabled=True`.

**Escalation triggers** — stop retrying and escalate when:

- **>3 failed fix cycles on the same root cause** across re-launches.
- **Test-suite flakiness** not attributable to the task (unrelated
  import errors, network-dependent tests).
- **Environment problems** surface (missing binary, broken venv,
  external API down). These flow up via the Environment Escalation
  Protocol — do not `pip install` your way around them.

## 7. Code Review Checklist

After `ngagent review` reports pass (or changes_requested), run these
inspection steps before filing your report:

- [ ] `git -C <worktree_path> log --oneline` — commits atomic, messages
  coherent, no "WIP" or "debug" left behind.
- [ ] `git -C <worktree_path> diff dev..HEAD --stat` — diff size
  plausible for the task; flag anything unexpected.
- [ ] `git -C <worktree_path> diff dev..HEAD -- pyproject.toml` — any
  new declared dependencies? If yes, they belong in
  `CompletionReport.dependencies_added`.
- [ ] `grep -R "TODO\|FIXME\|XXX" <changed-files>` — no leftover markers.
- [ ] Import sort / obvious dead code spot-check on the changed files.
- [ ] Write-scope: every changed path is under `allowed_write_paths`.
- [ ] `ngagent events task-001` — state transitions are complete and
  in order (pending → assigned → in_progress → completed → reviewed).

Anything flagged here goes into the `notes` field of your Sub-agent
report so the Main Agent has the evidence.

## 8. Report Format

File a structured JSON report back to the Main Agent. The top-level
shape mirrors `CompletionReport` with Sub-agent observations added:

```json
{
  "task_id": "task-001",
  "worktree_path": "/abs/path/.worktrees/task-001",
  "commit_sha": "abc1234",
  "review_decision": "pass | changes_requested | blocked | escalate",
  "test_result": {
    "tests_passed": true,
    "lint_passed": true,
    "typecheck_passed": true
  },
  "completion_report_ref": ".git/ngagent/tasks/task-001/completion-001.json",
  "review_artifact_ref":   ".git/ngagent/tasks/task-001/review-001.json",
  "dependencies_added":    ["package-name>=version"],
  "interface_changes":     "NONE | free-text summary",
  "notes": "anything the Main Agent should know before deciding merge vs retry"
}
```

If you hit the retry cap, include the `failure_diagnosis` verbatim from
the CompletionReport so the Main Agent can decide retry vs escalate
without re-reading the artifact file.

## 9. Patience Protocol

executor tasks take 5-30+ minutes. Same rules for Sub-agents and
Main Agent:

- **5-minute check interval.** Never poll `ngagent plan` or
  `ngagent task show` more often than once every 5 minutes while a
  spawn is in flight.
- **Never kill a running process** unless: (a) no file changes in the
  worktree for >15 minutes AND no CPU activity (truly hung), or (b)
  the Human explicitly orders abort. A killed process loses ALL
  partial work and wastes every token already spent.
- **Do not `ls` the worktree, `git status` inside it, or
  `ps | grep <provider>`** in a tight loop. Let the process run.
- **Parallelism:** Codex supports up to **6 concurrent Sub-agents**.
  Dispatch as many as the dependency graph allows with disjoint
  `allowed_write_paths`. Never force parallelism on tasks that share
  write scope.
- **Re-assign finished agents** to new `ready` tasks instead of
  spawning fresh Sub-agents. Respect the 6-concurrent cap.
- While waiting, do useful planning work — update
  `context_manifest.md`, plan the next phase, review `architecture.md`.
  Do not babysit.

## Pinned Context is authoritative (v0.8.0)

The rendered execution prompt may contain a top section titled
"Pinned Context From Main Agent". Treat that section as authoritative
over any later architecture block or curated claim that contradicts
it. The ngagent Curator cannot mutate Pinned Context; if a later
section disagrees, follow Pinned and flag the conflict in your
completion report's `notes`.
