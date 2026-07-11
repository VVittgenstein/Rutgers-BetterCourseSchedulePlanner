# Targeted Raw-History Gap Closure for Deliverable A

## 1. Gap decision: verified no access

**Decision: VERIFIED NO ACCESS.** Tasks 061-063 leave no Deliverable A
requirement or provenance question that justifies opening raw history. The
dispatched task declares zero exact raw-session files as external read-only
sources. Candidate IDs below refer only to metadata already recorded in
`docs/p1-a-recovery/04-recovery-corpus-evidence.md`; no raw pointer was copied,
followed, or opened.

The first table maps every named recovery-corpus gap to its proposed raw access
and to the higher-authority evidence that controls the decision.

| Gap | Named unresolved question from tasks 061-063 | Proposed raw access from task-063 | Comparison result | Access decision and gap outcome |
|---|---|---|---|---|
| `RC-GAP-001` | What exact user-authored wording and acceptance state established the complete, releasable Windows-local product, its honest all-and-only surface, and its ordinary-user distribution purpose? | `RC-CAND-001` | `ML-E003`-`ML-E007` provide current direct-user wording for the Windows/BAT release, releasability, verification/documentation, honest surface, and recovery duty. `PH-E001`-`PH-E004` independently provide exact historical user statements for the local first phase, complete releasability, broad completeness, and unpack-and-use distribution. | **NO ACCESS; CLOSED.** No material operative requirement or acceptance question remains for the broad raw candidate to answer. |
| `RC-GAP-002` | Must section information ship through a standalone `/api/sections` endpoint, through the course surface, or through another section-key/interface shape? | `RC-CAND-001` | `ML-E026` accepts course and section inspection as the product outcome. `ML-E028` and `ML-E090` deliberately leave section-key and wire/interface details for later design. `PH-E023`-`PH-E025` demonstrate endpoint/documentation drift but do not establish user intent. | **NO ACCESS; REJECTED AS OUT OF P1 SCOPE.** Section inspection is an accepted requirement; endpoint and key shape remain later design choices, not a missing A requirement. |
| `RC-GAP-003` | Were Calendar, Compact view, saved views, or share links explicitly required for A? | `RC-CAND-001` | `PH-E027`, `PH-E032`, `RC-E026`, and `RC-GAP-003` preserve these only as historical candidates. `ML-E032` leaves final UI treatment for later work. No admitted evidence accepts the named features, and task-063 supplies no narrow raw locator. | **NO ACCESS; REMAINS UNRESOLVED AND UNACCEPTED.** Broad crawling is disproportionate; the optional candidates are not promoted into A. |
| `RC-GAP-004` | What are A's unpacking, launcher, privilege, and separately installed runtime constraints? | `RC-CAND-001` was noted as too broad | `ML-E003`, `ML-E043`, `ML-E047`, and `ML-E048` accept an ordinary-user Windows package, BAT launch, local-only composition, and no separately installed Node, npm, Rust, SQLite. | **NO ACCESS; CLOSED** by accepted 0A-0C context. |
| `RC-GAP-005` | What are the watch limit, fresh-Open trigger, audio controls/modes, and latency status? | No minimally located recovery candidate | `ML-E014`-`ML-E020`, `ML-E027`, `ML-E029`-`ML-E030`, and `ML-E051`-`ML-E054` accept at most nine active-session watches, audio on every fresh Open message, no debounce, volume plus one-shot/continuous modes, and aspirational rather than guaranteed near-one-second behavior. | **NO ACCESS; CLOSED** by accepted 0A-0C context. |
| `RC-GAP-006` | What catalog-refresh cadence and A configurability are required? | No minimally located recovery candidate | `ML-E020`, `ML-E030`, `ML-E047`, and `ML-E053` accept a ten-minute default, local-A configurability, and centralized/single-flight refresh behavior subject to later responsible-cadence design. | **NO ACCESS; CLOSED** by accepted 0A-0C context. |
| `RC-GAP-007` | Was optional SendGrid email historically desired for the v1 A release? | `RC-CAND-001` was mentioned as a possible historical discussion | `ML-E008`-`ML-E010`, `ML-E025`, and `ML-E056` accept no email, SMTP, SendGrid, mail configuration, or invented notification channel in A/B v1; browser audio remains. | **NO ACCESS; CLOSED.** Historical curiosity cannot reopen the accepted no-email product decision. |
| `RC-GAP-008` | Is exact historical local-versus-cloud dialogue needed to repair a demonstrated provenance defect? | `RC-CAND-002` | `ML-E011`, `ML-E024`, `ML-E033`-`ML-E045`, and `ML-E058` establish the accepted A-local/B-public split, separate composition roots, and phase ownership. Tasks 061-063 demonstrate no audit-specific provenance defect requiring the raw dialogue. | **NO ACCESS; CLOSED.** The candidate is unnecessary for current A provenance. |

