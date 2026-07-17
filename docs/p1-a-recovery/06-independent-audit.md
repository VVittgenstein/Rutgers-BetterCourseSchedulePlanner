# Independent Audit of the Deliverable A P1 Candidate

Status: **CHANGES_REQUESTED**

Scope: independent P1 provenance, completeness, privacy, and phase-boundary
audit of `docs/deliverable-a-windows-local-release-requirements.md`

Audited candidate:

- merge commit: `b2047982a6adbf726b2783262c2cf131c355426a`;
- implementation commit: `a2b703820aa3d22111aeed8910bcdd4c4bcc9eb3`;
- candidate SHA-256:
  `4c9a83d6bee9e9c83d072ef69c4ba598139d0802c99aad97efb66d2a22365c3f`;
- candidate length: 303 lines;
- candidate status in the audited body: **P1 CANDIDATE - AWAITING MAINLINE
  REVIEW**, not final and not authorization for P2-P7.

This audit is evidence for task-067. It is not current-discussion-line approval,
does not finalize Deliverable A, and cannot authorize P2 or any implementation
phase.

## 1. Overall verdict

**CHANGES_REQUESTED.** The candidate is broad, well structured, non-final,
privacy-safe, and substantially complete. Mechanical provenance accounting
passes: all 33 mainline User messages reconcile one-to-one; all 53 requirement
IDs are unique; and all 125 distinct explicit evidence tokens resolve exactly
once to a reviewed companion definition with an allowed classification and a
non-empty source/locator.

The semantic source check does not fully pass. The candidate converts the
accepted suspension/background limitation into an unconditional active-watch
termination rule even though the accepted sources only make audio unreliable
on suspension and release state on connection close. It also uses `ML-E090`,
`PH-E031`, and `RC-E018` outside the subjects those records actually support.
These are correctable P1 provenance defects; there is no need to enter P2 or
open raw history.

No archived runaway-P1 artifact, discarded requirements document, raw session,
old runaway worktree/commit/transcript, secret, private inventory, or unrelated
conversation was used in this audit.

## 2. Reproducible audit method

The audit used this full-population procedure:

1. Recomputed raw-byte SHA-256 for all five task-declared external read-only
   sources before reading them. Every digest matched its pin.
2. Read the complete 1,277-line mainline export and all four accepted preflight
   records from first through last line. No keyword sample substituted for the
   full read.
3. Read the candidate, source register, and companions 02 through 05 in full.
4. Parsed every mainline `### N. User` heading, derived its exact span from the
   next message heading, and compared the resulting ordered population with
   every row of the Section 8 coverage appendix in companion 02.
5. Parsed every `A-*-NNN` requirement row and every explicit
   `ML-E*`/`PH-E*`/`RC-E*`/`TH-E*` token in the candidate. Definitions were
   matched by exact ID against companions 02-05. The check rejected missing or
   duplicate definitions, invalid classifications, empty sources, and empty or
   non-precise locators.
6. Reviewed every one of the 53 requirement rows and the candidate's historical,
   decision, journey, observation, inference, conflict, exclusion, deferred,
   and unresolved tables against the cited companion wording and authority.
7. For `ML-E*` claims, followed the underlying exact mainline/preflight locator
   in the five declared sources. For `PH-E*` and `RC-E*`, checked the reviewed
   companion definition, classification, immutable source identity, and precise
   locator without reopening undeclared external project-history or recovery
   files. For `TH-E*`, followed the cited upstream companion records. This
   preserves task-066's declared source boundary.
8. Compared the task-065 base and implementation commits, checked the candidate
   for prohibited lineage identities and private/secret path patterns, and used
   `ngagent task show task-065` only for control-plane TaskSpec and
   CompletionReport provenance—not as product evidence.

The scripts were read-only. They did not normalize source text, expand evidence
ranges, or infer that an ID supports a claim merely because it resolves.

## 3. External-source integrity

