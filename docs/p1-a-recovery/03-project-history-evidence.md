# Deliverable A Project and Git History Evidence

Status: complete project/Git evidence pass for fresh P1; evidence only, not a
canonical requirements specification

Scope: Deliverable A (the local release) as described by legitimate tracked
project content, admissible Git history, and the seven hash-pinned external
project-history files declared for task-062

Schema authority: `docs/p1-a-recovery/01-source-register.md`, especially
Sections 5, 8, and 9

Evidence-ID prefix: `PH-E`

## 1. Boundary and method

This artifact records what the project-history sources say or demonstrate. It
does **not** decide which historical features belong in A, does not perform the
later all-and-only product adjudication, and does not promote implementation
state into user intent. A direct historical user statement is classified
`DIRECT_USER`, but it remains older L2 primary history; the current discussion
line and accepted current decisions have higher authority.

The pass used these controls:

1. All seven declared external files were raw-byte SHA-256 verified before use
   and read in full. Their observed hashes matched their task pins.
2. The exported May chat was read end to end (lines 1-1181). The raw Codex
   session path mentioned in its header was not opened.
3. Git objects were limited to legitimate history at or before the
   uncontaminated pause commit
   `8004637c47e40ee3417b4d74d898124bd4b975f0`, except that the explicitly
   admitted side commit
   `5714a8f19481d22691ba799992609e6a5f619d02` was read only as failed-review
   candidate history.
4. Before a current tracked product file was cited, its last-changing commit
   was checked. Every cited last-changing commit is an ancestor of `8004637…`,
   and every cited current blob is byte-identical to its blob at that pause.
5. No claim uses the archived runaway-P1 ancestry slice `efe8fd6…b6d6a65`,
   commit `c6652a33d9996b731de9e400a0bce644ec207ffe`, or any later old-P1 planning,
   ledger, task, review, or merge. No blob from that lineage was opened. The
   prohibited old root requirements document was not opened, diffed, or used.
6. No `.ngagent/archives/**`, raw `.codex`/`.claude` history, private config,
   secret material, or arbitrary parent-checkout file was read. The seven
   declared parent files remained read-only.

## 2. Declared external project-history inventory

`Observed SHA-256` means the recomputed raw-byte digest, not a normalized-text
digest. A matching source can still be only a summary or provenance aid.

