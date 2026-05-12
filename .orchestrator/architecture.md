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

## Stage P Public Branch Architecture
Stage P is a publication/synchronization phase. Its target is not a raw merge from internal `dev` into public `main`; that would expose internal planning artifacts and would preserve internal artifacts in public branch history.

The intended branch model is:

```
origin/dev
  Internal working branch. May contain ngagent planning docs, Stage A reports,
  task execution context, and local cleanup evidence.

origin/main
  Public default branch. Should contain only the project-facing engineering
  surface and sanitized public history.

public-main-candidate
  Temporary reviewed candidate branch for replacing/updating main after human
  cutover approval.
```

Stage P must first classify the differences between `origin/main` and `origin/dev`. This matters because `origin/main` currently includes prior public-surface cleanup commits and product-facing remote changes, while `origin/dev` contains the local Stage A reconstruction. The branch conflict is real: the old public cleanup deleted historical workflow records, while Stage A moved similar records into an internal archive.

Expected public branch rules:
- Include product source, tests, package manifests, user-facing docs, public runbooks, schema/config examples, and normal repo metadata.
- Exclude `.orchestrator/`, `.git/ngagent/`, `AGENTS.md`, ngagent docs, Stage A internal reports, `docs/archive/stage-a-legacy/`, historical workflow JSON, Compact/review records, local runtime databases/checkpoints, private config, nested clone `far/`, and cleanup scratch files.
- Prefer sanitized replay commits over merging internal `dev` commits into public `main`.
- Preserve multiple normal commits on the public candidate branch so GitHub shows meaningful project activity without publishing internal workflow content.
- Use the nested clone `far/Rutgers-BetterCourseSchedulePlanner` only as temporary local evidence for `origin/main`; delete the local `far/` directory after Stage P is complete and no longer needs that evidence.

Stage P execution policy:
1. Delivery executors use Claude Opus 4.7 Max.
2. Review missions use GPT-5.5 with xhigh reasoning plus the formal ngagent review gate where applicable.
3. No model fallback is allowed without human instruction.
4. Branch cutover to `main`, force-pushes, default-branch changes, and remote branch deletion are human-approved cutover actions, not automatic task side effects.
5. Internal `dev` remains the coordination branch for ngagent task state unless a later plan changes the orchestration model.

## Stage P Remote Surface Closeout Architecture
Task-011 made `origin/main` clean, but a public repository can still expose non-default branches, tags, and GitHub Releases. For this user's portfolio/display goal, the public remote should not expose the internal construction history.

Closeout target state:

```
origin/main
  The only intended public branch. Points to the reviewed sanitized project tree.

local dev
  Local/internal coordination branch for ngagent records. It must not be pushed
  back to the public `origin` after `origin/dev` is deleted.
```

Remote closeout policy:
- Keep `origin/main`.
- Delete stale public remote branches such as `dev`, `public-main-candidate`, historical `ST-*`/`T-*` task branches, sync branches, patch branches, and other non-main branches after explicit human approval.
- Classify remote tags and GitHub Releases before deletion. Deleting a Git tag is possible through Git; deleting a GitHub Release object may require authenticated GitHub API/CLI support. If release deletion cannot be performed with available credentials/tooling, record the blocker and escalate instead of pretending the release surface is clean.
- Do not push local `dev` to `origin` after the remote closeout, because doing so recreates the public internal branch.
- Do not force-push `main` for closeout. `main` already points to the reviewed clean state; closeout should only remove stale public refs or release surfaces.

## Phase 1 Local Release Architecture
Phase 1 changes the project from cleanup/publication mode back into product delivery mode. The architecture goal is a complete local Rutgers BetterCourseSchedulePlanner release: local web UI, local API, local SQLite database, local Rutgers SOC ingest, local polling, and local notification flows.

Phase 1 must be driven by a feature matrix before implementation starts. The matrix is the product boundary contract and must classify each major surface:

| Status | Meaning |
|--------|---------|
| `complete` | Already exists in the right product direction and needs only release-level validation or polish. |
| `recover` | Was part of the intended local BCSP product but was skipped, disabled, or abandoned because of historical implementation trouble. Recovering it is not treated as new feature creep. |
| `repair` | Exists now but is incomplete, misleading, brittle, poorly integrated, or fails release-level validation. |
| `remove` | Exists in code, docs, UI, or scripts but should not be visible in the Phase 1 release. |
| `defer` | Reasonable future capability, but belongs to Phase 2 refactor, Phase 3 cloud deployment, or later product expansion. |
| `unclear` | Requires human product judgment before implementation. |

The most important product invariant is honesty of the release surface: no user-visible UI, route, command, config, or doc should pretend that a stubbed or deferred feature exists. A feature may be fully implemented, deliberately removed, or explicitly documented as out of scope, but it must not remain fake.

## Phase 1 Product Slices
Phase 1 should be executed as dependent product slices rather than as one broad rewrite:

1. Product boundary and feature matrix.
2. Validation baseline repair.
3. Data ingest, database defaults, and local startup reliability.
4. Course/section API contract repair.
5. Subscription, polling, and notification reliability.
6. UI/UX design direction.
7. Frontend rebuild.
8. UI polish pass.
9. Release packaging and cross-platform startup.
10. Documentation and release notes.
11. End-to-end release candidate verification.

The UI/UX work is explicitly two-stage:
- The design/rebuild task must use `gpt-tasteskill` for product-level interface direction, flows, screen structure, hierarchy, and visual system.
- A later polish task must use `emil-design-eng` for detailed interaction, motion, responsiveness, empty/loading/error states, focus behavior, and tactile quality.

The Main Agent should not load these skills during planning unless the user requests it. The relevant worker/sub-agent should load them when executing the assigned UI/UX task.

## Phase 1 Interface Principles
- `/api/courses` and `/api/sections` must be reconciled against the actual UX. If standalone section search/detail is part of the redesigned product, `/api/sections` must be implemented and validated. If it is not part of the release contract, public docs/UI should not present it as a real feature.
- Rutgers SOC ingest should expose predictable local configuration and failure states. Dry-run and fetch behavior should match docs and release defaults.
- Subscription and notification behavior should be testable without cloud services. Local sound is a first-class Phase 1 notification path. Email is optional only if the product contract says it is optional; if shipped, provider support and setup must be honest.
- Admin/config routes are acceptable only under the local-tool threat model. Phase 1 must not imply hosted multi-user security.
- i18n should remain coherent if the UI keeps multilingual support. Message coverage checks are part of release validation.

## Phase 1 Distribution Policy
The user wants an archive that a non-expert can unpack and use. The existing Node/Vite/Fastify/better-sqlite3 stack makes this a packaging problem, not just a ZIP problem, because native dependencies and local runtime state can break clean-machine startup.

Phase 1 should therefore distinguish:
- Windows release candidate: must be directly validated in the current Windows environment.
- macOS support: should be prepared in scripts/docs/package design, but cannot be claimed as validated until a real macOS run confirms install/start/fetch/use behavior.
- Developer source package: acceptable only as a secondary artifact, not as the primary "unzip and use" promise.

Packaging tasks may choose the minimal reliable mechanism after investigation, but the selected mechanism must make prerequisites, native dependency behavior, database paths, generated configs, logs, and troubleshooting explicit.
