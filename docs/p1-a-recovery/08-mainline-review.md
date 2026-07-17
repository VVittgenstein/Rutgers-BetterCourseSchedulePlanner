# P1 Mainline Review - Deliverable A

> **HISTORICAL REVIEW RECORD - COMPLETION VERDICT SUPERSEDED**

This record accurately preserves the 2026-07-12 review that accepted three
corrections. Its P1 completion verdict was later superseded on the same date
after the User identified that the old project's detailed capability and
behavior inventory had not been recovered. See
[`09-p1-reopen-legacy-capability-gap.md`](09-p1-reopen-legacy-capability-gap.md).

- Review date: 2026-07-12
- Reviewed target: Deliverable A, the Windows-local BCSP release
- NGAT thread: `RBSCP-NGAT` (`019f5001-5b34-7c81-8784-f454c2f934d1`)
- NGAT terminal dev merge: `a4b035a586a4b14fc3a75698caf99badce869fd5`
- Gate boundary: P2 has not started and requires a separate explicit handoff.

## 1. Review inputs and preservation boundary

The current discussion line reviewed the task-067 terminal candidate together
with its source register, evidence ledgers, independent audit, and stop-gate
report. The reviewed pre-mainline candidate was:

- path: `docs/deliverable-a-windows-local-release-requirements.md`;
- contentful lines: 303;
- raw-byte SHA-256:
  `aede4b98b69219b879a4c997f92ea3066cde79ac214e7990a23ce1f7c8edeef2`.

Companions `01` through `07` remain the immutable NGAT recovery and audit
record. They intentionally continue to describe and bind the pre-mainline
candidate. Mainline acceptance and corrections are recorded here rather than
silently rewriting that audit trail.

The eight terminal P1 artifacts were materialized from the reviewed task-067
worktree into the parent checkout. Their false deletion state, caused by the
parent checkout not being materialized after the dev merge, was removed without
changing unrelated tracked or untracked worktree content.

## 2. Accepted mainline corrections

The User accepted all three corrections below in the current discussion line.

| Review item | Accepted correction | Target locations | Evidence |
|---|---|---|---|
| `MR-A-001` | A/B v1 still contain no email, mail configuration, or mail fallback. Email reminders are explicitly GitHub-tracked future work rather than an exposed v1 surface. | `DEC-A-004`, `A-CFG-003`, `EX-A-001`, conflict ledger | `ML-E008`, `ML-E025` |
| `MR-A-002` | P7.2 must explicitly use `$industrial-brutalist-ui` and `$design-taste-frontend`. P7.3 is a later independent subphase that must explicitly use `$emil-design-eng`, with a `Before \| After \| Why` review artifact. | `A-UI-004` | `ML-E022`, `ML-E070`, `ML-E071`, `ML-E074` |
| `MR-A-003` | Every P7 task must create its own clean, substantive commit and push it to the approved remote branch after its task-level completion gate. Reviewability, safety, and public cleanliness outrank contribution-graph appearance; P7.2 and P7.3 remain distinct. | `A-VAL-006` | `ML-E074`, `ML-E077` |

The review also removed obsolete “awaiting P1 Review” control language. Explicit
inferences remain labeled by evidence class, but unresolved mechanics continue
to the named design or validation gate instead of reopening P1.

## 3. Accepted target identity

The accepted P1 target baseline is:

- path: `docs/deliverable-a-windows-local-release-requirements.md`;
- contentful lines: 310;
- raw-byte SHA-256:
  `2f9ad22a0818b03b8f40e25d953ce80c887c4ba945f874baac975ffecb06252c`;
- requirement rows: 53 unique requirement IDs, unchanged from the audited
  candidate;
- evidence references: all 571 pre-review evidence-ID occurrences remain, and
  `ML-E022` is now attached directly to `A-UI-004`; the accepted target has 572
  occurrences across 126 distinct evidence IDs.

The pre-mainline hash in Section 1 and the accepted-target hash above identify
different, intentional states. The latter is the controlling Deliverable A P1
baseline for later phases.

## 4. Questions retained for later gates

P1 acceptance does not invent answers to the nine questions already identified
by the NGAT candidate:

| ID | Deferred decision | Owning later gate |
|---|---|---|
| `UNR-A-001` | Stable section identity/key | P2/P3 data and interface design |
| `UNR-A-002` | WebSocket schema, versioning, liveness, reconnect, and replacement semantics | P3-P5 contract design |
| `UNR-A-003` | Responsible openSections cadence, adaptive policy, timeout, and backoff | P3/P5 design and P7 validation |
| `UNR-A-004` | Status-to-alert measurement definition and acceptable observed distribution | P6 approval and P7 validation |
| `UNR-A-005` | A and B capacity/resource thresholds | P4-P6 planning and P7 capacity testing |
| `UNR-A-006` | Final shared UI treatment and A-specific configuration placement | P7.2 implementation and P7.3 polish |
| `UNR-A-007` | Rust crate/module, contract, and ownership boundaries | P3-P5 design |
| `UNR-A-008` | Final Windows archive, package tree, per-user paths, upgrade, launcher, and diagnostics mechanics | P3 design, P6 review, and P7 validation |
| `UNR-A-009` | All-and-only membership of optional historical product surfaces | P2 product-surface audit |

## 5. Gate result

P1 is complete. The accepted target is locally persisted and ready to be used
as an input to P2. No source code, test, package, server, credential, release,
or remote branch was changed by this mainline review.

This completion does **not** automatically initiate P2. Starting P2 requires a
new, explicit NGAT handoff from the current discussion line, preserving the
dual-line workflow and its stop gates.

## 6. Supersession note

Section 5 records the verdict at the time and is not current authorization.
The legacy gap was reopened in
[`09-p1-reopen-legacy-capability-gap.md`](09-p1-reopen-legacy-capability-gap.md),
remediated in
[`10-legacy-capability-inventory.md`](10-legacy-capability-inventory.md), and
validated in
[`11-single-line-p1-validation.md`](11-single-line-p1-validation.md). The active
workflow now uses this one conversation rather than an NGAT handoff, and P2 is
still blocked pending renewed joint P1 Review.