| Source ID | Exact external path | Observed SHA-256 | Full-read and useful locators | Permitted evidentiary role |
|---|---|---|---|---|
| `PROJECT-DIGEST` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\DIGEST.md` | `ef7f3c3e462e6be676638a9347441bce0d769968ac19062081f9b3a6f44fd60d` | Whole file, lines 1-24; Phase 1 savepoint summary at lines 16-24 | L3 project-memory summary and pointer only; no independent A requirement extracted. |
| `PROJECT-TIMELINE` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\TIMELINE.md` | `e7bde98c21231f793c97e21eb6a9c23977b3524abb53ee62faf539eaad3f4525` | Whole file, lines 1-320; release reconciliation at lines 27-41; cleanup/public history at lines 117-269; memory/savepoint state at lines 273-320 | L3 chronological summary. Useful for locating legitimate Git work, not for converting a task summary into intent. |
| `PROJECT-MAY-CHAT` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\chat-log-2026-05-12T06-12Z.md` | `af793b4a0ab6917ecea62dc0ea6eb7f239bc7e062b5ffd72175d037b98d9daa9` | Whole file, lines 1-1181; exact user turns are cited per evidence row below | L2 historical transcript. Exact `用户` turns may support `DIRECT_USER`; Codex turns remain assistant analysis or historical planning. |
| `PROJECT-REGISTRY` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\registry.json` | `bdc179b8f11ee3472747c614f9b758f24fe6c75e4acedf8c8845459b20c3c3ac` | Whole file, lines 1-21; `/sessions/0`, `/sessions/1`, `/chat_logs` | Provenance/index metadata only. It registers the two memory sessions and contains no A requirement. |
| `PROJECT-SESSIONS-README` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\sessions\README.md` | `dfa667b4a9947632b1e2f8da400a60ffab176be6037dca5824496b9befc08f61` | Whole file, lines 1-92; preservation policy at lines 1-23; template at lines 30-92 | Memory-governance metadata only. It explains how summaries were maintained, not what A must do. |
| `PROJECT-SESSION-CLOSEOUT` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\sessions\2026-05-12-public-remote-closeout-savepoint.md` | `c5996b9048e3a9f51d2edc495caa1d41cacc452f95a6c77e9478b059d2dcd8fb` | Whole file, lines 1-107; purpose/result at lines 6-9; summarized dialogue at lines 11-44; release state at lines 80-98 | L3 session summary. It corroborates public/release history but cannot substitute for the exact user turns in `PROJECT-MAY-CHAT`. |
| `PROJECT-SESSION-INIT` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\sessions\2026-05-11-pm-init-setup.md` | `8f6d927116eb587d8236b9db685017f1db6590b177183b38e36d4b89b608c47d` | Whole file, lines 1-108; goal/output at lines 6-46; decision log at lines 48-56; result at lines 73-100 | Memory-system history and provenance only. It establishes how later summaries were organized, not A product intent. |

## 3. Tracked-file and immutable Git inventory

### 3.1 Relevant current tracked product surfaces

The identities below are the full last-changing commit plus repository-relative
blob path. These commits all precede and are ancestors of `8004637…`; the
current task worktree has the same blob at every listed path.

| Tracked surface | Immutable locator(s) | What it may establish |
|---|---|---|
| End-user overview and startup/notification instructions | `71f2a2b6d6a5afab234f1686965f657af5c3652c:README.md`, lines 113-211 | Current/historical documented behavior only: release download, Node installation, fetch/filter flow, SendGrid, and local sound. |
| Windows launcher | `f6b10e2288214962265d4dc43304c87ba6d633b7:Start-WebUI.bat`, lines 1-20 | The tracked Windows startup preflight and its Node dependency. |
| macOS launcher | `f6b10e2288214962265d4dc43304c87ba6d633b7:Start-WebUI.command`, lines 1-16 | A tracked macOS-oriented entry exists; existence is not proof of successful macOS validation. |
| One-click orchestration | `71f2a2b6d6a5afab234f1686965f657af5c3652c:scripts/oneclick_start.js`, lines 110-118 and 269-365 | Node-version, dependency-install, native-module, and fallback behavior. |
| Package/runtime manifest | `d24bfa56c3f10571983850bf21662097e1b10ff3:package.json`, lines 10-46 | npm command surface and Node/native dependencies; implementation state, not distribution intent. |
| Section endpoint implementation | `b297cb23a2a3c9b6304758328b764bb8dfe5ed29:api/src/routes/sections.ts`, lines 70-115 | The route has a response schema but returns an empty result. |
| Section endpoint documentation | `9283ba1a821f6126659363e40957ed01e2c806e9:docs/query_api_contract.md`, lines 152-185 | A tracked contract describes detailed section rows despite the empty handler. |
| Mounted frontend shell | `5c526aece8bd1d77d46dc225f0adfb53d2d7f143:frontend/src/App.tsx`, lines 70-126 | Current mounted fetch, filter, subscription, list, mail-settings, and management surfaces. |
| Current course-list behavior | `60a45a4ae85f2d428389cffc4ec0e06aae09161b:frontend/src/components/CourseList.tsx`, lines 44-122 | Current loading/error/empty/list/pagination behavior. |
| Older UI design document | `e4bc1554e0b981edc0ee65c807bedcc009d9468e:docs/ui_flow_course_list.md`, lines 110-128 | Historical design ideas (calendar, section drawer, presets); planning evidence only. |
| Browser-local audio implementation | `71f2a2b6d6a5afab234f1686965f657af5c3652c:frontend/src/hooks/useLocalSoundNotifications.ts`, lines 68-84 and 111-175 | Current polling/audio behavior; it cannot prove that behavior is the intended A contract. |
| Subscription documentation | `9283ba1a821f6126659363e40957ed01e2c806e9:docs/subscription_model.md`, lines 20-35 | A stale email-only local-mode description, useful as drift evidence. |

### 3.2 Admissible planning, release, and execution-state objects

| Git object | Exact path/locator | Treatment in this artifact |
|---|---|---|
| `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` | Registered public-baseline tree; exact anchors include `README.md` blob `7763643f0785068413e865aeeb812370bdd6e370` and `Start-WebUI.bat` blob `ed9c48647b9c674b49db8a0e4505d7faaed95810` | Public project-state anchor only. It does not establish user intent. |
| `296107c840813f7d4baacb8e17efb81e4f9d0256` | `.orchestrator/stage-a/02-release-reconciliation.md`, lines 29-31, 67-98, 266-302 | Contemporaneous release-pack audit. It records archive drift; the archives themselves were not declared to this task and were not reopened. |
| `b0289e3a5332d836c752547d79a7d9120677e944` | `.orchestrator/stage-p/07-public-tags-releases-closeout.md`, lines 4-6 and 23-28 | Historical release/tag inventory and closeout observation. |
| `0a61028c91a93906758d41120fd9544ae889cbc7` | `.orchestrator/phase-1/00-plan.md`, lines 3-33 and 63-73; `.orchestrator/goals.md`, lines 79-115; `.orchestrator/architecture.md`, lines 137-189 | L3 old plan. Useful as a lead and record of the assistant-authored planning state, never as a current decision by itself. |
| `342e4502a3466c3e88d1291e5ffff2754e1acc30` | `.orchestrator/context_manifest.md`, task-015 collision entry later superseded by the pause record | Historical execution-state object only; no product requirement extracted. |
| `5714a8f19481d22691ba799992609e6a5f619d02` | `.orchestrator/phase-1/01-release-surface-feature-matrix.md`, lines 1-152 | Explicitly admitted L3 candidate history only. It was never accepted by a passing provenance review. Its feature classifications are not adopted here. |
| `8004637c47e40ee3417b4d74d898124bd4b975f0` | `.orchestrator/context_manifest.md`, lines 57-58 | Uncontaminated pause and provenance limit. It records why `5714a8f…` could not be reviewed/merged. |

The historical release-reference set is therefore precise: the admitted Stage
A audit names `release/bcsp-20260121.tar.gz`,
`release/bcsp-20260121.zip`, and root `bcsp-20260122.zip`; the admitted Stage P
closeout records tags `Finalrelease`, `First-release`, `Release`,
`Release-0118`, `Release-0121`, `Release-0122`, `Release-1124`, and
`Second-release`, plus GitHub Release `264993969` (`tag_name=Release-0122`,
display name `Release-0121`) and asset `bcsp-20260122.zip`. These are historical
release observations, not A requirements or currently trusted artifacts.

## 4. Evidence ledger

### 4.1 Exact historical user statements

| Evidence ID | Classification | Source | Locator | Quotation or faithful paraphrase | Relevance to A | Confidence / contradiction notes |
|---|---|---|---|---|---|---|
| `PH-E001` | `DIRECT_USER` | `PROJECT-MAY-CHAT` — `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\chat-log-2026-05-12T06-12Z.md` | Lines 343-347, `turn_index: 40`, user at 07:52Z | Faithful English paraphrase: the user had three goals—first a local tool, second a complete refactor, and third deployment on Google Cloud. | Establishes that the local deliverable was historically a distinct first phase, not the complete refactor or cloud deployment. | High confidence in the historical statement. It is older L2 history; later current-mainline decisions may supersede platform details. |
| `PH-E002` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 442-443, `turn_index: 44`, user at 08:33Z | Faithful English paraphrase: “This time I am going to do Phase 1—a complete BCSP that can be released.” | Direct evidence that the historical local phase aimed at completeness and releasability rather than a demo. | High confidence. “Complete” is not independently a feature list. |
| `PH-E003` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 507-508, `turn_index: 46`, user at 08:35Z | Faithful English paraphrase: broaden the mandatory-inclusion statement; all functions should be present. | Records the initial completeness formulation for A. | High confidence, but incomplete alone. The user refined it in `PH-E008` and `PH-E009`; it must not be read as “ship every historical idea.” |
| `PH-E004` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 556-559, `turn_index: 48`, user at 08:41Z | Faithful English paraphrase: the user then had only Windows, wanted Windows and macOS to work without distribution problems, and expected any person to unpack the package and use it directly. | Direct historical evidence for an ordinary-user, unpack-and-use distribution expectation. | High confidence as historical intent. It does not prove that macOS was validated, and current A scope must be resolved by higher-authority current sources. |
| `PH-E005` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 628-630, `turn_index: 50`, user at 08:49Z (repeated at lines 695-697) | Faithful English paraphrase: some things had been cancelled and some skipped, so a present stub must be checked against persistent history before deciding whether it should be implemented. | Requires provenance-sensitive recovery rather than treating every empty implementation as a requirement. | High confidence. It deliberately leaves individual feature membership unresolved. |
| `PH-E006` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 629-630, `turn_index: 50`, user at 08:49Z | Faithful English paraphrase: rebuilding the UI necessarily includes rebuilding the UX. | Direct historical requirement that A’s interface work be more than visual reskinning. | High confidence. It does not specify the final screens or interaction model. |
| `PH-E007` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 741-743, `turn_index: 56`, user at 09:03Z | Faithful English paraphrase: use `gpt-tasteskill` during design/UI-UX work, then use `emil-design-eng` for refinement; do not read either during planning. | Historical process requirement for a two-stage UI/UX design and polish sequence. | High confidence for that historical plan. Skill names or availability may be renamed later; this row does not execute the plan. |
| `PH-E008` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 768-769, `turn_index: 59`, user at 09:07Z | Faithful English paraphrase: “everything should be there” also means that things which should not appear must not be there. | Direct evidence for both positive completeness and absence of inappropriate release surfaces. | High confidence. This task records the rule but intentionally does not adjudicate the all-and-only membership. |
| `PH-E009` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 803-804, `turn_index: 61`, user at 09:11Z | Faithful English paraphrase: do not call it adding new features, but restore things that were already designed and then abandoned because they had problems; the boundary is fuzzy. | Direct evidence that historically intended-but-lost behavior may be recovery rather than feature expansion. | High confidence in the rule; low confidence about which individual historical ideas qualify. |
| `PH-E010` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 836-837, `turn_index: 63`, user at 09:13Z | Faithful English paraphrase: cleanup is not limited to fake or bad surfaces; it also covers things that were written incorrectly. | Extends A’s historical correction goal to erroneous code/contracts/docs, not merely deletion of obvious stubs. | High confidence, but still not a per-file remediation decision. |
| `PH-E011` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 866-873, `turn_index: 65` and `turn_index: 67`, user at 09:15-09:16Z | Faithful English paraphrase: the user wanted the entire workflow to run through ngagent. | Historical execution/governance provenance for the old Phase 1 plan. | High confidence, but this is a process constraint rather than an end-user A capability. |
| `PH-E012` | `DIRECT_USER` | `PROJECT-MAY-CHAT` | Lines 973-976, `turn_index: 81`, user at 09:35Z | The user approved execution of the presented task plan. | Establishes approval to begin the old plan; it helps distinguish plan approval from acceptance of task-015’s later matrix content. | High confidence. This approval did **not** make `5714a8f…` authoritative because its review never passed (`PH-E015`). |

### 4.2 Historical plans, release state, and candidate provenance

| Evidence ID | Classification | Source | Locator | Quotation or faithful paraphrase | Relevance to A | Confidence / contradiction notes |
|---|---|---|---|---|---|---|
| `PH-E013` | `HISTORICAL_SUMMARY` | Commit `0a61028c91a93906758d41120fd9544ae889cbc7`, blob `.orchestrator/phase-1/00-plan.md` | Lines 3-33 and 63-73 | Faithful paraphrase: the old plan interpreted Phase 1 as a complete local release with an unpack/start/fetch/filter/inspect/subscribe/poll/local-notify/manage/recover flow, while excluding cloud deployment and broad refactoring. | Provides a compact lead map for historical A evidence and likely downstream validation areas. | Medium confidence only. It is an assistant-authored old plan, not a direct user message or current accepted decision. |
| `PH-E014` | `HISTORICAL_SUMMARY` | Commit `5714a8f19481d22691ba799992609e6a5f619d02`, blob `.orchestrator/phase-1/01-release-surface-feature-matrix.md` | Lines 1-43 (scope/evidence); lines 46-78 (candidate classifications); lines 120-130 (unresolved product decisions) | Faithful paraphrase: task-015 proposed classifications for startup, packages, APIs, UI, polling, local sound, email, scheduled refresh, and other surfaces, while flagging several human decisions. | Candidate discovery aid for where legitimate evidence may exist. | Low authority. It is explicitly candidate history, unmerged, and never accepted by provenance review. No candidate classification is adopted by this artifact. |
| `PH-E015` | `IMPLEMENTATION_OBSERVATION` | Commit `8004637c47e40ee3417b4d74d898124bd4b975f0`, blob `.orchestrator/context_manifest.md` | Lines 57-58 | The pause record says task-015’s first attempt was cross-project contaminated; attempt 2 produced `5714a8f…` but review was blocked by `execution_turn_count_mismatch`; attempt 3 remained idle; no fourth dispatch was allowed. | Establishes why task-015 cannot be treated as an accepted requirement decision. | High confidence for execution provenance. The record explicitly separates infrastructure failure from product-content judgment, so it neither validates nor disproves matrix content. |
| `PH-E016` | `IMPLEMENTATION_OBSERVATION` | Commit `296107c840813f7d4baacb8e17efb81e4f9d0256`, blob `.orchestrator/stage-a/02-release-reconciliation.md` | Lines 29-31 and 289-302 | Faithful paraphrase: the admitted audit found three historical package forms with differing layout/content and concluded none could be promoted as a canonical release without further work. | Demonstrates historical release drift and the need for a fresh, verified package rather than reuse by filename. | Medium-high confidence in the immutable contemporaneous audit. The underlying archives were not declared for task-062 and were not independently reverified here. |
| `PH-E017` | `IMPLEMENTATION_OBSERVATION` | Commit `296107c840813f7d4baacb8e17efb81e4f9d0256`, blob `.orchestrator/stage-a/02-release-reconciliation.md` | Lines 79-85, 92-98, and 266-269 | Faithful paraphrase: historical packs contained scheduled-fetch/auto-refresh files from an unmerged local branch, while the then-canonical development tree did not; the audit treated this as release-blocking product-surface divergence. | Shows that scheduled refresh is real historical implementation evidence worth investigating under `PH-E009`, but not automatically an A requirement. | Medium-high confidence as implementation history. It proves divergence, not user intent or a recover decision. |
| `PH-E018` | `IMPLEMENTATION_OBSERVATION` | Commit `b0289e3a5332d836c752547d79a7d9120677e944`, blob `.orchestrator/stage-p/07-public-tags-releases-closeout.md` | Lines 4-6 and 23-28 | The historical closeout records eight named tags and GitHub Release `264993969` (`Release-0122` / display `Release-0121`) with asset `bcsp-20260122.zip`, then records their removal from the public surface. | Precisely inventories historical release references while preventing a deleted/stale public artifact from being mistaken for A authority. | High confidence as immutable historical project state; zero product intent is inferred. |

### 4.3 Current implementation and documentation observations

| Evidence ID | Classification | Source | Locator | Quotation or faithful paraphrase | Relevance to A | Confidence / contradiction notes |
|---|---|---|---|---|---|---|
| `PH-E019` | `IMPLEMENTATION_OBSERVATION` | Commit `71f2a2b6d6a5afab234f1686965f657af5c3652c`, blob `README.md` | Lines 113-130 | The tracked English instructions tell a user to download/extract a release, run a one-click script, then download and install Node.js if it is absent. | Establishes the documented historical distribution path against which unpack-and-use intent can be compared. | High confidence for the tracked blob; documentation may be stale and cannot prove desired behavior. |
| `PH-E020` | `IMPLEMENTATION_OBSERVATION` | Commit `f6b10e2288214962265d4dc43304c87ba6d633b7`, blob `Start-WebUI.bat` | Lines 1-13 | The Windows launcher exits if `node` is absent, opens the Node download page, and otherwise runs `scripts\oneclick_start.js`. | Shows that the tracked Windows entry is not self-contained. | High confidence for implementation state; no inference about the final packaging mechanism. |
| `PH-E021` | `IMPLEMENTATION_OBSERVATION` | Commit `71f2a2b6d6a5afab234f1686965f657af5c3652c`, blob `scripts/oneclick_start.js` | Lines 110-118 and 269-365 | The launcher enforces a Node version, runs npm installs when dependencies are absent, rebuilds `better-sqlite3`, and may ask for Microsoft C++ Build Tools if native loading still fails. | Demonstrates concrete clean-machine/distribution failure modes relevant to A’s ordinary-user packaging expectation. | High confidence for code state. It is not a requirement statement and does not prove these prerequisites are acceptable. |
| `PH-E022` | `CONFLICT` | `PROJECT-MAY-CHAT` plus commits `71f2a2b6d6a5afab234f1686965f657af5c3652c` and `f6b10e2288214962265d4dc43304c87ba6d633b7` | `PH-E004` versus `PH-E019`-`PH-E021` | The direct historical expectation was that an ordinary person could unpack and use the package; the tracked path instead requires external Node/npm and can require native build tooling. | Records distribution drift that later A synthesis/review must address. | High-confidence conflict in historical intent versus implementation. This artifact does not choose a packaging solution or adjudicate current macOS scope. |
| `PH-E023` | `IMPLEMENTATION_OBSERVATION` | Commit `b297cb23a2a3c9b6304758328b764bb8dfe5ed29`, blob `api/src/routes/sections.ts` | Lines 70-115 | The route exposes a detailed-looking response schema but hard-codes `total: 0` and returns `data: []`. | Demonstrates an implementation stub/drift surface relevant to course/section behavior. | High confidence for code state. Under `PH-E005`, a stub alone does not prove that standalone section search must ship. |
| `PH-E024` | `HISTORICAL_SUMMARY` | Commit `9283ba1a821f6126659363e40957ed01e2c806e9`, blob `docs/query_api_contract.md` | Lines 152-185 | The tracked contract describes `GET /api/sections` as providing detailed rows for subscription and advanced filtering, with many query fields. | Historical documentation lead for intended section behavior. | Medium confidence as a plan/contract; it conflicts with implementation and is not direct user intent. |
| `PH-E025` | `CONFLICT` | Commits `b297cb23a2a3c9b6304758328b764bb8dfe5ed29` and `9283ba1a821f6126659363e40957ed01e2c806e9` | `PH-E023` versus `PH-E024` | The documented section contract promises detailed data while the route always returns an empty result. | Makes the API/documentation drift explicit without deciding implement-versus-remove-versus-fold. | High confidence that drift exists; current A membership remains unresolved. |
| `PH-E026` | `IMPLEMENTATION_OBSERVATION` | Commit `5c526aece8bd1d77d46dc225f0adfb53d2d7f143`, blob `frontend/src/App.tsx` | Lines 70-126 | The mounted UI composes language selection, data fetch, filtering, subscription entry, a course list, mail settings, and subscription management in a single shell. | Inventories the current user-visible surface that a later UI/UX recovery must compare with direct intent. | High confidence for current composition; existence is not a decision to keep every panel. |
| `PH-E027` | `HISTORICAL_SUMMARY` | Commit `e4bc1554e0b981edc0ee65c807bedcc009d9468e`, blob `docs/ui_flow_course_list.md` | Lines 110-128 | The older UI plan describes a virtualized course list, calendar view, section drawer using `/api/sections`, presets, and URL synchronization. | Identifies designed-but-not-necessarily-accepted UI candidates relevant to `PH-E009`. | Medium-low authority. A tracked design document is not proof that every described surface belongs in A. |
| `PH-E028` | `IMPLEMENTATION_OBSERVATION` | Commit `71f2a2b6d6a5afab234f1686965f657af5c3652c`, blob `frontend/src/hooks/useLocalSoundNotifications.ts` | Lines 68-84 and 111-175 | The frontend stores an enabled flag, polls for local notifications, creates browser audio, and plays one tone when a non-empty notification batch is handled. | Establishes an existing browser-local sound path for downstream comparison with higher-authority notification requirements. | High confidence for code behavior at this blob. It does not establish the required trigger semantics, volume controls, or final UX. |
| `PH-E029` | `IMPLEMENTATION_OBSERVATION` | Commit `71f2a2b6d6a5afab234f1686965f657af5c3652c`, blob `README.md`; commit `5c526aece8bd1d77d46dc225f0adfb53d2d7f143`, blob `frontend/src/App.tsx` | `README.md` lines 166-200; `App.tsx` lines 10-14 and 122-125 | The tracked documentation advertises SendGrid email and local sound, and the mounted application includes a mail-settings panel. | Inventories notification surfaces that exist in project history. | High confidence as implementation/documentation state. It does **not** prove email is an A requirement; higher-authority current evidence governs. |
| `PH-E030` | `CONFLICT` | Commit `9283ba1a821f6126659363e40957ed01e2c806e9`, blob `docs/subscription_model.md`; commit `71f2a2b6d6a5afab234f1686965f657af5c3652c`, blob `README.md` | `docs/subscription_model.md` lines 20-35 versus `README.md` lines 166-200 | The subscription model says local mode is always email, while the README presents local sound as an alternative. | Records internal documentation drift around notification semantics. | High confidence that the documents disagree; neither side alone establishes user intent or the current A contract. |

### 4.4 Analysis and unresolved boundaries

| Evidence ID | Classification | Source | Locator | Quotation or faithful paraphrase | Relevance to A | Confidence / contradiction notes |
|---|---|---|---|---|---|---|
| `PH-E031` | `INFERENCE` | `PROJECT-MAY-CHAT` | Supporting records `PH-E005`, `PH-E008`, `PH-E009`, and `PH-E010` (exact user lines 628-630, 768-769, 803-804, 836-837) | Taken together, the history supports a recovery **rule**: investigate lost intended behavior, correct things written wrongly, and exclude inappropriate surfaces. It does not supply a complete feature-membership answer. | Gives downstream synthesis a defensible interpretation without converting a fuzzy rule into feature decisions. | High confidence in the rule; deliberately low confidence on membership. This is not P2 all-and-only adjudication. |
| `PH-E032` | `UNRESOLVED` | `PROJECT-MAY-CHAT` plus admitted Git candidates | `PH-E005`, `PH-E009`, `PH-E014`, `PH-E015`, `PH-E017`, and `PH-E027` | The admissible project/Git pass cannot determine which skipped/cancelled designs were genuine local-product intent versus experiments, workarounds, or later-phase ideas. | Prevents calendar/presets, standalone sections, scheduled refresh, email, SMTP, Discord, or other historical surfaces from being silently promoted or removed here. | Explicitly unresolved. The only broad matrix (`5714a8f…`) failed provenance review and remains candidate history. |
| `PH-E033` | `UNRESOLVED` | `PROJECT-MAY-CHAT` and current tracked blobs | `PH-E004`, `PH-E022`, `PH-E025`, `PH-E029`, and `PH-E030` | Project history alone does not resolve current platform scope, the packaging mechanism, standalone-section API shape, or notification-channel membership. | Routes these product choices to the canonical candidate synthesis and current-line P1 Review rather than this collection task. | Explicitly unresolved; current-mainline evidence has higher authority and is collected in a separate artifact. |

## 5. Non-adjudicative handoff

The strongest project-history evidence for downstream A synthesis is:

- direct historical intent for a complete, releasable local product
  (`PH-E001`-`PH-E004`);
- direct intent for an actual UI **and UX** rebuild (`PH-E006`-`PH-E007`);
- a provenance-sensitive recovery/correction rule rather than “ship everything
  ever mentioned” (`PH-E005`, `PH-E008`-`PH-E010`, `PH-E031`);
- concrete distribution, API, documentation, and release drift
  (`PH-E016`-`PH-E030`).

This pass deliberately stops before assigning any historical feature to the
later all-and-only result. In particular, task-015/`5714a8f…` is only a map of
questions, release archives are historical observations rather than authority,
and current code proves implementation state rather than user intent.