| Source | Lines | Expected SHA-256 | Observed SHA-256 | Result |
|---|---:|---|---|---|
| `MAINLINE-LOG` | 1,277 | `76fee2f09567da0afde6fe9f36048c098b6f09f363ba7ad18b8abe5ea8387562` | `76fee2f09567da0afde6fe9f36048c098b6f09f363ba7ad18b8abe5ea8387562` | PASS |
| `DUAL-WORKFLOW` | 114 | `b8edca8f7af19f7986e26c5ebbccb399535e3dcd1fa28c217e2c5cb11eda9b06` | `b8edca8f7af19f7986e26c5ebbccb399535e3dcd1fa28c217e2c5cb11eda9b06` | PASS |
| `PUBLIC-WEB-TARGET` | 113 | `770a6ee37c475e9fe78fa1cd99106aa57b8acdcdb90f32634aa027611eb49d01` | `770a6ee37c475e9fe78fa1cd99106aa57b8acdcdb90f32634aa027611eb49d01` | PASS |
| `DEPLOYMENT-DECISION` | 291 | `6c213450c970382efe4ce731c460bce51ef15ff8066e39208a48f26f5c90e1ac` | `6c213450c970382efe4ce731c460bce51ef15ff8066e39208a48f26f5c90e1ac` | PASS |
| `SHARED-RUST-ADR` | 320 | `b31c2284fa6ebb698a3e05f7839c0309c69cf89f686f8c4def63438523346946` | `b31c2284fa6ebb698a3e05f7839c0309c69cf89f686f8c4def63438523346946` | PASS |

External files modified, copied, staged, committed, or packaged by task-066:
**0**.

## 4. Full-population accounting

### 4.1 Candidate and evidence populations

| Population | Result |
|---|---:|
| Candidate requirement rows | 53 |
| Unique candidate requirement IDs | 53 |
| Duplicate requirement IDs | 0 |
| Evidence-token occurrences in the 53 requirement rows | 200 |
| Distinct evidence IDs in the 53 requirement rows | 84 |
| Evidence-token occurrences in the complete candidate | 567 |
| Distinct explicit evidence IDs in the complete candidate | 125 |
| Distinct `ML-E*` IDs | 66 |
| Distinct `PH-E*` IDs | 29 |
| Distinct `RC-E*` IDs | 22 |
| Distinct `TH-E*` IDs | 8 |
| Reviewed companion definitions available | 162 (90 ML + 33 PH + 31 RC + 8 TH) |
| Candidate-cited IDs resolving exactly once | 125/125 |
| Candidate-cited IDs with allowed classification | 125/125 |
| Candidate-cited IDs with non-empty source | 125/125 |
| Candidate-cited IDs with non-empty precise locator | 125/125 |

The count of 125 is the population of distinct explicit tokens found in the
candidate. A compact range such as `ML-E001`-`ML-E090` contributes its two
explicit endpoint tokens to that scanner; it is not silently expanded into 90
citations.

### 4.2 Complete mainline User-message reconciliation

The mainline header declares 33 User messages. Parsing the complete export found
the same 33 headings in the same order. Companion 02 contains exactly 33 unique
coverage rows: 23 Included and 10 Excluded. Every locator exactly matches the
source span; there are no omissions, duplicates, ordering differences, or
coverage-locator errors.

| User message | Exact source span | Companion disposition | Independent audit |
|---:|---|---|---|
| 1 | lines 19-21 | Excluded | PASS — preparatory memory request, no A decision |
| 7 | lines 58-60 | Included | PASS — read-only whole-round recovery purpose |
| 9 | lines 88-93 | Included | PASS — A package, purpose, honesty, and provenance |
| 12 | lines 137-139 | Included | PASS — phase line before NGAT decomposition |
| 14 | lines 194-234 | Included | PASS — dual-line P1-P7 workflow and gates |
| 16 | lines 273-275 | Excluded | PASS — request for a checklist, no new decision |
| 17 | lines 277-279 | Excluded | PASS — request for a summary, no new decision |
| 19 | lines 320-322 | Included | PASS — explicit workflow approval |
| 21 | lines 355-360 | Included | PASS — no email and shared WebUI direction |
| 23 | lines 412-417 | Included | PASS — no mail config, sound, and B purpose |
| 25 | lines 466-470 | Included | PASS — earlier latency wording retained in conflict chain |
| 27 | lines 523-527 | Included | PASS — A/B sameness, no invented notifications, refresh |
| 30 | lines 582-586 | Excluded | PASS — exploratory browser-polling questions, not accepted |
| 32 | lines 632-635 | Included | PASS — A computation and alerting are local |
| 34 | lines 670-672 | Included | PASS — active-session toggle/liveness model |
| 36 | lines 714-716 | Excluded | PASS — process-status question only |
| 38 | lines 750-763 | Included | PASS — section watches, audio, latency, refresh, limits |
| 40 | lines 797-800 | Included | PASS — final cap 9 and 10-minute/A-configurable refresh |
| 42 | lines 823-825 | Included | PASS — centralized polling and live connection accepted |
| 45 | lines 840-842 | Excluded | PASS — transition into 0B only |
| 47 | lines 892-920 | Included | PASS — provenance receipt only; not blanket evidence |
| 51 | lines 950-976 | Excluded | PASS — injected plugin/environment metadata |
| 52 | lines 978-1004 | Excluded | PASS — duplicate injected metadata |
| 53 | lines 1006-1009 | Included | PASS — mandatory separate UI implementation/polish phases |
| 57 | lines 1023-1025 | Included | PASS — unnecessary side-branch change correction |
| 58 | lines 1027-1029 | Included | PASS — interim P7-only correction |
| 59 | lines 1031-1033 | Included | PASS — final P7-only correction |
| 64 | lines 1061-1063 | Excluded | PASS — request for restatement, no new decision |
| 67 | lines 1122-1124 | Excluded | PASS — transcript-export invocation only |
| 71 | lines 1142-1144 | Included | PASS — P1 handoff/language/pre-read plus superseded granularity |
| 76 | lines 1173-1175 | Included | PASS — objection to task explosion |
| 83 | lines 1230-1232 | Included | PASS — approval to stop/freeze only |
| 90 | lines 1267-1269 | Included | PASS — complete-log handoff correction |

