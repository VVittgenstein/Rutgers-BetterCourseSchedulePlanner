# P1/P2 Purpose and Delegation Clarification

> **CURRENT USER CLARIFICATION - CONTROLS THE WORKFLOW**

- Clarification date: 2026-07-12
- Scope: why P1 exists, what P1 must merge, what delegation is allowed, and what P2 `all-and-only` means
- Workflow effect: no NGAT; one authoritative conversation may use subordinate agents or child threads
- Phase state: P1 candidate under joint review; P2 has not started

## 1. Historical trigger for P1

P1 exists because the User returned to BCSP after a long interval and no
longer reliably remembered the original product intent, feature set, design,
or the reasons behind earlier implementation choices. The project nevertheless
retained unusually rich evidence:

- source code, tests, documentation, architecture notes, task registries, and
  release archives in the local project;
- recovered main-machine indexes and persistent Codex/Claude history;
- local Git objects, branches, commits, and the cleaned public GitHub history;
- the abandoned `task-015` line and its wider Phase 1 plan.

The old `task-015` was not itself the product goal. It was the first gate in
the approved `task-015` through `task-025` Phase 1 sequence. The immutable plan
states that Phase 1 was a feature-complete local release: recover the intended
local product, repair a working-but-rough implementation, and remove release
surfaces that should not exist. Its first task was a release-surface matrix
using `complete`, `recover`, `repair`, `remove`, `defer`, and `unclear`.

Therefore P1 is a **memory and intent reconstruction phase**, not a generic
documentation pass and not merely a summary of the newest conversation.

## 2. P1 has two required input layers

P1 must merge, not choose between, two layers:

### 2.1 Historical layer

Recover what the old project was intended to be and what it actually became:

- original descriptions, goals, constraints, non-goals, product flows,
  features, UX ideas, and quality expectations;
- code, tests, architecture, release contents, removed Git state, stubs,
  abandoned work, and contradictory documentation/configuration;
- why behavior was kept, skipped, disabled, removed, or left incomplete where
  the evidence can establish that chronology.

### 2.2 Current discussion layer

Merge every later accepted correction and expansion, including:

- A Windows-local package plus B public website as the two deliverables;
- the shared React WebUI and shared Rust modular-monolith direction;
- separate Windows-local and Linux-public composition roots;
- current SQLite, centralized Rutgers polling, and WebSocket direction;
- removal of all v1 mail configuration/delivery surfaces;
- current active-watch, nine-section, every-fresh-Open, audio-control, refresh,
  and latency rules;
- the mandatory P7.2 UI design/implementation and separate P7.3 UI polish
  subphases and their named skills.

### 2.3 Merge rule

P1 preserves chronology and applies evidence authority:

1. a later direct User decision supersedes a conflicting old requirement for
   the current target;
2. the superseded requirement remains visible as history, rather than being
   deleted from memory;
3. old intent without a later conflict remains a P2 candidate;
4. current code, UI, docs, routes, or packages prove existence/state, not
   product membership;
5. an unmerged matrix or archived AI narrative is a discovery source, not
   accepted truth; and
6. unresolved contradictions remain explicit for User judgment or P2.

The P1 output is consequently a **merged, provenance-backed target baseline**:
current accepted constraints plus a complete historical capability inventory,
chronology, conflicts, and known drift. P1 does not itself decide the final
all-and-only release surface.

## 3. Delegation model

The NGAT execution line is retired. That does not prohibit delegation.

The current Codex may create and supervise subagents or child threads when
bounded parallel work would improve coverage, implementation speed, or
independent validation. These are subordinate tools of this one execution
line, not a second authoritative line.

| Actor | Authority |
|---|---|
| User + current Codex conversation | Own product decisions, phase review, approval, and the authoritative record. |
| Current Codex | Own decomposition, dispatch, context supplied to subagents, integration, verification, and final accountability. |
| Subagent or child thread dispatched by current Codex | Perform a bounded research, audit, implementation, or validation assignment and return evidence/results to current Codex. |
| NGAT/Organ | No active role unless the User explicitly changes this decision later. |

Delegation guardrails:

- A subagent cannot approve a phase, make an unresolved product decision, or
  cross a joint-review gate.
- Current Codex must inspect and validate delegated output before incorporating
  it; an agent's completion claim is not acceptance.
- Durable conclusions must be persisted in the project, with provenance.
- Delegation must follow meaningful ownership boundaries and must not recreate
  the earlier mechanical task explosion.
- The User reviews the integrated result in this conversation, not a collection
  of unmediated child-thread outputs.

## 4. Why P2 exists

The old development process left structural uncertainty because requirements,
architecture, implementation, and agent workflow evolved at different speeds.
Earlier work was affected by less mature development practice, less capable or
less reliably used agent tooling, incomplete review, and requirements that were
not yet precise. Some old targets were later deliberately removed, such as the
server/mail-oriented notification model, while useful product behavior may
have been lost because implementation was difficult.

P2 exists to turn P1's recovered memory into a definitive product boundary. It
must not treat either old intent or current code as automatically correct.

## 5. P2 all-and-only standard

The User's rule is:

- **All:** every capability that belongs to the accepted A target is present,
  end-to-end real, reachable, integrated, testable, verifiable, and documented.
