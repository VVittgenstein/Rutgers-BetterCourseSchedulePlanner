# Single-Line P1 Revalidation After Scope Clarification

> **REVALIDATION PASS - STOP AT JOINT P1 REVIEW**

- Revalidation date: 2026-07-12
- Scope: the current User clarification plus the prior behavior-level P1 remediation
- Authority model: one User/Codex conversation with optional Codex-supervised subagents or child threads
- P2 status: not started
- Supersedes as current verdict: `11-single-line-p1-validation.md`

## 1. Clarifications under test

The User corrected four points during P1 Review:

1. P1 was created because the abandoned task-015 line and a long pause left the
   old product intent, features, and design difficult for the User to remember;
   persistent local and GitHub history must restore that memory.
2. P1 is not old-history-only. It must merge historical recovery with newer
   accepted discussion, including Rust architecture and UI/workflow decisions.
3. NGAT is retired, but the current Codex may create subordinate agents or
   child threads and remains responsible for their work.
4. P2 exists because the old workflow and requirements left extensive drift.
   Its all-and-only result must include every required capability and completely
   remove every non-member surface and dedicated support chain without harming
   shared retained systems.

Result: **all four corrections are now explicit in the controlling documents**.

## 2. Historical-anchor verification

| Check | Result | Evidence |
|---|---|---|
| task-015 was the first gate, not the whole goal | PASS | `0a61028:.orchestrator/phase-1/00-plan.md`, lines 35-61, defines task-015 through task-025. |
| Old Phase 1 was a feature-complete local release | PASS | The same plan, lines 3-9 and 21-33, defines complete/recover/repair/remove/defer/unclear and the end-to-end local flow. |
| Earlier limitations distorted the intended product | PASS | `0a61028:.orchestrator/goals.md`, lines 79-107, requires recovery of intended behavior and removal of fake/stale surfaces. |
| July 10 conversation recovered the whole abandoned round | PASS | `chat-log-codex-2026-07-10-1ce70862.md`, lines 60-86. |
| Original workflow defined P1 recovery and P2 all-and-only | PASS | Same transcript, lines 203-225, especially direct User lines 213 and 215. |
| Unmerged task-015 matrix is not accepted truth | PASS | Commit `5714a8f` remains unmerged discovery evidence and is explicitly classified that way in the target and clarification. |

Verified immutable identities:

- conversation export: 1,277 lines, SHA-256
  `76fee2f09567da0afde6fe9f36048c098b6f09f363ba7ad18b8abe5ea8387562`;
- Phase 1 plan commit/blob:
  `0a61028c91a93906758d41120fd9544ae889cbc7` /
  `943965e84fa56d58cd2c3e28492973d487a86e65`;
- Phase 1 goals blob:
  `ebd9fc16252887b08a30607f3cb8ea42f60aab86`;
- unmerged task-015 commit/blob:
  `5714a8f19481d22691ba799992609e6a5f619d02` /
  `4e0f3b04c0499edabdb4c884517fd4ed9d366770`.

## 3. P1 merge-contract verification

| Required property | Result | Where it is now controlled |
|---|---|---|
| P1 is memory reconstruction after task-015/pause | PASS | Workflow P1 row and `Why P1 exists`; target `HIST-A-008`; clarification sections 1-2. |
| Historical sources include local persistence and Git/GitHub history | PASS | Workflow P1 row/current historical layer; clarification sections 1, 2.1, and 5.1. |
| Newer decisions are a mandatory second layer | PASS | Workflow P1 row/current layer; target introduction/evidence layer; clarification section 2.2. |
| Rust/shared architecture is included | PASS | Target `DEC-A-002`/`DEC-A-003`; inventory `LEG-A-153`; workflow current layer. |
| Current UI requirements are included without doing P7 design in P1 | PASS | Target `A-UI-004`; workflow P7.2/P7.3; clarification section 2.2. |
| Later direct decisions supersede conflicts without erasing history | PASS | Target conflict ledger; inventory authority rules; clarification section 2.3. |
| Implementation existence does not decide membership | PASS | Target/inventory classifications and clarification merge rule 4. |

The 159-row capability inventory remains complete and is the historical
behavior layer of the merged target, rather than the whole P1 result by itself.

## 4. Delegation-contract verification

