# Dual Delivery Workflow

- Status: accepted operating workflow
- Created: 2026-07-07
- Last revised: 2026-07-12
- Project: Rutgers BetterCourseSchedulePlanner

This document records the agreed workflow for delivering two final artifacts:

- **A. Windows local release package**: a package that a normal Windows user can download, unpack, and start with a `.bat` launcher.
- **B. Public web deployment**: a website for external users, especially Rutgers students in the United States, that provides the core BCSP experience without local installation.

"Dual delivery" refers to the two final artifacts, not to two execution lines.
The project now uses this current Codex conversation as its one authoritative
working line. NGAT is not part of the active workflow. Codex may delegate
bounded work to its own subagents or child threads, but those workers remain
subordinate to this conversation and do not become a second authority line.

## Operating Model

| Working mode | Owner | Responsibility |
|---|---|---|
| Execution | Codex in this conversation, with optional bounded subagents/child threads | Read-only recovery, source and history audits, persisted plans, implementation, validation, commits, packaging, and approved GitHub Release publication. Codex remains responsible for every delegated result. |
| Review / discussion | User + Codex in this conversation | Clarify goals, examine execution results, resolve product questions, approve phase gates, make release/deployment decisions, and perform final acceptance. |

The work stays in one conversation. In execution mode Codex performs the
bounded phase, persists evidence and outputs locally, validates its work, and
then stops at the phase's review gate. In review / discussion mode the User and
Codex inspect those outputs and decide whether to correct, repeat, approve, or
advance. Codex must not silently cross a required review gate.

## Delegation Model

Subagents and child threads are execution tools, not independent project lines:

- Codex may delegate narrowly scoped source reading, research, design studies,
  implementation units, or independent validation when parallel work improves
  completeness or confidence.
- Codex must define each delegated scope, inspect the returned evidence or
  changes, reconcile conflicts, and persist only reviewed results.
- A subagent or child thread cannot approve a phase, make unresolved product
  decisions for the User, cross a review gate, publish a release, deploy to
  production, or become the authoritative holder of project memory.
- Delegated work must return to this conversation. The User and Codex perform
  every named review and approval here.
- NGAT remains historical evidence only; allowing Codex subagents does not
  reactivate NGAT or the retired dual-line ownership model.

## Complete Flow

| Phase | Working mode | Purpose | Output / Stop Point |
|---|---|---|---|
| Preflight 0A: Define B | Review / discussion | Define what the public website should provide, which capabilities must match A, and which capabilities must change for the public web context. | Persisted B target document. |
| Preflight 0B: Choose Deployment Platform | Review / discussion | Compare Oracle Cloud / OCI Always Free, Cloudflare Pages + Workers + D1/R2, Google Cloud Run, and any better-fit alternatives. Decide whether registration, purchase, SSH keys, API tokens, or other credentials are required. | Accepted deployment decision document. A target VM may be provisioned early, but no unaudited application is deployed. |
| Preflight 0C: Choose Shared A/B Architecture | Review / discussion | Decide whether A and B share a backend, define the high-level runtime and composition boundaries, and prevent local-only surfaces from leaking into B. | Accepted architecture decision record; exact implementation decomposition remains P3-P5 work. |
| P1: Recover and Merge A Product Memory | Execution | Reconstruct the old product intent that became difficult to remember after the abandoned `task-015` line and a long project pause. Read only from current/recovered trees, local releases, code, tests, architecture, docs, Git/GitHub history, and conversation records. Then merge that historical layer with later accepted decisions, including A/B delivery, Rust/shared architecture, current watch/audio behavior, deployment boundaries, and UI-phase requirements. Preserve source authority and contradictions; inventory is not automatic acceptance. | Codex persists a source-layered merged A candidate and complete behavior inventory, validates coverage, then stops at P1 Review. |
| P1 Review | Review / discussion | Check whether the merged candidate accurately reconstructs what the User originally intended and correctly incorporates the newer decisions without erasing chronology. Correct and finalize it here. | Accepted merged A product-memory/target input for P2. |
| P2: A All-and-Only Product Surface Audit | Execution | Re-perform the conceptual job of the abandoned `task-015` feature-matrix gate with stronger evidence and the accepted merged P1 target. Audit the local project, old/current release artifacts, actual remote GitHub surface/history, code, UI, routes, docs, config, tests, startup, packaging, and runtime composition. Decide every surface so all required capabilities are eventually end-to-end usable, verifiable, and documentable, while obsolete, misleading, stubbed, unsafe, or out-of-scope surfaces and their dedicated support chains do not remain in v1. Removal boundaries must preserve shared healthy systems. | Exhaustive disposition matrix, dependency-aware removal/retention map, and no unclassified release-visible surface, then a review stop. |
| P3: A Implementation Plan | Execution | Design the complete implementation plan for the Windows local release package. | A implementation plan candidate, then a review stop. |
| P4: B Implementation Plan | Execution | Design the complete implementation plan for the public web deployment, based on the B target and deployment decision. | B implementation plan candidate, then a review stop. |
| P5: A/B Reuse and Split Analysis | Execution | Treat A and B as a Venn diagram. Classify shared core, A-only work, B-only work, conflicts, adapters, and code that must split. | A/B matrix and local architecture split conclusion, then a review stop. |
| P6: Final Execution Plan Merge | Execution | Merge the P5 conclusions back into the A and B implementation plans. | Final execution-plan candidate, then P6 Review. |
| P6 Review | Review / discussion | Perform pre-execution audit: scope, risk, commit strategy, UI strategy, release strategy, domain/HTTPS readiness, deployment package strategy, and final user approval. | User approval gate for all P7 implementation and release subphases. |
| P7.1: Functional Implementation | Execution | Decompose and execute the approved final plan for the shared core, the Windows A entry point, the public B entry point, APIs, data paths, and the functional foundation consumed by the shared WebUI. This subphase must not claim final UI completion. | Integrated functional candidate with clean remote commits per task, then review. |
| P7.2: UI Design and Implementation | Execution | Mandatory independent UI subphase. Explicitly use `$industrial-brutalist-ui` and `$design-taste-frontend` to design and implement the one shared A/B WebUI for desktop and mobile, including its loading, empty, error, and interactive states. | Implemented and visually verified UI baseline, persisted design decisions, its own clean remote commit(s), then review. |
| P7.3: UI Polish | Execution | Mandatory independent polish subphase. Only after P7.2 is approved, explicitly use `$emil-design-eng` to audit and refine the real integrated UI, including interaction details, motion, perceived responsiveness, touch/keyboard behavior, reduced motion, accessibility, and frontend performance. | `Before \| After \| Why` audit, applied polish, visual and interaction evidence, separate clean remote commit(s), then review. |
| P7.4: Integration, Validation, and Packaging | Execution | Integrate both delivery paths, run the approved functional, contract, browser, package, security, and capacity checks, then build and audit the two deliverables. | A Windows release package and B Linux deployment package, with required documentation and checksums, then review. |
| P7 Release | Execution after approval | If feasible, safe, and included in the P6 approval, publish the audited A release package and B deployment package as GitHub Release assets. | Optional GitHub Release assets, then final release review. |
| Deployment Guidance | Review / discussion with Codex execution | Guide the User through hardening the selected machine, deploying the exact B deployment package, configuring domain/HTTPS, and verifying the public website. | Public B website deployed and verified with the User. |