Coverage result: **33/33 accounted for exactly once in source order**.

## 5. Findings

### A-AUD-001 — Suspension is promoted into unconditional watch termination

- Severity: **material**.
- Candidate locators:
  - line 67, `DEC-A-005`;
  - line 87, ordinary-user journey step 8;
  - line 141, `A-WATCH-002`;
  - line 172, `A-ERR-005`.
- Cited evidence: `ML-E027`, `ML-E052`, and `TH-E005`.
- Underlying locators:
  - accepted `PUBLIC-WEB-TARGET`, lines 45-57;
  - accepted `SHARED-RUST-ADR`, lines 208-221;
  - companion 02 line 81 (`ML-E027` definition).

The accepted 0A source says locking or browser suspension means audio alerting
is not guaranteed. The accepted 0C source says connection close releases active
state. Neither says that suspension itself must immediately release active watch
state. Companion `ML-E027` over-paraphrases its underlying 0A locator by saying
state closes on suspension, and the candidate then turns that over-paraphrase
into `shall` behavior. Exact heartbeat, liveness, reconnect, and timeout
semantics are explicitly deferred elsewhere in the candidate.

Impact: the candidate silently decides part of the later realtime/liveness
contract and conflates “audio is not guaranteed” with “server watch state has
ended.” That can change reconnect behavior and what the UI may truthfully claim.

Required correction:

1. State that tab/browser close, confirmed connection close, disconnect, or a
   later-designed liveness timeout releases active watch state.
2. State separately that lock, backgrounding, or suspension makes reliable
   audio/monitoring unavailable and may lead to disconnect/timeout; do not claim
   immediate release solely from suspension.
3. Apply the correction consistently to `DEC-A-005`, journey step 8,
   `A-WATCH-002`, and `A-ERR-005`.
4. Keep exact heartbeat, timeout, reconnect, and replacement behavior unresolved
   for P3-P5.

### A-AUD-002 — `ML-E090` is used as a generic unresolved-item citation

- Severity: **moderate provenance defect**.
- Candidate locators where the token exceeds its definition:
  - line 34 (package-layout choice);
  - lines 109 and 155 (per-user directory and migration policy);
  - line 111 (final package filenames/layout);
  - line 173 (diagnostic format, retention, redaction, support bundle);
  - line 265 (crate/module split and frontend-contract mechanism);
  - line 266 (package tree, migration, launcher timeout, diagnostics).
- Evidence definition: companion 02 line 184.

`ML-E090` covers final section-key shape, WebSocket schema/reconnect,
responsible polling cadence, measured sub-second behavior, and capacity. It does
not cover package layout, Windows migration policy, diagnostic/support-bundle
mechanics, or crate decomposition. Other cited records correctly show those
details are approximate or not fixed, but `ML-E090` itself does not support
those uses.

Impact: all IDs resolve mechanically, yet the traceability graph falsely makes
one evidence record appear broader than its actual locator and wording.

Required correction: at line 34, retain `ML-E090` for its in-scope identifier
and performance subjects but add `ML-E048` for the package-layout statement.
At the other listed locators, remove the off-scope `ML-E090` token and retain
the specific accepted record that establishes the outcome or approximate
direction (`ML-E045`-`ML-E048`, `ML-E072`, or diagnostics/security records as
applicable). Say that the exact mechanic is not fixed by that source. Keep
`ML-E090` on its actual section-key, realtime, cadence, latency, and capacity
subjects.

### A-AUD-003 — `PH-E031` does not support catalog-retention inference