Every raw candidate proposed or retained by task-063 also has an explicit
disposition:

| Raw candidate | Named gap mapping | Disposition |
|---|---|---|
| `RC-CAND-001` | Primary proposal for `RC-GAP-001`-`RC-GAP-003`; also mentioned but already rejected as too broad or obsolete for `RC-GAP-004` and `RC-GAP-007` | **UNDECLARED AND UNOPENED.** Its broad multi-hour span has no minimal message locator. Gap 001 is closed, gap 002 is later design, and gap 003 remains unaccepted rather than triggering a crawl. |
| `RC-CAND-002` | `RC-GAP-008` | **UNDECLARED AND UNOPENED.** No audit-specific provenance defect is demonstrated, and accepted mainline decisions already govern the A/B split. |
| `RC-CAND-003` | No material unresolved A question; task-063 identifies it as task-015 supervision history | **REJECTED AS OUT OF P1 PRODUCT SCOPE; UNOPENED.** Execution failure history cannot establish user intent. |
| `RC-CAND-004` | No material unresolved A question; task-063 identifies it as task-015 retry-supervision history | **REJECTED AS OUT OF P1 PRODUCT SCOPE; UNOPENED.** |
| `RC-CAND-005` | No material unresolved A question; task-063 identifies it as task-015 collision-supervision history | **REJECTED AS OUT OF P1 PRODUCT SCOPE; UNOPENED.** |
| `RC-CAND-006` | No material unresolved A question; task-063 identifies it as memory-system setup history | **REJECTED AS OUT OF P1 PRODUCT SCOPE; UNOPENED.** Document-creation provenance does not answer an A requirement gap. |
| `RC-CAND-007` | No material unresolved A question; task-063 identifies the entries as Stage A review history | **REJECTED AS OUT OF P1 PRODUCT SCOPE; UNOPENED.** Review activity is not direct user intent. |

## 2. Evidence ledger

The records below use the evidence schema and closed classification vocabulary
from `docs/p1-a-recovery/01-source-register.md`, Section 9. They synthesize only
the three merged evidence artifacts; they do not elevate raw-candidate metadata
into requirement evidence.

| Evidence ID | Classification | Source | Locator | Quotation or faithful paraphrase | Relevance to A | Confidence / contradiction notes |
|---|---|---|---|---|---|---|
| `TH-E001` | `INFERENCE` | `docs/p1-a-recovery/02-mainline-evidence.md`; `docs/p1-a-recovery/03-project-history-evidence.md` | Mainline Section 2, `ML-E003`-`ML-E007`; project-history Section 4.1, `PH-E001`-`PH-E004` | Faithful synthesis: direct current and historical user records establish A as a complete, releasable Windows-local product for ordinary users, with an honest all-and-only surface and unpack/start usability. | Closes the provenance and acceptance question in `RC-GAP-001` without raw history. | High confidence. The synthesis preserves the distinction between current direct wording and older historical wording; it does not claim that every historical feature is accepted. |
| `TH-E002` | `UNRESOLVED` | `docs/p1-a-recovery/02-mainline-evidence.md`; `docs/p1-a-recovery/03-project-history-evidence.md`; `docs/p1-a-recovery/04-recovery-corpus-evidence.md` | `ML-E026`, `ML-E028`, `ML-E090`; `PH-E023`-`PH-E025`, `PH-E033`; `RC-E012`, `RC-E021`, `RC-E023`, `RC-GAP-002` | Faithful synthesis: section inspection is accepted, but a standalone endpoint, course-surface delivery, final section key, and wire shape are not selected by P1 evidence. | Separates A's user-visible requirement from later interface design. | High confidence that the design question remains unresolved. It is rejected as a raw-history target because P1 must not make or backfill that later design decision. |
| `TH-E003` | `UNRESOLVED` | `docs/p1-a-recovery/02-mainline-evidence.md`; `docs/p1-a-recovery/03-project-history-evidence.md`; `docs/p1-a-recovery/04-recovery-corpus-evidence.md` | `ML-E032`; `PH-E027`, `PH-E032`; `RC-E005`, `RC-E026`, `RC-GAP-003` | Faithful synthesis: Calendar, Compact view, saved views, and share links remain optional historical UI candidates without admitted direct acceptance. | Prevents unverified UI ideas from entering A's recovered requirement set. | High confidence about non-acceptance; their historical intent remains unresolved because no narrow raw locator exists. |
| `TH-E004` | `ACCEPTED_MAINLINE_DECISION` | `docs/p1-a-recovery/02-mainline-evidence.md` | `ML-E003`, `ML-E043`, `ML-E047`, `ML-E048` | Faithful paraphrase: A is an unpackable ordinary-user Windows package, starts through a BAT launcher, runs locally, and requires no separately installed Node, npm, Rust, SQLite. | Supplies the operative distribution and startup contract missing from the L3 recovery corpus. | High confidence. Approximate filenames and archive layout remain later design/package details. |
| `TH-E005` | `ACCEPTED_MAINLINE_DECISION` | `docs/p1-a-recovery/02-mainline-evidence.md` | `ML-E014`-`ML-E020`, `ML-E027`, `ML-E029`-`ML-E030`, `ML-E051`-`ML-E054` | Faithful paraphrase: active WebUI watches are connection-scoped and capped at nine; every fresh Open message triggers browser audio without debounce; volume and one-shot/continuous modes are required; near-one-second status-to-alert behavior is aspirational and must be validated later. | Supplies A's operative watch and audio contract. | High confidence. Exact wire schema and responsible polling cadence remain later design/validation questions. |
| `TH-E006` | `ACCEPTED_MAINLINE_DECISION` | `docs/p1-a-recovery/02-mainline-evidence.md` | `ML-E020`, `ML-E030`, `ML-E047`, `ML-E053` | Faithful paraphrase: the course directory refresh defaults to ten minutes; A may expose configuration while B ordinary users may not; refresh/polling is centralized rather than browser-direct. | Supplies the current refresh requirement that the recovery corpus lacked. | High confidence. Fine-grained retry, polling, and load behavior remains subject to later design and testing. |
| `TH-E007` | `ACCEPTED_MAINLINE_DECISION` | `docs/p1-a-recovery/02-mainline-evidence.md` | `ML-E008`-`ML-E010`, `ML-E025`, `ML-E056` | Faithful paraphrase: A/B v1 exclude email, SMTP, SendGrid, mail configuration, and unrequested notification forms; active-WebUI browser audio remains the alert channel. | Closes the historical notification-membership conflict for current A. | High confidence. Historical email discussion remains provenance context only and cannot reopen the accepted v1 boundary. |
| `TH-E008` | `INFERENCE` | `docs/p1-a-recovery/02-mainline-evidence.md`; `docs/p1-a-recovery/04-recovery-corpus-evidence.md` | `ML-E011`, `ML-E024`, `ML-E033`-`ML-E045`, `ML-E058`; `RC-E004`, `RC-GAP-008` | Faithful synthesis: accepted evidence already distinguishes Windows-local A from public B and assigns separate composition roots; the merged evidence artifacts identify no concrete provenance defect that requires historical cloud dialogue. | Closes the conditional raw gate for `RC-GAP-008`. | High confidence for this task's access decision. A future audit finding would require its own explicit, newly scoped evidence response; it cannot justify a preemptive raw crawl here. |

