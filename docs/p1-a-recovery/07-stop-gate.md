# P1 Stop Gate for Deliverable A

Status: **corrected P1 candidate complete; current-discussion-line review required**

Scope: final P1 verification for the Windows-local Deliverable A candidate

Authority boundary: this report completes the bounded P1 correction handoff. It
does not finalize Deliverable A, decide the P2 all-and-only surface, authorize
P2, or perform any P3-P7 design, implementation, packaging, release, or
deployment work.

## 1. Final artifact manifest

The corrected canonical candidate remains visibly non-final and retains the
status **P1 CANDIDATE - AWAITING MAINLINE REVIEW**.

| Artifact | Repository-relative path | Contentful lines | Raw-byte SHA-256 | State |
|---|---|---:|---|---|
| Corrected candidate | `docs/deliverable-a-windows-local-release-requirements.md` | 303 | `aede4b98b69219b879a4c997f92ea3066cde79ac214e7990a23ce1f7c8edeef2` | Corrected P1 candidate; not final |
| Companion 01 | `docs/p1-a-recovery/01-source-register.md` | 323 | `81214f39e086056eaca23ca897e25dbc4d102f8803b512b541c174a0869a642a` | Source/provenance authority |
| Companion 02 | `docs/p1-a-recovery/02-mainline-evidence.md` | 252 | `9c2aeaba7b1c1d8b5d2196198e1a1041e671844d4fde433cc9a078b0b6becb6b` | Complete mainline evidence and 33/33 coverage |
| Companion 03 | `docs/p1-a-recovery/03-project-history-evidence.md` | 176 | `d203257c9d7bfeace8b4fc68f8f20ab25a507e341a90fdb82bf16c6f7f1965a5` | Legitimate project/Git evidence |
| Companion 04 | `docs/p1-a-recovery/04-recovery-corpus-evidence.md` | 201 | `0a72da4578473b73e33f12cc49ec4814748c15e5ce30362e097db63c87768617` | Recovery-corpus evidence with authority limits |
| Companion 05 | `docs/p1-a-recovery/05-targeted-history-gap-closure.md` | 97 | `ea09dd124fac5a6952a035aaac96c122092e6bbd25e4a0626c25300949f241d8` | **VERIFIED NO ACCESS** raw-history result |
| Companion 06 | `docs/p1-a-recovery/06-independent-audit.md` | 533 | `f50e968e8edf22a54d698076a106fcc871432a3cb0927946d0202d2fb433046b` | Independent audit; verdict `CHANGES_REQUESTED` |

The candidate, companions 01-06, and this report are non-empty Markdown with a
top-level heading and substantive body. No empty marker or terminal-carried
evidence substitute was used.

### 1.1 Self-hash boundary for this report

`docs/p1-a-recovery/07-stop-gate.md` cannot embed a stable SHA-256 of itself:
embedding that digest would change the bytes being hashed. The task-067
CompletionReport will attest this file's final raw-byte SHA-256 after the
substantive commit. The later NGAT ReviewArtifact and MergeArtifact remain the
managed provenance gates that can independently bind the reviewed and merged
bytes. This report does not claim that either downstream gate has already run.

## 2. Dependency, review, and merge status

| Gate | Status at this handoff | Proof and qualification |
|---|---|---|
| Canonical candidate input | Satisfied | Task-065 merged the non-final candidate as `b2047982a6adbf726b2783262c2cf131c355426a`. |
| Independent-audit dependency | Satisfied | Task-066 is merged as `ab40c80cbfec465eb5f60ab2d272656e618b739f`; therefore task-067's declared dependency is present. |
| Independent product/provenance audit | `CHANGES_REQUESTED`, now dispositioned | Companion 06 identified four correction findings and one control observation. Its verdict was not a candidate PASS. Section 3 records every resolution. |
| Task-067 executor validation | Complete before handoff | The corrected full population and phase boundary pass the checks in Sections 4-7. |
| Task-067 NGAT managed review | Pending after CompletionReport | The supervisor must run the managed review; this document does not pre-claim its result. |
| Task-067 merge | Pending a passing managed review | Merge remains a Main Agent lifecycle action. |
| Current-discussion-line P1 Review | Mandatory and pending | Only the current discussion line may correct, accept, or reject the candidate and authorize any later transition. |

The upstream task-066 managed lifecycle being merged establishes that its audit
artifact passed its own task gate; it does not convert the audit's
`CHANGES_REQUESTED` product verdict into approval of the pre-correction
candidate.

## 3. Independent-audit finding resolutions