- Severity: **minor provenance defect**.
- Candidate locators:
  - line 83, ordinary-user journey step 4;
  - line 170, `A-ERR-003`;
  - line 212, `INF-A-002`.
- Evidence definition: companion 03 line 157.

`PH-E031` synthesizes the historical rule to investigate lost intended
behavior, correct wrongly written behavior, and exclude inappropriate surfaces.
It says nothing about atomic catalog replacement or retaining the last
successful catalog. `ML-E050` is the relevant accepted source; last-successful
retention is a transparent inference from its staging/atomic-switch rule.

Impact: the inference is visibly labeled and substantively reasonable, but the
extra citation is semantically unrelated.

Required correction: remove `PH-E031` from these three catalog-retention uses,
retain `ML-E050`, and keep the retention/retry wording explicitly labeled as a
candidate inference whose exact semantics require later confirmation.

### A-AUD-004 — `A-VAL-008` cites task-015 provenance as release evidence

- Severity: **moderate provenance defect**.
- Candidate locator: line 187, `A-VAL-008`.
- Evidence definition: companion 04 line 104, `RC-E018`.

`RC-E018` says the task-015 feature matrix was unmerged and failed provenance
review. It does not support the row's claim about historical release archives,
names, tags, deleted GitHub assets, or a freshly built package. `PH-E016` and
`PH-E018` do establish the untrusted historical package/release state. Accepted
workflow records `ML-E072` and `ML-E075` establish that P7 builds and audits a
new mandatory Windows package.

Impact: the row mixes an unrelated failed feature-matrix record into release
artifact provenance and presents an inferred safety rule as an unqualified
`shall`/`must` requirement.

Required correction: remove `RC-E018` from `A-VAL-008`; add the accepted P7
build/audit evidence (`ML-E072`, `ML-E075`) and retain `PH-E016`/`PH-E018` for
historical-artifact distrust. Label the non-reuse clause as a candidate
provenance/validation inference, or move it to the observation/provenance guard
section while keeping the mandatory newly built and audited package requirement.

### A-AUD-C01 — Premature terminal phrase in task-065 CompletionReport

- Severity: **control observation; not a candidate-body defect**.
- Control locator: task-065 CompletionReport hash
  `6343ddec493c4110b8a7f0adc130c7ee07aacae77d1d34890f869c86e6cb60be`,
  `notes` field.
- Candidate locators: lines 3-5 and 295-303.

The task-065 CompletionReport begins its notes with
`P1_COMPLETE_AWAITING_MAINLINE_REVIEW` before independent audit and task-067.
The candidate body itself is correct: it remains visibly non-final, P1-only,
and awaiting mainline review. The CompletionReport phrase is control-plane
metadata and was not used as evidence.

Required task-067 disposition: record that the earlier note was premature, do
not cite it as proof of P1 completion, apply and verify the audit corrections,
and emit the terminal phrase only in the actual task-067 stop-gate handoff after
all required P1 artifacts are complete.

## 6. Task-065 acceptance-criterion decisions

| Criterion | Decision | Independent basis / correction |
|---|---|---|
| `AC-001` — English candidate visibly awaiting mainline review | **PASS** | Correct path; 303-line English document; no Han-script content; lines 3-8 and 295-303 are explicitly non-final and P1-only. |
| `AC-002` — every material claim cites reviewed IDs and every ID resolves precisely | **CHANGES_REQUESTED** | Mechanical resolution passes 125/125, but semantic fidelity fails at A-AUD-001 through A-AUD-004. Apply the exact citation and wording corrections. |
| `AC-003` — evidence layers remain separate | **PASS** | Sections 1-2, 6-10 visibly separate history, accepted decisions, observations, inference, conflict, deferred items, and unresolved questions. No summary is silently labeled direct User evidence. |
| `AC-004` — substantive A coverage | **PASS** | All required purpose, journey, data, UI, watch/audio, config, package, recovery, documentation, validation, and release categories are present. Some rows need provenance correction, not missing-category recovery. |
| `AC-005` — no archived outcome, P2 decision, or excess P3-P7 design | **CHANGES_REQUESTED** | Archive/P2/P7 boundaries pass, but unconditional suspension termination decides a deferred liveness behavior beyond the accepted constraint. Correct A-AUD-001. |
| `AC-006` — candidate-only change; detail remains in companions | **PASS** | Upstream diff adds only the canonical candidate; detailed quotations/locators remain in companions. Candidate was not modified by task-066. |

## 7. Substantive-category decisions

