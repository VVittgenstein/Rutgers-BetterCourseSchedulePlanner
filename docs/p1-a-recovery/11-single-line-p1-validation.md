# Single-Line P1 Remediation Validation

> **HISTORICAL VALIDATION SNAPSHOT - SUPERSEDED DURING JOINT REVIEW**

This file records the validation state before the User clarified P1's
task-015/memory-reconstruction origin, the mandatory historical-plus-current
merge, allowed Codex subagents/child threads, and P2's dependency-aware
all-and-only standard. See
[`12-p1-p2-purpose-and-delegation-clarification.md`](12-p1-p2-purpose-and-delegation-clarification.md)
and the fresh validation in
[`13-single-line-p1-revalidation.md`](13-single-line-p1-revalidation.md).
The pass below remains valid for its exact artifact snapshot but is not the
current gate verdict.

- Validation date: 2026-07-12
- Validator: Codex, in a separate verification pass after synthesis
- Workflow: one authoritative conversation; Codex execution followed by User + Codex review
- Scope: P1 recovery completeness and provenance only
- P2 status: not started

## 1. Validation question

Does the remediated P1 candidate now recover both parts of Deliverable A's
input without making P2 decisions?

1. the later requirements and constraints accepted in the current discussion;
2. the old project's source-backed product requirements, code behavior,
   architecture, removed behavior, stubs, package drift, and chronology.

Result: **yes, as a P1 candidate awaiting joint review**. This result does not
mean that all recovered legacy items belong in v1.

## 2. Artifact integrity checks

| Check | Result | Evidence |
|---|---|---|
| Capability IDs are complete and deterministic | PASS | 159 rows, 159 unique IDs, contiguous `LEG-A-001` through `LEG-A-159`, no gaps or duplicates. |
| Evidence register is closed | PASS | 34 unique `LC-E` definitions; every referenced `LC-E` resolves; no undefined evidence IDs. |
| Target incorporates the whole inventory | PASS | 22 `A-LEG-*` groups cover all 159 rows exactly once, with no gap or overlap. |
| Target exposes behavior rather than a generic filter label | PASS | `A-UI-001`, ordinary-user journey step 5, and `A-LEG-FILTER-001` through `A-LEG-FILTER-005` name retained, removed, composed, meeting, and stubbed behavior. |
| Chronology is preserved | PASS | Original requirements, current implementation, removed Git state, orphan branch/package code, current superseding decisions, and unresolved P2 membership are separate columns/classifications. |
| P1/P2 boundary is preserved | PASS | Inventory rows are explicitly not tasks or automatic v1 membership; P2 must later assign `KEEP`, `REMOVE`, `REDESIGN`, or `DEFER`. |
| Workflow ownership is current | PASS | `docs/dual-delivery-workflow.md` retains A/B dual delivery and P1-P7, but execution and review occur in this one conversation. |

## 3. Original requirement coverage

### 3.1 Functional requirements

| Historical requirement | Inventory coverage | Validation |
|---|---|---|
| FR-01: selected term/campus and complete course/section fields | `LEG-A-017`-`LEG-A-024`, `LEG-A-084`-`LEG-A-086` | Covered, including the current sparse-detail gap. |
| FR-02: multidimensional combined filters | `LEG-A-044`-`LEG-A-083` | Covered field by field, including current filters, AND/OR/same-section semantics, and filters removed by the rewrite. |
| FR-03: week/calendar and day/time filtering | `LEG-A-056`-`LEG-A-058`, `LEG-A-091`-`LEG-A-100` | Covered, distinguishing strict meeting filters, orphan schedule UI, and explicitly later free-time/conflict/ICS ideas. |
| FR-04: subscribe to a specific section | `LEG-A-106`-`LEG-A-110`, `LEG-A-121`-`LEG-A-124`, `LEG-A-133` | Covered with old Index-centric flow and current live-watch/identity correction separated. |
| FR-05: detect opening and notify | `LEG-A-111`-`LEG-A-132`, `LEG-A-157` | Covered with old transition/dedupe behavior explicitly superseded by every-fresh-Open browser audio. |
| FR-06: view/manage/cancel subscriptions | `LEG-A-109`, `LEG-A-121`-`LEG-A-123` | Covered as an old persistent-user outcome whose current connection-scoped translation remains for P2. |
| FR-07: English/Chinese UI | `LEG-A-011`, `LEG-A-105` | Covered, including persistence, HTML language, and missing-key checks. |
| FR-08: one-click deployment | `LEG-A-002`, `LEG-A-013`, `LEG-A-142`-`LEG-A-147`, `LEG-A-151`, `LEG-A-156` | Covered with old cloud mechanics separated from current Windows package and B deployment-package decisions. |

### 3.2 Non-functional requirements and old success metrics

| Historical target | Inventory coverage | Validation |
|---|---|---|
| At least 95% official-field coverage | `LEG-A-018`-`LEG-A-024` | Preserved as historical metric; not asserted as achieved. |
| <3 s initial load, <0.5 s filter feedback, about 1,000 courses, 200 concurrent browsers | `LEG-A-149` | Preserved as architecture-dependent historical targets, not current claims. |
| Old notification average <30 s, maximum <60 s, no misses | `LEG-A-132`, `LEG-A-157` | Preserved and explicitly contrasted with current near-one-second aspiration and different trigger model. |
| Free/very-low cost and example <5,000 durable subscriptions | `LEG-A-154` | Preserved as an old cloud/mail target, not an A or Vultr capacity promise. |
| Privacy/no unnecessary personal data or secrets | `LEG-A-004`, `LEG-A-014`, `LEG-A-143`, `LEG-A-151` | Covered. |
| Lint/tests and at least 80% core-logic coverage | `LEG-A-148`, `LEG-A-150` | Preserved as a historical quality target; final validation policy remains later work. |
| Time-view usability and one-hour third-party deployment goals | `LEG-A-155`, `LEG-A-156` | Preserved as unproven historical targets. |