| Required property | Result | Evidence |
|---|---|---|
| NGAT is not active | PASS | Workflow introduction, Delegation Model, Required Gates; clarification section 3. |
| Subagents/child threads are permitted | PASS | Workflow execution owner and Delegation Model; target `DEC-A-010`. |
| They remain subordinate | PASS | They cannot approve phases, decide unresolved product questions, publish/deploy, or cross gates. |
| Current Codex remains accountable | PASS | Codex defines scope/context, inspects results, reconciles conflicts, validates changes, and persists integrated conclusions. |
| No mechanical task explosion | PASS | Clarification delegation guardrails explicitly prohibit recreating the earlier pattern. |
| Joint review remains here | PASS | User + Codex retain every named gate and final acceptance in this conversation. |

## 5. P2 all-and-only verification

| Required property | Result | Evidence |
|---|---|---|
| P2 reason is explicit | PASS | Workflow `Why P2 exists` and clarification section 4 identify immature workflow/tooling/requirements and resulting drift. |
| Every old and extant surface is audited | PASS | Workflow P2 row and clarification 5.1 require P1 inventory, local source/releases, actual remote GitHub, UI/API/WS/config/docs/tests/startup/package/runtime, and history. |
| Complete disposition vocabulary exists | PASS | `KEEP`, `RECOVER`, `REPAIR/REDESIGN`, `REMOVE`, `DEFER`, `USER DECISION REQUIRED`. |
| "All" is end-to-end | PASS | Retained/recovered/redesigned capabilities require reachable user outcomes plus verification and documentation obligations. |
| "Only" is not cosmetic hiding | PASS | Removed/deferred behavior must leave UI, routes, workers, config, docs, tests, startup, packaging, and runtime claims. |
| Removal protects healthy systems | PASS | P2 requires a dependency-aware removal/retention map and preserves shared dependencies used by retained capabilities. |
| New decisions override obsolete implementations | PASS | Email/SendGrid/mail config cannot return merely because old code is substantial; Rust/browser-audio decisions control. |
| Nothing is silently unclassified | PASS | Every inventory row and discovered release-visible surface must receive exactly one disposition. |
| P2 remains non-implementation | PASS | Workflow and clarification assign implementation planning to P3-P6 and execution to approved P7. |

This is the formal engineering equivalent of the User's root-canal analogy:
remove the obsolete surface and its dedicated supporting chain completely,
while preserving the shared structures required by the healthy product.

## 6. Existing inventory integrity

| Machine check | Result |
|---|---|
| `LEG-A` rows | 159 rows, 159 unique, contiguous `001`-`159`, no gaps or duplicates. |
| `LC-E` register | 34 definitions, all referenced IDs resolve, no undefined ID. |
| Target grouping | 22 `A-LEG-*` groups cover all 159 inventory rows exactly once. |
| Markdown tables | Consistent delimiters across controlling and P1 recovery documents. |
| Local links | All Markdown document links resolve after this revalidation file is present. |

## 7. Artifact snapshot

| Artifact | Lines | SHA-256 at revalidation |
|---|---:|---|
| `docs/dual-delivery-workflow.md` | 217 | `8aa54e22676b8f6b873b2e509546305d146cd9d61ecd2e67fe6e558f0fe5073a` |
| `docs/deliverable-a-windows-local-release-requirements.md` | 377 | `3bc42fdd3ed42ad65fa685d98ea862f0fb4a9c8fab0a6cc2236b80418b83e07a` |
| `docs/p1-a-recovery/09-p1-reopen-legacy-capability-gap.md` | 92 | `d82856fdd6e96f2947360a2491db3f2564810a7e0d2994d65d1ec2ce96eca6f3` |
| `docs/p1-a-recovery/10-legacy-capability-inventory.md` | 319 | `69fa3fe62316a096f70c1d3f22bde2eaca33655a08a2a98c72525a0e87e589c3` |
| `docs/p1-a-recovery/12-p1-p2-purpose-and-delegation-clarification.md` | 209 | `d22faf9b7944b404bff0477aff11ac7e2bcb9c8cdb07b613c733d8e43e4cd8b5` |

The superseded `11` file is intentionally excluded from the controlling
snapshot because it records the pre-clarification state.

## 8. Stop-gate verdict

The corrected P1 package passes the scope/provenance revalidation and now
matches the User's four clarifications. It remains a candidate at joint P1
Review, not an approved P2 input. P2 has not started, and no product source,
test, package, server, credential, release, or remote branch was changed by
this correction.
