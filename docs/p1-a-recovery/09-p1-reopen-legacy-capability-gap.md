# P1 Reopen Decision - Legacy Capability Recovery Gap

> **P1 REOPENED - P2 BLOCKED**

- Decision date: 2026-07-12
- Owner: the current Codex conversation under the single-line workflow
- Supersedes: only the P1-complete verdict in `08-mainline-review.md`
- Preserves: the three accepted corrections in that review
- Next stop: renewed joint P1 Review after remediation and independent validation

## 1. User concern

The first P1 result recovered the later 0A-0C decisions and historical release
purpose in detail, but did not recover the old project's product capabilities
at behavior-level granularity. For example, it reduced the old filtering
surface to generic "course search and filtering" even though source-backed
records describe multi-value filters, day and time-window filters, simultaneous
filter composition, range filters, and exact meeting-match semantics.

This is a P1 coverage defect, not a request to add an unrelated new feature.
Deliverable A's recovered target must include both:

1. decisions and constraints discussed in the current conversation; and
2. requirements and capability candidates recovered from old code,
   architecture, tests, documentation, release artifacts, Git history, and
   historical records.

## 2. Confirmed gap evidence

| Evidence | What it proves |
|---|---|
| `docs/deliverable-a-windows-local-release-requirements.md`, `A-UI-001` | The first candidate says only "course search and filtering" and does not enumerate behavior. |
| `docs/p1-a-recovery/04-recovery-corpus-evidence.md`, `RC-E011` | The first recovery explicitly admits that exact filters and UX behavior were not enumerated. |
| `README.md`, lines 149-162 | The documented old product promises simultaneous filtering by status, subject, location, day, time, credits, Core, Exam, prerequisites, teaching mode, and level. |
| `frontend/src/state/courseFilters.ts`, lines 19-43 and 69-76 | The implemented state contract contains ranges, multi-value groups, meeting windows, open-only state, pagination, and sorting. |
| `docs/ui_flow_course_list.md`, lines 97-106 | The historical architecture specifies subset-day semantics, time windows, multi-value URL encoding, and composed filter state. |
| `api/tests/course_search.test.ts`, lines 32-43 | Tests exercise a combined meeting-day, delivery-mode, and time-window query. |
| `docs/archive/stage-a-legacy/Compact/Compact-ST-20251122-filter-rewrite-01-frontend-state-ui-2025-11-23-T062853Z.md`, lines 4-19 | Historical task records preserve feature chronology, removals, and exact semantics that generic summaries lost. |

These examples establish the defect. They are not the full remediation
inventory; the same omission risk must be checked across every product domain.

## 3. Inventory versus adjudication

P1 and P2 have different jobs:

- **P1 recovery:** enumerate every source-backed legacy capability and behavior,
  preserve chronology and contradictions, and classify evidence authority.
- **P2 adjudication:** decide whether each recovered item is `KEEP`, `REMOVE`,
  `REDESIGN`, or `DEFER` in the all-and-only release surface.

P1 must not omit a capability merely because P2 has not accepted it. Conversely,
the existence of code, UI, documentation, or an old task does not automatically
make that capability a final v1 requirement.

## 4. Required remediation

The single-line P1 execution must:

1. build a complete capability inventory across discovery/filtering, course and
   section presentation, schedule views, subscription/watch behavior,
   notifications, data acquisition/refresh, persistence, configuration,
   localization, accessibility, startup/packaging, diagnostics, administration,
   sharing/saved state, and any other source-backed domain discovered;
2. inspect current and recovered code, tests, architecture documents, README and
   runbooks, historical compacts, release artifacts, relevant Git history, and
   available conversation records rather than relying on a summary document;
3. record for every capability its exact behavior, source locators, chronology,
   implementation/test/documentation state, contradictions, confidence, and
   unresolved P2 disposition;
4. distinguish course-level, section-level, meeting-level, and watch-level
   behavior instead of merging them under broad labels;
5. update the Deliverable A P1 candidate so behavior-level requirements and the
   complete legacy-candidate inventory are visible to P2;
6. run an independent coverage audit against the source corpus; and
7. stop at a renewed joint P1 Review without beginning P2.

## 5. Governance change

The earlier dual-line organization is retired. The A/B dual-delivery phases and
their outputs remain unchanged. Authority, integration, and review remain in
this one Codex conversation:

- Codex performs the bounded execution work and validation and may supervise
  bounded subagents or child threads as subordinate execution tools.
- The User and Codex jointly review and approve required gates.
- A delegated worker cannot become a second authority line or cross a gate.
- NGAT is historical context only.

The controlling operating model is `docs/dual-delivery-workflow.md`; the later
clarification is recorded in
`docs/p1-a-recovery/12-p1-p2-purpose-and-delegation-clarification.md`.