## 4. Source-domain coverage

| Domain | Sources checked | Material recovered |
|---|---|---|
| Original product research | `read_only.md`, deep research, architecture patch | Goals, non-goals, FR/NFR, complete field/filter lists, week view, language, one-click, later ideas, and unresolved scope. |
| Historical task system | `record.json`, `rRevision.json`, `rSubscribe.json`, `rEmail.json`, Compact files | Data/API/UI/subscription/poller/mail/deployment chronology, parent/subtask inconsistencies, final filter rewrite, and removed fields. |
| Current frontend | App, FilterPanel, state, query hooks, CourseList, SchedulePreview, subscription/sound/i18n/fetch components | Mounted versus unmounted behavior, exact controls, query interaction, sparse details, old subscription UI, sound polling, and language behavior. |
| Current API/data | Course query/routes/tests, section stub, dictionaries, data model, ingest/refresh docs, SOC notes | AND/OR and meeting semantics, test evidence, field model, dynamic/fallback dictionaries, exam mismatch, data limitations, and empty section route. |
| Workers/runtime | Poller/open-event docs/tests, notifications, launcher, health/readiness/logging | Transition/dedupe/checkpoint model, local claim polling, Node/native-build prerequisites, and diagnostics. |
| Immutable Git history | Pre-rewrite `d845a09`, auto-refresh `e770bf2`, task-015 `5714a8f` | Removed filters, orphan scheduled/auto refresh, and unmerged matrix provenance boundary. |
| Release artifacts | 2026-01-21 ZIP/TAR and 2026-01-22 ZIP | Divergent layouts/content and packaged-but-unwired scheduled-refresh files. |
| Main-machine recovery corpus | `Z:/resume-from-main-machine/Rutgers-BetterCourseSchedulePlanner/current` | Confirmed it is a 20-file recovery index, then incorporated its source inventory, product direction, unresolved surfaces, and chronology without mistaking it for a product checkout. |
| Current decisions | 0A, 0B, 0C, workflow, prior P1 evidence | Windows A, Vultr B, shared Rust/React architecture, browser audio, nine connection-memory watches, WebSocket, ten-minute catalog refresh, and phase gates. |

## 5. Conflict checks

| Conflict | Validation result |
|---|---|
| Generic filtering versus exact old behavior | Fixed: 40 filter rows cover retained fields, composition, strict meeting semantics, UI/query behavior, removed fields, and the section stub. |
| Code existence versus working user surface | Fixed: mounted, unmounted/orphaned, removed, stubbed, tested, and historical-only states are distinct. |
| Old durable subscriptions versus current live watches | Fixed: old user outcomes/mechanics and current accepted rules are both present, with supersession explicit. |
| Old email/Discord versus current audio only | Fixed: mail/Discord history is retained but v1 exclusion and future-email boundary control. |
| Catalog refresh versus open-status polling versus browser result refresh | Fixed: they are three separate inventory behaviors and must not share one cadence by accident. |
| Old Node/Fastify implementation versus final Rust design | Fixed: implementation evidence informs behavior/drift, while 0C controls future architecture. |
| Old archive contents versus release proof | Fixed: archives are evidence of divergence/orphans, not canonical release artifacts. |
| Old static/cloud performance targets versus current A/B architecture | Fixed: old metrics are historical hypotheses until P6 defines and P7 measures current targets. |

## 6. Residual uncertainty intentionally left for later gates

The following are not P1 omissions:

- P2 must decide all-and-only membership for every applicable `LEG-A` row,
  especially removed filters, Calendar, detailed section UI, presets/share,
  Compact view, and local fetch/admin surfaces.
- P2/P3 must verify the stable Rutgers section identity and define the final
  section-detail outcome without inheriting the empty route.
- P3-P5 must define WebSocket schema, liveness, reconnect/replacement,
  polling policy, Rust module split, and A/B adapters.
- P6 must approve current performance, capacity, package, security, and
  usability validation thresholds.
- P7.2 and P7.3 remain the separate mandatory UI implementation and polish
  subphases recorded in the workflow.

## 7. Artifact snapshot

| Artifact | Lines | SHA-256 at validation |
|---|---:|---|
| `docs/deliverable-a-windows-local-release-requirements.md` | 359 | `e0ad4dc8278bd608b875cfeb747bf661b04f3db74b366e16c1bf70c2e2313492` |
| `docs/p1-a-recovery/09-p1-reopen-legacy-capability-gap.md` | 88 | `572b0ec6596bc468ebdc98eca06978af8e858a1b3eae179fdca9f350f9a10867` |
| `docs/p1-a-recovery/10-legacy-capability-inventory.md` | 308 | `90ef8c732eddb0c3263496ee7032666583dd022a37fc5bfba294c6f530db1184` |
| `docs/dual-delivery-workflow.md` | 129 | `f344cbd05b6aa472939402acae8e3353239b52004517812a38537685713e1bca` |

These hashes identify the inputs inspected by this validation pass. Adding this
validation file or a link to it necessarily changes a linked file's later hash;
the table is an audit snapshot, not a self-hash promise.

## 8. Stop-gate verdict

P1 remediation passes the single-line completeness/provenance check and is
ready for the renewed joint P1 Review. It is not yet approved. P2 has not
started, no implementation work was authorized, and Codex must stop here for
the User's review.