| Finding | Resolution in the corrected candidate | Result |
|---|---|---|
| `A-AUD-001` | Corrected `DEC-A-005`, ordinary-user journey step 8, `A-WATCH-002`, and `A-ERR-005`. Tab/browser close, confirmed connection close, disconnect, or a later-designed liveness timeout releases active-watch state. Lock, backgrounding, or suspension separately makes reliable audio and monitoring unavailable and may lead to disconnect or timeout; suspension alone is not specified as immediate release. Heartbeat, timeout, liveness, reconnect, and replacement mechanics remain P3-P5 questions. The corrected rows cite the connection, audio-limitation, realtime, and unresolved-design records. | **RESOLVED** |
| `A-AUD-002` | Added `ML-E048` to the early package-layout/deferred statement. Removed off-subject `ML-E090` from `A-PKG-005`, `A-PKG-007`, `A-CFG-002`, `A-ERR-006`, `UNR-A-007`, and `UNR-A-008`. The affected text now says the cited records fix outcomes or approximate direction but not exact directory, migration, layout, module, contract, launcher, or diagnostic mechanics. `ML-E090` remains only where its section-key, realtime/reconnect, cadence, latency, or capacity subjects are actually used. | **RESOLVED** |
| `A-AUD-003` | Removed `PH-E031` from ordinary-user journey step 4, `A-ERR-003`, and `INF-A-002`. Each keeps `ML-E050`; last-successful-catalog retention and retry remain visibly labeled candidate inference whose exact semantics require later confirmation. | **RESOLVED** |
| `A-AUD-004` | Reworked `A-VAL-008`: removed unrelated `RC-E018`; added accepted P7 build/audit support `ML-E072` and `ML-E075`; retained `PH-E016` and `PH-E018` only for historical package/release distrust. Building and auditing a new Windows package remains mandatory, while historical-artifact non-reuse is explicitly labeled a candidate provenance/validation inference. | **RESOLVED** |
| `A-AUD-C01` | Task-065 CompletionReport hash `6343ddec493c4110b8a7f0adc130c7ee07aacae77d1d34890f869c86e6cb60be` used `P1_COMPLETE_AWAITING_MAINLINE_REVIEW` prematurely. That immutable note is control metadata only, is not cited as completion evidence, and does not alter the candidate body's non-final status. Task-067 alone owns the actual terminal handoff after correction and validation. | **RECORDED; NOT PRODUCT EVIDENCE** |

All four `CHANGES_REQUESTED` findings are corrected rather than silently
waived. No audit correction is being converted into a P2 or P3-P7 decision.
Genuine product/design questions remain explicit in Section 7.

## 4. Full-population revalidation

The revalidation scanned the complete corrected candidate, not a sample. It
parsed every requirement row and every explicit `ML-E*`, `PH-E*`, `RC-E*`,
and `TH-E*` token, then matched each cited token against the complete
definition population in companions 02-05. Raw bytes, rather than normalized
text, produced the hashes in Section 1.

| Population or check | Corrected result |
|---|---:|
| Candidate requirement rows | 53 |
| Unique candidate requirement IDs | 53 |
| Duplicate requirement IDs | 0 |
| Requirement rows without evidence | 0 |
| Evidence-token occurrences in the 53 requirement rows | 201 |
| Distinct evidence IDs in the 53 requirement rows | 82 |
| Requirement-row `ML` occurrences / distinct IDs | 154 / 51 |
| Requirement-row `PH` occurrences / distinct IDs | 16 / 15 |
| Requirement-row `RC` occurrences / distinct IDs | 14 / 9 |
| Requirement-row `TH` occurrences / distinct IDs | 17 / 7 |
| Evidence-token occurrences in the complete candidate | 571 |
| Distinct explicit evidence IDs in the complete candidate | 125 |
| Complete-candidate `ML` occurrences / distinct IDs | 371 / 66 |
| Complete-candidate `PH` occurrences / distinct IDs | 96 / 29 |
| Complete-candidate `RC` occurrences / distinct IDs | 46 / 22 |
| Complete-candidate `TH` occurrences / distinct IDs | 58 / 8 |
| Reviewed companion definitions available | 162 |
| Candidate-cited IDs resolving exactly once | 125/125 |
| Candidate-cited IDs with an allowed classification | 125/125 |
| Candidate-cited IDs with a non-empty source | 125/125 |
| Candidate-cited IDs with a non-empty precise locator | 125/125 |

The corrected distinct-token count truthfully remains 125, while occurrence
counts changed with the audit edits. Compact evidence-family ranges contribute
only their explicitly written endpoint tokens; they are not silently expanded.

Semantic revalidation used the independent audit's exhaustive baseline and
then reviewed every changed citation plus every surviving occurrence of
`ML-E090`, `PH-E031`, and `RC-E018`. No unsupported semantic token use remains:
`ML-E090` is limited to its real subjects, `PH-E031` supports only the
historical recovery rule, and `RC-E018` supports only task-015 provenance.

Companion 02 is byte-unchanged, so its complete mainline reconciliation remains
**33/33 User messages accounted for exactly once in source order**. No
coverage row or companion 01-06 was edited by task-067.

Additional candidate guards pass:

- English-only body with no Han-script content;
- exactly one visible `P1 CANDIDATE - AWAITING MAINLINE REVIEW` marker;
- no terminal completion state embedded in the candidate;
- one canonical candidate document at the approved `docs/` path;
- no absolute private path or prohibited runaway-lineage identity;
- contentful Markdown rather than an empty marker.

## 5. Accepted substantive boundaries preserved