| Category | Decision | Basis / correction |
|---|---|---|
| Purpose and ordinary Windows target users | **PASS** | Direct current and historical User evidence is retained without collapsing chronology. |
| Ordinary-user journey | **CHANGES_REQUESTED** | Steps 1-7 and 9 pass; step 8 must distinguish suspension from confirmed disconnect per A-AUD-001. |
| Rutgers catalog acquisition and local SQLite persistence | **PASS** | Shared client/parser, catalog domain, and SQLite direction are supported by accepted 0C plus historical context. |
| Staging validation and atomic catalog replacement | **PASS** | Accepted rule is accurate; last-successful retention remains labeled inference. Remove unrelated `PH-E031` per A-AUD-003. |
| Ten-minute A-configurable directory refresh | **PASS** | Default, A-only configurability, centralized coordination, and single-flight direction match accepted evidence. |
| Search, filtering, course/section inspection, and status | **PASS** | User-visible outcomes are required without selecting the historical standalone endpoint. |
| Active watches capped at nine | **CHANGES_REQUESTED** | Cap, explicit start, and connection scope pass; suspension termination must be corrected per A-AUD-001. |
| Central local polling and WebSocket delivery | **CHANGES_REQUESTED** | Centralization, freshness, and latest-value behavior pass; exact liveness/reconnect remains deferred and must not be partially decided through suspension wording. |
| Fresh-Open audio, volume, one-shot/continuous, no debounce | **PASS** | Correctly requires every fresh Open message rather than transition-only alerting. |
| Background/lock limitation | **PASS** | “Audio not guaranteed” is accurate. It must remain separate from active-state release semantics. |
| Local-only configuration | **CHANGES_REQUESTED** | Product behavior passes; remove off-scope `ML-E090` uses for exact directory/migration mechanics. |
| Windows package, BAT launcher, and no separate runtime | **CHANGES_REQUESTED** | Accepted behavior passes; package-layout citation cleanup is required by A-AUD-002. |
| Loopback-only service, local SQLite, and per-user state | **CHANGES_REQUESTED** | `127.0.0.1`, SQLite, and appropriate per-user storage pass; migration-detail traceability needs A-AUD-002. |
| Startup, health, error, recovery, and diagnostics | **CHANGES_REQUESTED** | Health/freshness and labeled inferences pass; correct suspension semantics and off-scope/generic evidence uses under A-AUD-001 through A-AUD-003. |
| Documentation truthfulness | **PASS** | Working behavior, limitations, startup, and diagnostics must match shipped documentation; no obsolete capability is promoted. |
| Windows clean-machine validation | **PASS** | Reasonable, visible inference from ordinary-user/no-separate-runtime acceptance and observed Node/native-build drift. |
| Secret and private-artifact audit | **PASS** | No secret values, provider inventory, private paths, or raw tool traces occur in the candidate. Accepted P7 audit boundary is preserved. |
| Release expectations | **CHANGES_REQUESTED** | Mandatory audited package and conditional GitHub Release are correct; repair `A-VAL-008` under A-AUD-004. |
| Aspirational near-one-second posture | **PASS** | It is consistently a later measurement target, never a current guarantee. |
| Email/push/account/architecture/deployment exclusions | **PASS** | No email fallback or invented notification product; no persistent active-watch account; B deployment remains outside A/P1. |
| Historical optional surfaces and stale artifacts | **PASS** | Calendar, Compact, saved views, share links, presets, standalone sections route, Discord, stale refresh code, task-015, deleted releases, and old packages are not promoted. |
| Deferred exact decisions | **CHANGES_REQUESTED** | The required key/schema/cadence/latency/capacity/UI/module/path questions are visible; correct suspension predecision and `ML-E090` overbreadth. |
| P1-only and non-final phase boundary | **PASS** | No P2 adjudication or authorization, no P3-P7 implementation, and no production Vultr action occurs. P7.2/P7.3 appear only as accepted later gates. |
| Quarantine, privacy, and raw-history gate | **PASS** | No prohibited runaway lineage identity or private/secret pattern appears. Companion 05 accurately records `VERIFIED NO ACCESS`; the candidate's TH index does not claim any raw read. |

## 8. Requirement-row population

Every one of the 53 requirement rows is enumerated below. “Citation only” means
the product wording is otherwise supported but a cited token must be removed or
re-scoped.

