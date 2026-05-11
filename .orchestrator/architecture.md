# Architecture Plan

## System Design
Stage A is a governance and evidence-reconciliation project, not a product implementation project.

The working model is:

1. Evidence sources are inspected independently.
2. Findings are recorded as audit artifacts.
3. Conflicts are classified instead of immediately resolved by code edits.
4. A final baseline packet defines the trusted current state and the entry conditions for Stage B refactoring.
5. Approved non-product cleanup is applied to produce a repository that is cleaned, separated, and organized without changing product behavior.

Primary evidence sources:
- Current git repository on `dev`.
- Release archives in `release/` and root-level archive artifacts.
- Legacy planning JSON files: `record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`.
- Historical execution records: `Compact/`, `review/`, `reports/`, `Rutgers-dr/`.
- Current user-facing and developer docs: `README.md`, `docs/`, `.orchestrator/`.
- Obsidian workflow notes under `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner`.
- Local runtime/config surfaces: `data/`, `configs/`, ignored files, generated files, startup scripts.

## Key Interfaces
Stage A agents should produce reports, not product behavior changes.

Expected artifact interfaces:
- Inventory report: classifies repository paths and explains keep/archive/delete-candidate/verify categories.
- Release reconciliation report: compares release pack contents against the current repo and identifies drift.
- Record reconciliation report: maps legacy planning records to actual files and current implementation.
- Runtime/config hygiene report: lists generated data, local configs, ignored artifacts, secret-risk surfaces, and clean-checkout blockers.
- Stage B handoff report: prioritized refactor candidates with evidence and sequencing.
- Cleanup application patch: applies only non-product organization changes justified by the Stage A baseline, such as archive separation, documentation index updates, source-of-truth notes, and removal from tracking of files proven to be generated/local-only.
- Delivery commit: every Stage A delivery task should result in one normal task commit before review/merge, unless the task explicitly discovers that no repository change is appropriate.
- Review gate: every task result is reviewed through `ngagent review <task-id>` by a dedicated reviewer sub-agent using the Claude CLI route with Opus 4.7 Max only. Review model fallback is not allowed; unavailable model/CLI configuration is a blocker, not permission to downgrade.

## Technology Decisions

| Decision | Choice | Rationale | Date |
|----------|--------|-----------|------|
| Stage A execution mode | Audit-first, report-only by default | The project has multiple stale and contradictory evidence sources; changing code before reconciliation would risk preserving the wrong behavior. | 2026-05-11 |
| Active task system | ngagent | The repository now includes ngagent orchestration scaffolding, and Stage A needs durable task state separate from legacy `record.json`. | 2026-05-11 |
| Legacy records | Evidence, not authority | `record.json` and Compact/review files contain useful history but are known to be stale or inconsistent with code and release artifacts. | 2026-05-11 |
| Release artifacts | To be reconciled before trusted | The user suspects release pack drift, and current local data/config artifacts already show path and runtime inconsistencies. | 2026-05-11 |
| Review model | Seven delivery tasks plus seven review missions | The user wants every Stage A task paired with an explicit independent review. To avoid infinite recursion, reviews are modeled as review-gate missions that produce ngagent ReviewArtifacts, not as ordinary implementation tasks that themselves require review. Review must use Opus 4.7 Max only; fallback to another model is not allowed. | 2026-05-11 |
| Task commit shape | One delivery task, one normal task commit | The remote repository already has a task/branch-oriented history. Stage A should preserve that discipline for delivery work; review missions may produce ngagent artifacts but should not create Git commits merely to inflate commit count. | 2026-05-11 |
| Remote sync | Push each accepted task after merge | The user wants every task submitted to the remote repository, so accepted Stage A progress should not remain local-only. | 2026-05-11 |

## File Structure
Stage A may introduce audit artifacts under an approved report area. Proposed structure:
```
.orchestrator/
  goals.md
  architecture.md
  context_manifest.md
  stage-a/
    01-inventory.md
    02-release-reconciliation.md
    03-record-reconciliation.md
    04-runtime-config-hygiene.md
    05-stage-b-handoff.md
    06-final-baseline.md
    07-cleanup-application.md
```

Source code, tests, and package scaffolding remain outside Stage A write scope unless the human approves a later Stage B implementation plan.

## Execution Semantics
Stage A currently has seven durable delivery tasks in ngagent. Operationally, each delivery task has a paired review mission:

1. Dispatch delivery task.
2. Wait for completion.
3. Dispatch a separate review sub-agent to run `ngagent review <task-id>` through the Claude CLI/Opus review path where available.
4. Apply the review decision:
   - `pass`: merge the task into `dev`, push `dev` to `origin`, then continue.
   - `changes_requested`: classify the issue, update task spec or planning docs if needed, retry the same delivery task, then review again.
   - `blocked`: inspect whether the blocker is mechanical, spec, architecture, environment, or external.
   - `escalate`: interrupt the human only for Tier 4 issues.

This means the minimum Stage A operational plan is 14 work units: 7 delivery missions plus 7 review missions. The expected Git shape is one normal delivery commit per delivery task, not one commit per review mission. Additional retry/review cycles may be created dynamically if reviews find real problems.