All `P7.x` rows are mandatory subphases of P7. They preserve the existing
P1-P7 phase numbering and must not be promoted into new P8/P9 phases.
`P7 Release` remains conditional, as originally agreed; production deployment
remains outside P7 and happens only in the later guided deployment step.

## P1 And P2 Contract

### Why P1 exists

The abandoned 2026 `task-015` line was the first gate in an older
`task-015`-`task-025` plan for a feature-complete local release. It attempted a
release-surface matrix, but the line was never accepted or completed. After a
long pause, the User no longer reliably remembered every original requirement,
feature, or design choice. P1 therefore exists to rebuild trustworthy product
memory from persistent evidence before any new all-and-only judgment.

P1 is not legacy-only. Its output is the explicit merge of:

1. **Historical layer:** original User intent, requirements, code, tests,
   architecture, abandoned/removed behavior, release packages, Git/GitHub
   history, and local recovery records.
2. **Current layer:** later direct discussion and accepted decisions, including
   the Windows A/public B split, shared React and Rust architecture, local and
   public composition boundaries, WebSocket/live-watch rules, browser audio,
   refresh behavior, deployment choice, and mandatory UI subphases.

P1 must retain provenance and chronology. A newer accepted decision can
supersede an old goal, such as old server/mail assumptions, but the old goal
must remain visible as history so P2 can explain why it is excluded. P1 does
not decide the final release surface.

### Why P2 exists

The legacy project was built while the User's development workflow, experience,
Agent tooling, and requirements were less mature. Some old goals were vague or
later rejected; some implementations, docs, UI, routes, configs, and packages
drifted apart. P2 is the modern, evidence-complete successor to the old
`task-015` release-surface audit.

For every recovered capability and every discovered current/remote/package
surface, P2 must record one disposition: `KEEP`, `RECOVER`,
`REPAIR/REDESIGN`, `REMOVE`, `DEFER`, or `USER DECISION REQUIRED`. It must also
record source evidence, current/new intent, conflict resolution, rationale,
affected user/UI/API/WS/config/docs/tests/startup/package/runtime surfaces, and
the acceptance/validation obligation for anything retained or restored.

P2 reaches the all-and-only gate only when:

- every P1 inventory item and every discovered code/UI/API/docs/config/package
  surface has a disposition; nothing remains silently unclassified;
- every capability required by the accepted present product is included with a
  complete user outcome and later verification/documentation obligation;
- every `REMOVE` or `DEFER` item is absent from the v1 user surface, runtime
  contract, configuration, documentation, and release package, while future
  work may be tracked separately and honestly;