| Requirement | Evidence IDs as audited | Decision |
|---|---|---|
| `A-PROD-001` | `ML-E003`, `ML-E004`, `ML-E005`, `PH-E002`, `PH-E004`, `RC-E003`, `TH-E001` | PASS |
| `A-PROD-002` | `ML-E005`, `ML-E006`, `PH-E008`, `PH-E010`, `RC-E006` | PASS |
| `A-PROD-003` | `ML-E010`, `ML-E024`, `ML-E026`, `ML-E044`, `ML-E047`, `ML-E057` | PASS |
| `A-PROD-004` | `ML-E062`, `ML-E068`, `PH-E032`, `TH-E002`, `TH-E003` | PASS |
| `A-PKG-001` | `ML-E003`, `ML-E043`, `ML-E048`, `TH-E004` | PASS |
| `A-PKG-002` | `ML-E043`, `ML-E047`, `TH-E004` | PASS |
| `A-PKG-003` | `ML-E003`, `ML-E048` | PASS |
| `A-PKG-004` | `ML-E047` | PASS |
| `A-PKG-005` | `ML-E047`, `ML-E090` | **CHANGES_REQUESTED — citation only; A-AUD-002** |
| `A-PKG-006` | `ML-E038`, `ML-E075`, `ML-E078` | PASS |
| `A-PKG-007` | `ML-E048`, `ML-E072`, `ML-E090` | **CHANGES_REQUESTED — citation only; A-AUD-002** |
| `A-DATA-001` | `ML-E046`, `ML-E050`, `RC-E010` | PASS |
| `A-DATA-002` | `ML-E050`, `RC-E011`, `RC-E012` | PASS |
| `A-DATA-003` | `ML-E046`, `ML-E050` | PASS |
| `A-DATA-004` | `ML-E020`, `ML-E030`, `ML-E047`, `TH-E006` | PASS |
| `A-DATA-005` | `ML-E021`, `ML-E047`, `ML-E053`, `TH-E006` | PASS |
| `A-DATA-006` | `ML-E053`, `ML-E090` | PASS |
| `A-DATA-007` | `ML-E051` | PASS |
| `A-DATA-008` | `ML-E053`, `ML-E090` | PASS |
| `A-UI-001` | `ML-E026`, `RC-E011`, `RC-E012` | PASS |
| `A-UI-002` | `PH-E023`, `PH-E024`, `PH-E025`, `RC-E021`, `RC-E023`, `TH-E002` | PASS |
| `A-UI-003` | `ML-E024`, `ML-E026`, `ML-E032`, `ML-E044`, `ML-E074` | PASS |
| `A-UI-004` | `ML-E070`, `ML-E071`, `ML-E074` | PASS |
| `A-WATCH-001` | `ML-E012`, `ML-E020`, `ML-E027`, `ML-E052`, `TH-E005` | PASS |
| `A-WATCH-002` | `ML-E027`, `ML-E052` | **CHANGES_REQUESTED — A-AUD-001** |
| `A-WATCH-003` | `ML-E028`, `ML-E052`, `ML-E090`, `TH-E002` | PASS |
| `A-WATCH-004` | `ML-E047`, `ML-E054`, `ML-E090` | PASS |
| `A-WATCH-005` | `ML-E015`, `ML-E029`, `ML-E051`, `ML-E054`, `TH-E005` | PASS |
| `A-WATCH-006` | `ML-E014`, `ML-E029`, `TH-E005` | PASS |
| `A-WATCH-007` | `ML-E009`, `ML-E016`, `ML-E025`, `ML-E027`, `TH-E007` | PASS |
| `A-WATCH-008` | `ML-E051`, `ML-E054` | PASS |
| `A-WATCH-009` | `ML-E017`, `ML-E030`, `ML-E081`, `ML-E090`, `TH-E005` | PASS |
| `A-CFG-001` | `ML-E020`, `ML-E030`, `ML-E047`, `TH-E006` | PASS |
| `A-CFG-002` | `ML-E047`, `ML-E090` | **CHANGES_REQUESTED — citation only; A-AUD-002** |
| `A-CFG-003` | `ML-E008`, `ML-E009`, `ML-E025`, `ML-E056`, `TH-E007` | PASS |
| `A-CFG-004` | `ML-E027`, `ML-E052` | PASS |
| `A-CFG-005` | `ML-E038`, `ML-E075`, `ML-E078` | PASS |
| `A-ERR-001` | `ML-E046`, `ML-E048` | PASS |
| `A-ERR-002` | `ML-E005`, `ML-E046`, `ML-E047`, `ML-E048`, `ML-E070` | PASS — visibly labeled inference |
| `A-ERR-003` | `ML-E050`, `PH-E031` | **CHANGES_REQUESTED — citation only; A-AUD-003** |
| `A-ERR-004` | `ML-E051`, `ML-E053` | PASS |
| `A-ERR-005` | `ML-E027`, `ML-E052`, `ML-E054`, `ML-E070`, `ML-E090` | **CHANGES_REQUESTED — A-AUD-001** |
| `A-ERR-006` | `ML-E038`, `ML-E046`, `ML-E047`, `ML-E078`, `ML-E090` | **CHANGES_REQUESTED — citation only; A-AUD-002** |
| `A-ERR-007` | `ML-E005`, `ML-E006`, `ML-E070`, `RC-E006`, `RC-E015` | PASS |
| `A-VAL-001` | `ML-E005`, `ML-E006`, `ML-E048`, `RC-E006`, `RC-E015`, `PH-E019`, `PH-E029` | PASS |
| `A-VAL-002` | `ML-E003`, `ML-E043`, `ML-E047`, `PH-E019`, `PH-E020`, `PH-E021`, `PH-E022`, `RC-E007` | PASS |
| `A-VAL-003` | `ML-E005`, `ML-E026`, `ML-E029`, `ML-E046`, `ML-E050`, `ML-E051`, `ML-E052`, `ML-E053`, `ML-E072` | PASS |
| `A-VAL-004` | `ML-E030`, `ML-E081`, `ML-E090`, `TH-E005` | PASS |
| `A-VAL-005` | `ML-E072`, `ML-E075`, `ML-E077`, `ML-E078` | PASS |
| `A-VAL-006` | `ML-E074`, `ML-E077` | PASS |
| `A-VAL-007` | `ML-E064`, `ML-E072`, `ML-E073`, `ML-E075` | PASS |
| `A-VAL-008` | `PH-E016`, `PH-E018`, `RC-E018` | **CHANGES_REQUESTED — A-AUD-004** |
| `A-VAL-009` | `ML-E066`, `ML-E067`, `ML-E068`, `ML-E076` | PASS |

