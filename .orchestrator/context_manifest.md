# Context Manifest
> This is a LIVING document. Agents MUST append to this after every significant decision.

## Architectural Decisions
<!-- Format: [date] Decision — Rationale -->

## Upstream Summaries

<!-- Main Agent appends after each merged task:
     [task-001] Changed auth.py: verify() → validate(). New: TokenExpiredError.
     [task-002] Added Redis caching layer. New dep: redis>=5.0
-->
- [task-001] Added `.orchestrator/stage-a/01-inventory.md` as the Stage A repository inventory. Downstream tasks must consume its evidence-layer framing: `.gitignore`, tracked state, ignored state, untracked state, and remote state are observations only, not authorities. It explicitly flags `release/` and `bcsp-20260122.zip` as main-checkout local surfaces for task-002, legacy records for task-003, runtime/config artifacts for task-004, and verify rows for task-005/task-006.

## Failed Approaches
<!-- Format: [date] What was tried — Why it failed — Lesson -->

## Interface Contracts
<!-- Current API signatures, data schemas, type definitions -->

## Known Gotchas
<!-- Environment quirks, library bugs, workarounds -->
- [2026-05-11] Existing agent processes may not inherit later persistent user PATH edits. For Claude CLI execution in this session, prefix PATH with `Z:\.npm-global\node_modules\@anthropic-ai\claude-code\bin` so bare `claude` resolves to the installed executable. Review missions remain pinned to Claude Opus 4.7 Max with max effort; no model fallback.
- [2026-05-11] `ngagent merge task-001` created the merge commit correctly, but left `.orchestrator/stage-a/01-inventory.md` staged as deleted in the main worktree. Restoring that single path from HEAD cleaned the index. Check `git status --short --branch` after each `ngagent merge` before pushing.

## Session Log
<!-- Brief summary of each work session for continuity -->

- [2026-05-11] Stage A defined as cleanup/separation/baseline reconstruction. Scope is audit-first and report-oriented: reconcile current repo, release packs, legacy records, Obsidian workflow notes, runtime/config artifacts, and docs before any Stage B refactoring.
- [2026-05-11] Stage A execution policy refined: every task must receive a dedicated ngagent review gate using the Claude CLI/Opus review path where available; every accepted merge should be pushed to `origin`; final Stage A should leave a cleaned and organized repository at the documentation/history/repository-structure layer, without changing product behavior.
- [2026-05-11] Clarified Stage A task accounting: ngagent stores seven durable delivery tasks, but the operational plan has at least fourteen work units because each delivery task is paired with an independent review mission. Review missions are not modeled as normal tasks to avoid recursive review-of-review loops.
- [2026-05-11] Human clarified commit/review policy: each Stage A delivery task should behave like normal work and produce one task commit before review/merge; review missions are independent work units but not Git commits by default. Review must use Opus 4.7 Max only; if that model path is unavailable, block/escalate instead of falling back.
- [2026-05-11] Remote repository is a selective-disclosure surface, not a complete historical source of truth. The human recalls a prior cleanup from "everything submitted" toward "only the project submitted"; Stage A reports must compare remote/tracked state against the local main checkout, ignored artifacts, release/archive files, and historical workflow records before inferring what is current or public.
- [2026-05-11] `.gitignore`, tracked state, ignored state, untracked state, and remote state are evidence layers, not authorities. Current ignored/local status may itself be wrong or stale. Stage A must describe what is observed and why it may be suspect; it must not imply that an ignored artifact is correctly excluded merely because it is ignored today.