- every `REMOVE` item has a dependency-aware removal boundary that removes its
  dedicated code, route, worker, config, docs, tests, startup, and packaging
  chain without removing shared components needed by retained behavior;
- stubs, fake UI, fake docs, dead entry points, orphan controls, obsolete
  provider settings, unsafe defaults, and stale release claims cannot survive
  merely because files exist; and
- conflicts such as old hosted-server/email goals versus newer local/shared
  Rust/browser-audio decisions are resolved explicitly using the accepted
  current intent, not silently or by whichever implementation happens to exist.

P2 defines the product surface. It does not write the P3 implementation plan or
perform P7 implementation.

## Mandatory P7 UI Separation

P7.2 and P7.3 are two independent execution subphases, not two labels for one
UI task:

- P7.2 must explicitly use both `$industrial-brutalist-ui` and
  `$design-taste-frontend`. It establishes the visual system and implements the
  complete shared A/B interface. The industrial skill's visual archetypes must
  be resolved into one coherent direction rather than mixed across screens.
- P7.3 must explicitly use `$emil-design-eng` against the real, integrated
  result of P7.2. Its review artifact must use the skill's required
  `Before | After | Why` table, and the accepted changes must then be
  implemented and verified.
- P7.3 cannot begin until P7.2 has been implemented, integrated, visually
  verified, and approved at its review stop. P7.2 and P7.3 must have different
  execution records and commit series; one task or commit must not claim both
  subphases.
- Both subphases operate on the one shared React WebUI. A and B must not acquire
  separate ordinary-user visual implementations.

## P7 Completion Boundary

P7 implements, verifies, commits, and packages the approved product. Its
mandatory final artifacts are the A Windows release package and B Linux
deployment package.

- GitHub Release publication is performed by Codex only when feasible, safe,
  and approved at P6.
- Codex does not deploy B to Vultr as part of P7.
- After P7 and the optional P7 Release step, the same conversation enters the
  guided deployment step, where Codex and the User use the audited B package.
- Credentials, server inventory, SSH material, provider data, and runtime
  secrets remain outside Git, release assets, and public logs.

## Current Preflight Status

- 0A is persisted in `docs/public-web-target.md`.
- 0B is accepted in `docs/deployment-platform-decision.md`; Vultr EWR is the B
  v1 target and the baseline VM has been provisioned.
- 0C is accepted in `docs/shared-rust-architecture-decision.md`.
- Provisioning the VM early does not authorize application deployment or skip
  P1-P7.
- Earlier NGAT P1 artifacts are retained as historical evidence, but NGAT is no
  longer an active owner or execution line.
- P1 was reopened on 2026-07-12 because the first candidate summarized
  "search/filtering" without recovering the old project's complete feature and
  behavior inventory. The behavior-level remediation and its single-line
  validation are now persisted in `docs/p1-a-recovery/10-legacy-capability-inventory.md`
  and `docs/p1-a-recovery/11-single-line-p1-validation.md`.
- P1 joint review is now open. Round-by-round rulings are recorded in
  `docs/p1-a-recovery/14-joint-p1-review.md`. P2 has not started.
- The joint review further clarified the `task-015` origin, P1's old-plus-new
  merge requirement, permitted Codex subagent delegation, and P2's exact
  all-and-only completion rule. These corrections are recorded in
  `docs/p1-a-recovery/12-p1-p2-purpose-and-delegation-clarification.md`.
- The corrected P1 candidate and workflow are checked in
  `docs/p1-a-recovery/13-single-line-p1-revalidation.md`.

## Required Gates

- P1 execution must stop for joint review before A requirements are finalized.
- P6 execution must stop for joint review before implementation starts.
- P7.2 must stop at its own completion gate before the separate P7.3 polish
  subphase starts.
- Codex must not make unresolved final product decisions on behalf of the User.
- No NGAT handoff is part of the current operating model. Codex may use bounded
  subagents/child threads under the Delegation Model, but all authority,
  integration, and review return to this conversation.

## Commit and Release Principles

- Every P7 task must produce a clean commit and push it to the approved remote branch, so the GitHub project history and contribution graph reflect the work. P7.2 and P7.3 must have distinct commit histories.
- Commit quality, public cleanliness, and safety take priority over contribution graph appearance.
- Historical NGAT state, `.ngagent` runtime artifacts, private history, local-only config, secrets, SSH keys, API tokens, and service credentials must not be committed or packaged.
- The A package and B deployment package may be attached to the same GitHub Release only after secret scanning and artifact audit.
- Later deployment guidance must use the audited B package produced by P7 rather than rebuilding a different production artifact.

## Credential Boundary

Deployment credentials such as SSH keys, API tokens, service account keys, cloud provider tokens, and email provider credentials must stay outside tracked Git content and outside release artifacts. For this project they may be stored in the anchored, ignored `.secrets/` directory inside the local checkout, another user-controlled secret location, or the target platform's secret manager / environment variable system. Project-local `.secrets/` content must never be force-added, copied into temporary worktrees, committed, packaged, or published.
