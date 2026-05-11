# Context Manifest
> This is a LIVING document. Agents MUST append to this after every significant decision.

## Architectural Decisions
<!-- Format: [date] Decision — Rationale -->

## Upstream Summaries

<!-- Main Agent appends after each merged task:
     [task-001] Changed auth.py: verify() → validate(). New: TokenExpiredError.
     [task-002] Added Redis caching layer. New dep: redis>=5.0
-->

## Failed Approaches
<!-- Format: [date] What was tried — Why it failed — Lesson -->

## Interface Contracts
<!-- Current API signatures, data schemas, type definitions -->

## Known Gotchas
<!-- Environment quirks, library bugs, workarounds -->

## Session Log
<!-- Brief summary of each work session for continuity -->

- [2026-05-11] Stage A defined as cleanup/separation/baseline reconstruction. Scope is audit-first and report-oriented: reconcile current repo, release packs, legacy records, Obsidian workflow notes, runtime/config artifacts, and docs before any Stage B refactoring.
- [2026-05-11] Stage A execution policy refined: every task must receive a dedicated ngagent review gate using the Claude CLI/Opus review path where available; every accepted merge should be pushed to `origin`; final Stage A should leave a cleaned and organized repository at the documentation/history/repository-structure layer, without changing product behavior.
- [2026-05-11] Clarified Stage A task accounting: ngagent stores seven durable delivery tasks, but the operational plan has at least fourteen work units because each delivery task is paired with an independent review mission. Review missions are not modeled as normal tasks to avoid recursive review-of-review loops.
- [2026-05-11] Human clarified commit/review policy: each Stage A delivery task should behave like normal work and produce one task commit before review/merge; review missions are independent work units but not Git commits by default. Review must use Opus 4.7 Max only; if that model path is unavailable, block/escalate instead of falling back.
- [2026-05-11] Remote repository is a selective-disclosure surface, not a complete historical source of truth. The human recalls a prior cleanup from "everything submitted" toward "only the project submitted"; Stage A reports must compare remote/tracked state against the local main checkout, ignored artifacts, release/archive files, and historical workflow records before inferring what is current or public.