- **Only:** everything that does not belong is absent. Obsolete, false,
  superseded, unsafe, duplicate, stubbed, misleading, or deferred surfaces do
  not remain visible or packaged as if supported.

The User compared this to root-canal treatment. The engineering translation is
that removal must be complete but dependency-aware: remove the obsolete product
surface and its dedicated supporting chain, while preserving shared healthy
components and the operation of every retained capability. Merely hiding a
button while leaving a public route, config, worker, docs, package entry, or
runtime startup path is not sufficient.

### 5.1 Mandatory P2 evidence inputs

P2 must reconcile at least:

- the jointly accepted P1 merged target and all `LEG-A` inventory rows;
- current local source, tests, UI, routes, configs, scripts, docs, and runtime
  composition;
- every local historical release artifact;
- current local Git objects/history and the actual remote GitHub surface and
  history at audit time;
- local historical/recovery records, including task-015 lineage; and
- the accepted 0A, 0B, 0C, workflow, Rust, and UI decisions.

### 5.2 Mandatory P2 dispositions

Every recovered capability and every extant product/release surface must receive
one explicit disposition:

| Disposition | P2 meaning |
|---|---|
| `KEEP` | Belongs and is already conceptually correct; later work verifies and completes release quality. |
| `RECOVER` | Belongs, but intended behavior was lost, skipped, disabled, or abandoned. |
| `REPAIR/REDESIGN` | Belongs, but the present contract, implementation, UX, architecture, or integration is wrong or incomplete. |
| `REMOVE` | Does not belong; remove the surface and its dedicated dependency chain without damaging shared retained systems. |
| `DEFER` | Valid later work, but absent from the current shipped claim and ordinary-user surface. |
| `USER DECISION REQUIRED` | Evidence cannot safely resolve membership; P2 stops the dependent decision rather than guessing. |

### 5.3 P2 closure invariants

P2 is complete only when:

1. every applicable `LEG-A` row maps to exactly one disposition;
2. every current UI control/view, API/WS route, worker, config key/file, script,
   startup entry, document claim, test contract, package entry, and release
   asset maps to a disposition;
3. every `KEEP`, `RECOVER`, or `REPAIR/REDESIGN` item has a defined end-to-end
   user outcome and verification obligation;
4. every `REMOVE` item has a dependency-aware removal boundary across source,
   UI, routes, config, docs, tests, startup, packaging, and runtime artifacts;
5. every `DEFER` item is absent from current release claims and cannot masquerade
   as a working feature;
6. superseded targets such as v1 email/SendGrid/mail configuration are not
   revived merely because substantial old code exists;
7. shared code needed by retained capabilities is not accidentally removed with
   an obsolete surface;
8. contradictions and unknowns have no silent default; and
9. there is no unclassified user-visible or release-visible surface.

P2 produces the decision record and dependency-aware product-surface audit. It
does not implement those decisions. P3-P7 plan and execute the approved result.

## 6. Evidence anchors

| Evidence | Exact locator | Contribution |
|---|---|---|
| July 10 conversation export | `chat-log-codex-2026-07-10-1ce70862.md`, lines 60-86 | Recovers the whole abandoned task-015-to-025 round and its feature-complete local-release purpose. |
| July 10 direct A restatement | Same file, lines 90-93 | Preserves the User's requirement to recover the old wording and make the release real/honest. |
| Original P1/P2 phase definition | Same file, lines 203-225, especially lines 213 and 215 | Defines P1 as read-only A-intent recovery and P2 as local/release/GitHub/history all-and-only adjudication. |
| Conversation export identity | Same file, 1,277 lines, SHA-256 `76fee2f09567da0afde6fe9f36048c098b6f09f363ba7ad18b8abe5ea8387562` | Fixes the reviewed transcript version. |
| Approved historical Phase 1 plan | Commit `0a61028c91a93906758d41120fd9544ae889cbc7`, blob `943965e84fa56d58cd2c3e28492973d487a86e65`, lines 3-19 and 21-73 | Establishes product boundary rule, end-to-end flow, statuses, task-015 gate, and task-015-to-025 sequence. |
| Historical Phase 1 goals | Commit `0a61028c91a93906758d41120fd9544ae889cbc7`, blob `ebd9fc16252887b08a30607f3cb8ea42f60aab86`, lines 79-115 | Explains earlier tooling/review distortion and the complete, recover/repair/remove release intent. |
| Unmerged task-015 matrix | Commit `5714a8f19481d22691ba799992609e6a5f619d02`, blob `4e0f3b04c0499edabdb4c884517fd4ed9d366770`, especially sections 0-6 | Useful product-surface discovery and chronology, but not an accepted final decision because its review/provenance gate failed. |
| Current direct clarification | User message in the active conversation on 2026-07-12 | Controls the old-plus-new P1 merge, subordinate-agent allowance, and dependency-aware P2 all-and-only standard. |

## 7. Gate effect

This clarification is part of the current P1 Review. It corrects the workflow
and interpretation but does not approve P1 or begin P2. The controlling P1
candidate and workflow must incorporate it, then pass a fresh validation before
returning to the User at the same joint-review gate.