Requirement-row result: **45 PASS, 8 CHANGES_REQUESTED**. No requirement row
is blocked by missing evidence or a need for raw-history access.

## 9. Complete cited-evidence token population

The following are the 125 distinct explicit tokens independently resolved and
semantically reviewed.

### `ML-E*` — 66

`ML-E001`, `ML-E003`, `ML-E004`, `ML-E005`, `ML-E006`, `ML-E008`,
`ML-E009`, `ML-E010`, `ML-E011`, `ML-E012`, `ML-E014`, `ML-E015`,
`ML-E016`, `ML-E017`, `ML-E020`, `ML-E021`, `ML-E024`, `ML-E025`,
`ML-E026`, `ML-E027`, `ML-E028`, `ML-E029`, `ML-E030`, `ML-E032`,
`ML-E036`, `ML-E038`, `ML-E039`, `ML-E040`, `ML-E042`, `ML-E043`,
`ML-E044`, `ML-E045`, `ML-E046`, `ML-E047`, `ML-E048`, `ML-E050`,
`ML-E051`, `ML-E052`, `ML-E053`, `ML-E054`, `ML-E055`, `ML-E056`,
`ML-E057`, `ML-E058`, `ML-E061`, `ML-E062`, `ML-E064`, `ML-E066`,
`ML-E067`, `ML-E068`, `ML-E070`, `ML-E071`, `ML-E072`, `ML-E073`,
`ML-E074`, `ML-E075`, `ML-E076`, `ML-E077`, `ML-E078`, `ML-E080`,
`ML-E081`, `ML-E082`, `ML-E083`, `ML-E084`, `ML-E089`, `ML-E090`.

### `PH-E*` — 29

`PH-E001`, `PH-E002`, `PH-E003`, `PH-E004`, `PH-E005`, `PH-E006`,
`PH-E008`, `PH-E009`, `PH-E010`, `PH-E014`, `PH-E015`, `PH-E016`,
`PH-E017`, `PH-E018`, `PH-E019`, `PH-E020`, `PH-E021`, `PH-E022`,
`PH-E023`, `PH-E024`, `PH-E025`, `PH-E026`, `PH-E027`, `PH-E028`,
`PH-E029`, `PH-E030`, `PH-E031`, `PH-E032`, `PH-E033`.

### `RC-E*` — 22

`RC-E001`, `RC-E002`, `RC-E003`, `RC-E006`, `RC-E007`, `RC-E009`,
`RC-E010`, `RC-E011`, `RC-E012`, `RC-E013`, `RC-E014`, `RC-E015`,
`RC-E016`, `RC-E018`, `RC-E021`, `RC-E022`, `RC-E023`, `RC-E025`,
`RC-E026`, `RC-E027`, `RC-E029`, `RC-E031`.

### `TH-E*` — 8