| Boundary | Verification result |
|---|---|
| Ordinary-user distribution | Windows-local x64 package direction, BAT entry, and no separately installed Node, npm, Rust, SQLite, compiler/build tooling, or web server remain required. |
| Local composition | Loopback-only Rust/local composition, bundled React WebUI, local SQLite, local refresh/polling, and per-user data/config/log outcome remain intact; exact paths and mechanics remain open. |
| Catalog behavior | Rutgers catalog acquisition, staging validation, atomic replacement, and a default ten-minute A-configurable directory refresh remain intact. Last-successful retention remains inference. |
| Discovery experience | Search/filter, course inspection, section inspection, and current status remain required without selecting an unaccepted endpoint or screen shape. |
| Active watches | Explicit live-WebUI watches remain capped at nine. Remembered choices are inactive until explicitly started. |
| Central status path | Rutgers work remains centralized in the local core with accepted realtime delivery; browser-direct Rutgers polling is not introduced. |
| Alert semantics | Every fresh received Open status triggers browser audio, with volume and one-shot/continuous modes and no debounce. |
| Background limitation | Lock, backgrounding, and suspension make reliable audio/monitoring unavailable; they are not misrepresented as guaranteed background alerting or automatic immediate watch release. |
| Notification exclusions | No email, SMTP, SendGrid, mail configuration, push, app, or invented notification channel is introduced. |
| Latency posture | Near-one-second behavior remains aspirational, measurable later, and never a P1 implementation guarantee. |
| Phase boundary | Optional-surface membership, detailed interface/architecture choices, implementation, release, and deployment remain outside P1. |

## 6. Raw-history, quarantine, and privacy proof

Raw-history decision: **VERIFIED NO ACCESS**.

- Task-067 used only merged repository artifacts as correction evidence.
- No raw Codex or Claude session, archived runtime outcome, discarded
  requirements document, runaway-P1 worktree/commit/transcript, private
  inventory, secret, or unrelated conversation was opened or used.
- Companion 05's eight-gap and seven-candidate no-access dispositions remain
  byte-unchanged and authoritative for this P1 run.
- The candidate and this report contain no credential value, secret material,
  absolute private path, raw tool output, private inventory, or unsupported
  final product decision.
- No completion claim depends on an archived runaway-P1 outcome. Quarantined
  history appears only as an exclusion/control boundary, never as substantive
  evidence.
- The dirty parent checkout and all unrelated state were left untouched.

## 7. Explicit questions for current-discussion-line P1 Review

The current discussion line must review these questions and either accept their
deferral/ownership or resolve any product-level ambiguity. P1 does not answer
their later design mechanics.

| Question | Required review disposition |
|---|---|
| Section key | Confirm what exact section identity must later be proven unique and stable across the supported term/campus scope. |
| Realtime contract and liveness | Confirm deferral of exact schema/versioning, heartbeat, liveness timeout, reconnect, replacement, and suspension/disconnect behavior to P3-P5. |
| Responsible polling | Confirm deferral of active/idle openSections cadence, adaptive policy, timeout/backoff constants, and upstream-rate evidence. |
| Latency measurement | Confirm the later start/end timestamps, machine/network/load conditions, measured distribution, and acceptable observed result for the aspiration. |
| Capacity thresholds | Confirm which A resource thresholds and B shared-core/load thresholds later validation must measure; no hypothesis is promoted into a promise. |
| Final A/B UI treatment | Confirm later ownership of shared UI treatment, A-only configuration placement, accessibility, responsiveness, and interaction detail. |
| Crate/module/contracts split | Confirm later ownership of exact crate/module boundaries, checked frontend-contract mechanism, and internal responsibilities. |
| Final package and local mechanics | Confirm later ownership of archive/executable names, package tree, per-user Windows location, upgrade/migration policy, launcher timeout, and diagnostic/log format and retention. |
| Optional historical surface membership | Decide at the current-line P1 Review/P2 boundary whether Calendar, Compact view, saved views, share links, presets, standalone section routing, Discord, or any other unaccepted historical candidate belongs in the all-and-only surface. None is promoted here. |

## 8. No-work and phase-boundary attestation

| Prohibited expansion | Observed in task-067 |
|---|---:|
| Source-code files created or modified | 0 |
| Test files created, modified, or deleted | 0 |
| Product test suites or implementation validation run | 0 |
| Packages built or installed | 0 |
| Releases created or published | 0 |
| Servers started, changed, or deployed | 0 |
| Credentials, remote infrastructure, or private inventory accessed | 0 |
| Remote pushes | 0 |
| Dependencies added | 0 |
| Application interfaces changed | 0 |
| P2 audits or all-and-only decisions performed | 0 |
| P3-P7 design, implementation, packaging, or deployment work performed | 0 |

The only repository changes are the bounded independent-audit corrections to
`docs/deliverable-a-windows-local-release-requirements.md` and this stop-gate
report. Validation was document-only and read-only; it created no test or
tool-output artifact. There is no dependency change and no application
interface change.

After the substantive task-067 commit and CompletionReport, NGAT managed review
and merge remain pending. No action beyond this P1 gate is authorized, and P2
has not begun.

P1_COMPLETE_AWAITING_MAINLINE_REVIEW
