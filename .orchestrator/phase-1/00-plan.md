# Phase 1 Local Release Plan

## Purpose
Phase 1 should produce a complete local Rutgers BetterCourseSchedulePlanner release. It is not a minimal MVP and not a broad Phase 2 refactor. It should recover the intended local-tool product, repair the current working-but-rough implementation, and remove release surfaces that should not exist.

## Product Boundary Rule
"All features must exist" means all features in the Phase 1 local-release product contract must be real, usable, and documented. It also means features that should not exist in Phase 1 should not be visible as fake UI, fake docs, fake routes, stale scripts, or misleading config.

Historical local-product ideas that were skipped or disabled because of implementation trouble are `recover` candidates. Current code that exists but is wrong, stubbed, confusing, or obsolete is not automatically a ship candidate.

## Feature Statuses
| Status | Release Meaning |
|--------|-----------------|
| `complete` | Already correct enough for the local product; verify and polish. |
| `recover` | Intended local-product behavior was skipped, disabled, or abandoned; restore it if evidence supports it. |
| `repair` | Exists but is broken, misleading, brittle, or poorly integrated. |
| `remove` | Exists somewhere but should not appear in Phase 1. |
| `defer` | Valid future work, but belongs to Phase 2, Phase 3, or later. |
| `unclear` | Needs human product decision before implementation. |

## Required Local Flow
The release candidate must support this flow end to end:

1. Unpack or set up the local release without project-specific hidden state.
2. Start the local service/UI.
3. Fetch or load Rutgers SOC data into local SQLite.
4. Search/filter courses.
5. Inspect the section-level information needed for registration decisions.
6. Subscribe to a target section/course condition.
7. Run open-seat polling.
8. Receive a local notification, including local sound.
9. Manage or remove the subscription.
10. Recover from common empty, loading, error, and config states without guessing.

## Planned Task Graph
1. `task-015` — `Phase 1 release surface audit and feature matrix`
   - Produce the durable feature matrix.
   - Decide which surfaces are `complete`, `recover`, `repair`, `remove`, `defer`, or `unclear`.
2. `task-016` — `Phase 1 repair validation baseline`
   - Bring TypeScript/test/script validation to a credible baseline or remove stale failing expectations.
3. `task-017` — `Phase 1 repair data ingest, DB defaults, and local startup`
   - Make fetch, migration, database path, generated artifacts, and one-click startup coherent.
4. `task-018` — `Phase 1 repair course and section API contract`
   - Reconcile `/api/courses`, `/api/sections`, docs, and frontend needs.
5. `task-019` — `Phase 1 repair subscriptions, polling, and notifications`
   - Make subscription creation, polling, local sound, and optional email honest and verified.
6. `task-020` — `Phase 1 design UI/UX direction with gpt-tasteskill`
   - Use `gpt-tasteskill` in the worker task.
   - Produce the screen/flow/design contract for implementation.
7. `task-021` — `Phase 1 implement frontend UI/UX rebuild with gpt-tasteskill`
   - Implement the redesigned UI against the real API/product contract.
8. `task-022` — `Phase 1 polish frontend interaction quality with emil-design-eng`
   - Use `emil-design-eng` in the worker task.
   - Refine motion, states, focus, responsiveness, and detailed usability.
9. `task-023` — `Phase 1 build release packaging and cross-platform startup path`
   - Produce the practical local release mechanism.
   - Validate Windows directly and prepare macOS support honestly.
10. `task-024` — `Phase 1 align user docs and release notes`
   - Make README, quickstart, troubleshooting, and release notes match the actual release.
11. `task-025` — `Phase 1 final release candidate verification`
   - Run full local validation and produce a release-readiness report.

## Non-Goals
- Google Cloud deployment, hosted multi-user operation, cloud queues, cloud DBs, and production auth.
- Full architecture rewrite beyond what is necessary for local release correctness.
- Speculative new product features not supported by local BCSP product intent.
- Publishing internal ngagent/orchestrator artifacts as part of the public user release.
- Claiming macOS validation without a real macOS validation run.

## Execution Gate
The current ngagent plan has `task-015` ready and `task-016` through `task-025` blocked on dependencies. No implementation sub-agent should be dispatched until the human approves this Phase 1 task plan.

After approval, the Main Agent should dispatch only the ready task(s), review each completion through ngagent review, merge passed tasks, record upstream interface changes in `context_manifest.md`, then continue through the dependency graph. If task-015 marks any feature as `unclear`, implementation tasks that depend on that decision should pause for human product judgment rather than guessing.
