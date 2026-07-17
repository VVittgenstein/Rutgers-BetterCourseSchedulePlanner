# Joint P1 Review

> **IN PROGRESS - ROUND 1 AWAITING USER RULING**

- Review opened: 2026-07-12
- Review owners: User + Codex in the authoritative conversation
- Current phase: P1 Review
- P1 acceptance: not yet granted
- P2 status: blocked and not started

## 1. Review inputs

| Input | Role | Pre-review identity |
|---|---|---|
| `docs/dual-delivery-workflow.md` | Controlling phase/delegation workflow | 217 lines; SHA-256 `8aa54e22676b8f6b873b2e509546305d146cd9d61ecd2e67fe6e558f0fe5073a` |
| `docs/deliverable-a-windows-local-release-requirements.md` | Merged Deliverable A P1 candidate | 377 lines; SHA-256 `3bc42fdd3ed42ad65fa685d98ea862f0fb4a9c8fab0a6cc2236b80418b83e07a` |
| `10-legacy-capability-inventory.md` | Historical behavior/state inventory | 159 rows; 319 lines; SHA-256 `69fa3fe62316a096f70c1d3f22bde2eaca33655a08a2a98c72525a0e87e589c3` |
| `12-p1-p2-purpose-and-delegation-clarification.md` | Current clarification of P1, P2, and delegation | 209 lines; SHA-256 `d22faf9b7944b404bff0477aff11ac7e2bcb9c8cdb07b613c733d8e43e4cd8b5` |
| `13-single-line-p1-revalidation.md` | Pre-review validation verdict | 126 lines; SHA-256 `d870141f3ed4730d317e6eaf9b5214b26c075adaff2db0e3acca92c7e4ccba5b` |

Companions `01`-`09` and the historical `11` validation remain provenance and
chronology inputs. They do not override the later clarification/revalidation.

## 2. Review method

P1 Review is divided into bounded decision rounds so acceptance is explicit
and corrections are attributable:

| Round | Subject | Status |
|---|---|---|
| R1 | P1 purpose, evidence authority, old-plus-new merge, and delegation | `AWAITING USER` |
| R2 | Current accepted Deliverable A product/runtime/watch/package constraints | `NOT STARTED` |
| R3 | Historical capability-inventory completeness by product domain | `NOT STARTED` |
| R4 | Conflicts, exclusions, unresolved later-gate questions, and P2 handoff contract | `NOT STARTED` |
| R5 | Corrections, final consistency validation, and explicit P1 acceptance | `NOT STARTED` |

Reviewing an inventory item does not assign its P2 disposition. P1 Review asks
whether memory and requirements were recovered and merged correctly. P2 later
decides `KEEP`, `RECOVER`, `REPAIR/REDESIGN`, `REMOVE`, `DEFER`, or
`USER DECISION REQUIRED`.

## 3. R1 proposed rulings

| Ruling ID | Statement proposed for acceptance | Current evidence |
|---|---|---|
| `P1-R1-01` | P1 exists because the abandoned task-015-to-025 line and long pause left the User without dependable memory of the old product's intent, features, and design. Persistent local and Git/GitHub evidence must reconstruct it. | July 10 transcript lines 60-86 and 203-215; immutable `0a61028` Phase 1 plan/goals; current User clarification. |
| `P1-R1-02` | P1 has two mandatory layers: historical product recovery and later accepted discussion. The current target is their provenance-preserving merge, not either layer alone. | Workflow P1 contract; target introduction/evidence layers; clarification section 2. |
| `P1-R1-03` | A later direct User decision controls when it conflicts with an old requirement, but the superseded old requirement remains visible as chronology and rationale. | Target conflict ledger; inventory authority rules; clarification merge rule. |
| `P1-R1-04` | Code, UI, routes, docs, tests, configs, branches, and packages prove existence or state, not automatic product membership. Unmerged/archived AI records are discovery evidence rather than accepted truth. | Inventory classifications; task-015 provenance; clarification merge rules 4-5. |
| `P1-R1-05` | P1 recovers and merges memory but does not produce the final all-and-only release surface. P2 performs that adjudication only after P1 is accepted. | Workflow P1/P2 contract; target `DEC-A-009`; clarification sections 4-5. |
| `P1-R1-06` | NGAT remains retired. Codex may dispatch bounded subagents or child threads, but they are subordinate and cannot decide unresolved product questions, approve phases, or cross joint-review gates. | Workflow Delegation Model; target `DEC-A-010`; clarification section 3. |

## 4. Decision log

| Round | User ruling | Corrections requested | Applied/validated | Gate effect |
|---|---|---|---|---|
| R1 | `PENDING` | `PENDING` | No | P1 Review remains open; R2 cannot be treated as reviewed. |

## 5. Current stop point

Codex has opened the review and presented R1. No R1 statement is accepted until
the User explicitly accepts it or supplies corrections. P2 remains blocked.