## 3. Verified no-access result

The conditional gate in `docs/p1-a-recovery/01-source-register.md`, Section 7,
was evaluated and remained closed:

- exact raw-session external read-only sources declared for task-064: **0**;
- raw Codex or Claude session files opened: **0**;
- archived NGAT conversations, runtime artifacts, transcripts, or worktrees
  inspected: **0**;
- discarded requirements-document versions inspected or compared: **0**;
- raw candidate pointers followed beyond the metadata already present in
  task-063: **0**;
- raw tool traces, secrets, provider or infrastructure details, and unrelated
  private conversation content copied into this artifact: **0**.

No candidate satisfied all raw-access gate conditions. Where metadata was
exact and narrow, no material unresolved gap required it; where a product gap
remained, the locator was too broad. No raw source was present in the task's
external read-only allowlist. The only inputs used for the substantive
comparison were the merged repository artifacts from tasks 061-063. The source
register was consulted only for schema and access rules.

## 4. Gap outcome handoff

| Outcome | Gaps | Requirements-facing result |
|---|---|---|
| **Closed** | `RC-GAP-001`, `RC-GAP-004`, `RC-GAP-005`, `RC-GAP-006`, `RC-GAP-007`, `RC-GAP-008` | Direct/accepted evidence supplies the operative A purpose, package/start contract, watch/audio behavior, refresh behavior, no-email boundary, and A-local/B-public provenance. |
| **Remains unresolved and unaccepted** | `RC-GAP-003` | Calendar, Compact view, saved views, and share links are not recovered as A requirements. Preserve them only as optional historical candidates for current-line review; do not infer acceptance. |
| **Rejected as out of P1 scope** | `RC-GAP-002` | A must support course and section inspection. Endpoint placement, section-key shape, and wire contracts belong to later design and cannot be decided through a broad historical crawl. |

This artifact adds no product requirement beyond the cited evidence, performs
no P2 all-and-only adjudication, and authorizes no transition beyond P1.

## 5. Boundary attestation

- Raw-history access decision: **VERIFIED NO ACCESS**.
- Proposed candidates dispositioned: **7 of 7**.
- Named recovery gaps dispositioned: **8 of 8**.
- External evidence sources modified, copied, staged, or packaged: **0**.
- Repository artifacts written by this task: only
  `docs/p1-a-recovery/05-targeted-history-gap-closure.md`.
