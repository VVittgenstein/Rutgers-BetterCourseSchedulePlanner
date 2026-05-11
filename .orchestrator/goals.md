# Project Goals

## Objective
Stage A is a cleanup, separation, and baseline-reconstruction phase for Rutgers-BetterCourseSchedulePlanner.

The goal is to establish a trustworthy picture of the existing project before any new feature work or architectural rewrite begins. This phase treats the current repository, release archives, Obsidian workflow notes, legacy `record.json` files, Compact/review records, README/docs, local runtime artifacts, and generated configs as separate evidence sources that must be reconciled rather than assumed correct.

Stage A should produce a clear handoff into Stage B refactoring: what is current, what is historical, what is unsafe or stale, what should be archived, and which modules deserve redesign.

## Success Criteria
- [ ] Current repository contents are inventoried by category: product source, tests, current docs, historical AI workflow records, generated/runtime artifacts, local-only configs, release artifacts, and unknowns.
- [ ] Current repository and release pack(s) are compared, with differences classified by significance and likely source of truth.
- [ ] Legacy planning records (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `Compact/`, `review/`, Obsidian notes) are classified as authoritative/current, historical evidence, stale, contradictory, or unsafe to rely on.
- [ ] The current minimal runnable path is documented from a clean checkout, including required generated data/config steps and any blockers.
- [ ] Local/private artifacts and secret-risk surfaces are identified without exposing secret values.
- [ ] A Stage B refactor candidate list is produced with evidence, priority, blast radius, and recommended sequencing.
- [ ] Each Stage A delivery task produces one normal task commit before review/merge, unless the task is explicitly found to require no repository change.
- [ ] Each Stage A task is independently reviewed through the ngagent review gate by a Claude CLI reviewer configured for Opus 4.7 Max; review must block rather than fall back to another model if that model is unavailable.
- [ ] Each reviewed and merged Stage A task is pushed to the remote repository so remote state tracks the accepted cleanup work.
- [ ] The final Stage A repository is cleaned, separated, and organized at the repository-structure/documentation/history layer: product code remains behaviorally unchanged, while current-vs-historical sources of truth are explicit.
- [ ] No product functionality, source-code architecture, or runtime behavior is changed without a separate Stage B plan.

## Out of Scope
- Adding product features.
- Rewriting architecture or refactoring source modules.
- Deleting historical records or release artifacts before explicit approval.
- Treating legacy `record.json` as the active task system without reconciliation.
- Treating any release pack as authoritative until it is compared against the repository.
- Publishing a new release pack.

## Constraints
- Use `ngagent` as the task-state system for Stage A.
- Main Agent owns planning, decomposition, tracking, and merge decisions; implementation work is delegated after plan approval.
- Stage A execution starts audit/report oriented. Source code and test files are read-only unless a later approved task explicitly changes scope.
- Stage A may include non-product cleanup after audits establish what should move or be separated. This cleanup may affect docs, planning files, archive/history placement, and repository metadata, but not product behavior.
- All Stage A findings must be evidence-backed and traceable to file paths, release contents, command outputs, or recorded history.
- Secrets and private user config values must be redacted in reports.
- Review is not modeled as a recursive implementation task. Instead, every task must pass a dedicated ngagent review gate before merge.
- Review model fallback is not allowed for Stage A. If the requested Opus 4.7 Max review path cannot run, the task is blocked/escalated instead of reviewed by a weaker or different model.
- After each successful merge into `dev`, push `dev` to `origin` unless the remote rejects or the human pauses execution.