`TH-E001`, `TH-E002`, `TH-E003`, `TH-E004`, `TH-E005`, `TH-E006`,
`TH-E007`, `TH-E008`.

Resolution result: **125/125 definitions unique; 125/125 source/locator fields
present**. Semantic-use exceptions are exhaustively identified in A-AUD-001
through A-AUD-004; no missing or duplicate definition exists.

## 10. Quarantine, privacy, and phase-scope audit

### 10.1 Evidence quarantine

PASS. The source register prohibits old runaway tasks 016-060 and their
attempts, worktrees, commits, transcripts, reviews, merges, generated outcomes,
the old root requirements document, and all NGAT archives. The candidate
contains none of the ten named runaway merge identities and no archive path.
Its references to task-015/`5714a8f` occur only in the explicitly admitted
candidate-history/provenance context and consistently reject that matrix as
authority.

Companions 02-05 attest that archived runtime, discarded requirements, raw
session files, and undeclared private material were not opened. Companion 05
accurately records zero declared raw files and **VERIFIED NO ACCESS** for all
eight gaps and seven candidates. This audit did not reopen those sources.

### 10.2 Privacy

PASS. The candidate contains no absolute user/private path, IP address, UUID,
credential value, token, SSH material, billing datum, raw tool trace, or provider
inventory. It states the no-secret boundary without reproducing the private
details present in operational records. The audit likewise omits them.

### 10.3 P1-only boundary

PASS subject to A-AUD-001. The candidate:

- remains non-final and awaiting current-discussion-line P1 Review;
- does not perform the P2 all-and-only decision for optional historical
  surfaces;
- does not authorize P2;
- does not design or implement P3-P7 beyond already accepted 0A-0C/workflow
  constraints;
- states P7.2 and P7.3 only as later mandatory gates;
- performs no deployment or Vultr action;
- leaves exact section key, realtime schema/liveness/reconnect, responsible
  cadence/backoff, latency measurement, capacity thresholds, final A/B UI,
  crate/module split, filenames/layout/per-user details, and optional surface
  membership for named later gates.

A-AUD-001 must be corrected because its suspension wording partially collapses
the otherwise deferred liveness/reconnect design.

## 11. Task-067 correction checklist

Task-067 should perform all of the following before issuing the P1 stop-gate
handoff:

- [ ] Correct suspension/background/connection-close semantics at candidate
  lines 67, 87, 141, and 172 as specified by A-AUD-001.
- [ ] Apply A-AUD-002: add `ML-E048` to line 34 for package layout; remove
  `ML-E090` from listed uses where it supports no subject; retain it for section
  key, realtime/reconnect, polling cadence, latency, and capacity.
- [ ] Remove `PH-E031` from the catalog-retention inference at lines 83, 170,
  and 212; keep `ML-E050` and the explicit inference qualification.
- [ ] Correct `A-VAL-008`: remove `RC-E018`, add accepted P7 package build/audit
  support, and label the historical-artifact non-reuse rule as inference or a
  provenance guard.
- [ ] Re-run full-population validation after editing: 53 unique requirement
  rows, no uncited material row, no missing/duplicate evidence definition, and
  no unsupported token use. Do not preserve the number 125 if the corrected
  explicit-token population changes; report the new truthful count.
- [ ] Preserve the exact 33/33 mainline coverage accounting and do not rewrite
  companion coverage dispositions without new admissible evidence.
- [ ] Preserve all accepted substantive boundaries: no email or invented
  notification channel; active-session cap nine; fresh-Open audio; 10-minute
  A-configurable directory refresh; local centralized polling; Windows
  package/BAT/no-separate-runtime; loopback/SQLite/per-user state; aspirational
  latency; and the listed deferred questions.
- [ ] Preserve quarantine and privacy: do not open raw sessions, archives,
  discarded requirements, runaway worktrees/commits/transcripts, or private
  inventory; do not reproduce secrets or tool traces.
- [ ] Record A-AUD-C01 as a control/provenance observation. Do not treat the
  task-065 CompletionReport note as P1 completion evidence.
- [ ] Keep the corrected candidate visibly non-final and P1-only. Do not
  authorize P2 or perform P2-P7 work.
- [ ] Emit `P1_COMPLETE_AWAITING_MAINLINE_REVIEW` only from the actual task-067
  stop-gate completion after all corrections and validations pass.

## 12. Handoff

The candidate is close to review-ready but is not yet clean enough for a PASS.
Task-067 should apply the bounded corrections above, verify the corrected
population, publish the P1 stop-gate record, and return control to the current
discussion line. No raw-history access or phase expansion is justified.
