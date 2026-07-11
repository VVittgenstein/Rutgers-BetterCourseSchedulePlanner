# Deliverable A — Windows-Local Release Requirements

> **P1 CANDIDATE - AWAITING MAINLINE REVIEW**

Status: evidence-synthesized candidate; not a final specification
Language: English
Deliverable: A, the Windows-local Rutgers BetterCourseSchedulePlanner release
Decision gate: current-discussion-line P1 Review

## 1. Status, scope, and authority

This document is the single P1 candidate passed to independent audit and then
to the current discussion line. It recovers and organizes requirements; it
does not finalize them, perform the P2 all-and-only surface adjudication, or
authorize P2-P7 work. The current discussion line must correct and accept the
candidate before it becomes Deliverable A's target document. (Evidence:
`ML-E061`, `ML-E062`, `ML-E067`, `ML-E076`, `ML-E080`.)

The words **must** and **shall** below identify candidate requirements supported
by direct User evidence or later accepted mainline decisions. They do not
erase chronology: an older historical expectation, a later accepted decision,
an implementation observation, an inference, a contradiction, and an
unresolved question remain different kinds of evidence. (Evidence: `ML-E089`,
`TH-E001`, `RC-E027`, `RC-E029`.)

### 1.1 Evidence layers used here

| Layer | Treatment in this candidate | Resolving evidence |
|---|---|---|
| Original A history | Preserved as older direct User intent or lower-authority historical context. It explains the release's purpose but cannot override later accepted decisions. | `PH-E001`, `PH-E002`, `PH-E004`, `PH-E005`, `PH-E008`, `PH-E009`, `PH-E010` |
| Later accepted 0A-0C decisions | Operative candidate constraints for A unless the current-line P1 Review changes them. | `ML-E024`, `ML-E025`, `ML-E026`, `ML-E027`, `ML-E029`, `ML-E030`, `ML-E043`, `ML-E044`, `ML-E047`, `ML-E048` |
| Implementation observations | Evidence of drift, stubs, prerequisites, or contradictory documentation; never proof of User intent. | `PH-E016`, `PH-E019`, `PH-E020`, `PH-E021`, `PH-E023`, `PH-E026`, `PH-E028`, `PH-E029` |
| Investigator inference | Explicit synthesis only. An inference remains reviewable and cannot silently become an accepted product decision. | `ML-E089`, `PH-E031`, `RC-E031`, `TH-E001`, `TH-E008` |
| Conflicts and unresolved items | Kept visible for the named later gate; no endpoint, identifier, UI, performance, or package-layout choice is invented here. | `ML-E028`, `ML-E032`, `ML-E048`, `ML-E081`, `ML-E082`, `ML-E083`, `ML-E090`, `TH-E002`, `TH-E003` |

Detailed quotations, hashes, source identities, and message/line or immutable
Git locators remain in the companion evidence artifacts. Section 11 maps every
evidence-ID family used by this candidate to those resolving ledgers.

## 2. Original historical A intent

This section preserves the original layer. It is not rewritten as if the
later Rust, WebSocket, or A/B decisions had existed at the same time.

| Historical intent ID | Recovered historical statement | Authority and qualification | Evidence |
|---|---|---|---|
| `HIST-A-001` | A began as a distinct local first phase and was intended to become a **complete BCSP that could be released**, rather than the later complete refactor or cloud-deployment phase. | Older direct User history. “Complete” states ambition, not a self-executing feature list. | `PH-E001`, `PH-E002` |
| `HIST-A-002` | The distribution was meant for an ordinary person who could unpack it and use it directly, not only for a developer working from the repository. | Older direct User history. The same message mentioned Windows and macOS; current A is later narrowed to Windows-local, and that chronology is retained in Section 8. | `PH-E004`, `ML-E003`, `TH-E001` |
| `HIST-A-003` | The historical goal was not a minimal MVP. The User initially said all functions should be present, then clarified that things which should not appear must also be absent. | Older direct User history. This is the origin of the complete-but-honest boundary, not permission to ship every historical idea. | `PH-E003`, `PH-E008`, `RC-E002`, `RC-E003` |
| `HIST-A-004` | Cancelled, skipped, stubbed, or abandoned behavior had to be checked against persistent history; designed behavior lost because of problems might be recovery, while inappropriate surfaces must not be promoted. | Older direct User history plus explicit inference. Exact feature membership remains P2 work. | `PH-E005`, `PH-E009`, `PH-E031`, `PH-E032` |
| `HIST-A-005` | Incorrect code, contracts, UI, UX, configuration, and documentation were part of the correction problem; rebuilding the UI also meant rebuilding the UX. | Older direct User history. Final screens and interactions were not specified by this evidence. | `PH-E006`, `PH-E010` |
| `HIST-A-006` | A's release surface had to be honest: promised behavior should work, be verifiable, and match the documentation; false or obsolete surfaces should be removed, hidden, or explicitly deferred. | Reaffirmed by current direct User evidence; item-by-item adjudication is deferred to P2. | `ML-E004`, `ML-E005`, `ML-E006`, `RC-E006`, `RC-E015` |
| `HIST-A-007` | The recovered end-to-end purpose was a local data-to-alert workflow: start locally, acquire Rutgers SOC data, store it in SQLite, browse course and section information, watch openings, and alert through local browser sound. | L3 recovery synthesis, corroborated by later accepted capabilities; it is context rather than an independent accepted matrix. | `RC-E009`, `RC-E010`, `RC-E011`, `RC-E012`, `RC-E013`, `RC-E014`, `RC-E031` |

## 3. Later accepted 0A-0C constraints

These decisions post-date the original history and control the current
candidate where they resolve an older ambiguity or supersede a stale
implementation assumption.

| Decision ID | Accepted constraint for A | Evidence |
|---|---|---|
| `DEC-A-001` | A is a Windows-local x64 release direction for a normal user: unpack the package and enter through a BAT launcher without separately installing Node, npm, Rust, SQLite, or a web server. Final archive and binary names remain later packaging decisions. | `ML-E003`, `ML-E043`, `ML-E047`, `ML-E048`, `TH-E004` |
| `DEC-A-002` | A and B share one React/Vite/TypeScript ordinary-user WebUI and one Rust application core wherever deployment permits, while separate Windows-local and Linux-public composition roots structurally control environment-specific capabilities. | `ML-E024`, `ML-E044`, `ML-E045`, `ML-E057` |
| `DEC-A-003` | A's computation, catalog refresh, status polling, storage, WebSocket service, and alert path run locally; its local service binds only to `127.0.0.1`, uses local SQLite, and serves the bundled WebUI. | `ML-E011`, `ML-E047`, `TH-E008` |
| `DEC-A-004` | A/B v1 exclude email, SMTP, SendGrid, mail configuration, and mail-worker delivery. No push, app, or invented system-notification product is substituted; active-WebUI browser audio is the current alert channel. | `ML-E008`, `ML-E009`, `ML-E010`, `ML-E025`, `ML-E056`, `TH-E007` |
| `DEC-A-005` | Watches are explicit active-WebUI sessions with at most nine section keys. Remembered selections are not active watches. Tab/browser close, confirmed connection close, disconnect, or a later-designed liveness timeout releases active-watch state. Lock, backgrounding, or suspension makes reliable audio and monitoring unavailable and may lead to disconnect or timeout; it does not by itself establish immediate watch-state release. Exact heartbeat, timeout, reconnect, and replacement behavior remains P3-P5 design work. | `ML-E012`, `ML-E016`, `ML-E020`, `ML-E027`, `ML-E052`, `ML-E054`, `ML-E090`, `TH-E005` |
| `DEC-A-006` | Every **fresh received Open** status for a watched section triggers browser audio even without a Closed-to-Open transition. There is no debounce; volume and one-shot/continuous modes are required. | `ML-E014`, `ML-E015`, `ML-E029`, `ML-E051`, `ML-E054`, `TH-E005` |
| `DEC-A-007` | The course directory refresh defaults to ten minutes and is locally configurable in A. Rutgers work is centralized in the local core rather than performed independently by browsers. | `ML-E020`, `ML-E021`, `ML-E030`, `ML-E047`, `ML-E053`, `TH-E006` |
| `DEC-A-008` | Near-one-second status-to-alert behavior is an aspiration that later validation must measure; it is not a P1 claim that the current or future implementation already meets a sub-second guarantee. | `ML-E017`, `ML-E030`, `ML-E081`, `ML-E090`, `TH-E005` |
| `DEC-A-009` | P1 recovers the target, P2 decides all-and-only surface membership, P3-P6 design and reconcile A/B, and approved P7 implements, validates, packages, and conditionally publishes. P1 cannot cross those gates. | `ML-E058`, `ML-E061`, `ML-E062`, `ML-E067`, `ML-E068`, `ML-E073`, `ML-E076` |

## 4. Ordinary-user Windows-local journey

The journey below states the supported outcome without choosing unaccepted
screens, endpoint names, message schemas, or file layouts.

| Step | Candidate ordinary-user outcome | Evidence |
|---:|---|---|
| 1 | The user obtains the audited Windows-local package, unpacks it, and does not install a language runtime, package manager, database engine, compiler, or separate web server. | `ML-E003`, `ML-E043`, `ML-E047`, `TH-E004` |
| 2 | The user starts A through the package's BAT entry. The launcher starts the local binary, waits for a healthy service, and opens the default browser; the BAT file is not the business application. | `ML-E003`, `ML-E048` |
| 3 | The browser reaches a WebUI served from the loopback-only local composition, with application data, configuration, and logs kept in an appropriate per-user Windows location. | `ML-E024`, `ML-E047` |
| 4 | The local core acquires and refreshes Rutgers course-directory data, validates staged data before replacement, and presents a usable last-known catalog rather than a half-refreshed catalog. The last-known-catalog preservation wording is a candidate inference from the accepted atomic-replacement rule and must be checked at P1 Review. | `ML-E046`, `ML-E050` |
| 5 | The user can search and filter courses, inspect course and section information, and see section open/closed status without knowing or choosing an API route. | `ML-E026`, `RC-E011`, `RC-E012`, `TH-E002` |
| 6 | The user chooses up to nine sections and explicitly starts watching them in the open WebUI. Remembered selections alone do not activate monitoring. | `ML-E027`, `ML-E052`, `TH-E005` |
| 7 | The local core centrally polls Rutgers, sends fresh watched-section status over the accepted realtime path, and the browser applies the selected volume and one-shot/continuous audio behavior to every fresh Open message without debounce. | `ML-E021`, `ML-E029`, `ML-E053`, `ML-E054`, `TH-E005` |
| 8 | Tab/browser close, confirmed connection close, disconnect, or a later-designed liveness timeout releases active-watch state. Lock, backgrounding, or suspension makes reliable audio and monitoring unavailable and may lead to disconnect or timeout; it does not by itself establish immediate release. Exact heartbeat, timeout, reconnect, and replacement behavior remains P3-P5 design work. | `ML-E012`, `ML-E016`, `ML-E027`, `ML-E052`, `ML-E054`, `ML-E090` |
| 9 | Startup, catalog refresh, Rutgers polling, or realtime failures are reported honestly through user-facing state and local diagnostics; stale Open data must not masquerade as a fresh result or replay queued alerts. Exact error copy and recovery controls remain later design work. | `ML-E046`, `ML-E051`, `ML-E053`, `ML-E054`, `ML-E070` |

## 5. Candidate requirements

### 5.1 Product purpose and user boundary

| Requirement | Candidate requirement | Evidence |
|---|---|---|
| `A-PROD-001` | A shall be a complete, corrected, genuinely releasable local BCSP for ordinary Windows users, not a minimal demonstration or a repository-only developer workflow. | `ML-E003`, `ML-E004`, `ML-E005`, `PH-E002`, `PH-E004`, `RC-E003`, `TH-E001` |
| `A-PROD-002` | Every capability ultimately claimed for the release shall work, be verifiable, and be documented accurately; obsolete entry points, stubs, false UI/docs, and out-of-scope configuration shall not be presented as shipped behavior. | `ML-E005`, `ML-E006`, `PH-E008`, `PH-E010`, `RC-E006` |
| `A-PROD-003` | A shall share B's ordinary-user WebUI and core course-planning behavior where deployment permits, while local configuration and local runtime behavior remain A-specific. | `ML-E010`, `ML-E024`, `ML-E026`, `ML-E044`, `ML-E047`, `ML-E057` |
| `A-PROD-004` | This candidate shall state supported outcomes without deciding the P2 all-and-only membership of unresolved historical surfaces. | `ML-E062`, `ML-E068`, `PH-E032`, `TH-E002`, `TH-E003` |

### 5.2 Package, startup, and local runtime

| Requirement | Candidate requirement | Evidence |
|---|---|---|
| `A-PKG-001` | A shall ship as an unpackable Windows-local x64 package direction suitable for a normal user. The final archive filename and exact internal layout are not fixed by P1. | `ML-E003`, `ML-E043`, `ML-E048`, `TH-E004` |
| `A-PKG-002` | Using A shall not require a separately installed Node, npm, Rust toolchain, SQLite installation, compiler/build tools, or web server. | `ML-E043`, `ML-E047`, `TH-E004` |
| `A-PKG-003` | The package shall expose a BAT entry that starts the local executable, waits until the local health check succeeds, and opens the default browser. The launcher shall remain orchestration only, not contain application business logic. | `ML-E003`, `ML-E048` |
| `A-PKG-004` | The local composition shall bind only to `127.0.0.1`, use local SQLite, serve the bundled React WebUI, expose the local WebSocket path, and run local catalog refresh and openSections polling. | `ML-E047` |
| `A-PKG-005` | Product data, user configuration, and logs shall live under an appropriate per-user Windows location rather than requiring writes beside the unpacked application or into the source tree. The accepted evidence fixes that outcome but not the precise Windows directory or migration policy; those mechanics remain later design choices. | `ML-E047` |
| `A-PKG-006` | The package, release assets, public documentation, and logs shall contain no credentials, provider inventory, private keys, tokens, billing data, or other runtime secrets. | `ML-E038`, `ML-E075`, `ML-E078` |
| `A-PKG-007` | The accepted approximate four-item layout (`bcsp-local.exe`, `start.bat`, user documentation, and license) is direction only; those sources do not fix the exact mechanics, and later packaging design and clean-machine validation shall select and audit the final filenames and layout. | `ML-E048`, `ML-E072` |

### 5.3 Course data, refresh, and persistence

| Requirement | Candidate requirement | Evidence |
|---|---|---|
| `A-DATA-001` | The local core shall acquire Rutgers SOC catalog data through shared Rutgers clients/parsers and persist the searchable course directory in local SQLite. | `ML-E046`, `ML-E050`, `RC-E010` |
| `A-DATA-002` | The persisted catalog shall support the accepted domain information needed for course and section use: terms, subjects, courses, sections/index numbers, instructors, meetings, campus/location relationships, and last-successful-refresh metadata. | `ML-E050`, `RC-E011`, `RC-E012` |
| `A-DATA-003` | A catalog refresh shall build and validate staging data before a short atomic replacement so users never observe a half-refreshed catalog. | `ML-E046`, `ML-E050` |
| `A-DATA-004` | The course-directory refresh interval shall default to ten minutes and shall be configurable for local A. The configuration mechanism and allowed range remain later design questions. | `ML-E020`, `ML-E030`, `ML-E047`, `TH-E006` |
| `A-DATA-005` | Rutgers catalog refresh and openSections polling shall be coordinated in the local core; browsers shall not independently poll Rutgers. Relevant polling shall be single-flight so slow requests do not overlap. | `ML-E021`, `ML-E047`, `ML-E053`, `TH-E006` |
| `A-DATA-006` | Polling shall expose explicit freshness and health and shall handle timeouts plus retry/backoff for upstream 429 and 5xx responses. Exact retry values and responsible openSections cadence are deferred. | `ML-E053`, `ML-E090` |
| `A-DATA-007` | Current open/closed state shall primarily remain in memory. A successful Rutgers result shall create a fresh snapshot with sequence and observation time; a failed request or stale cached Open state shall not trigger fresh audio. | `ML-E051` |
| `A-DATA-008` | With no active watches, status polling may slow; starting the first watch should cause an immediate poll. The final responsible idle/active cadence requires later upstream-behavior validation. | `ML-E053`, `ML-E090` |

### 5.4 Course discovery and section information

| Requirement | Candidate requirement | Evidence |
|---|---|---|
| `A-UI-001` | The ordinary-user WebUI shall support course search and filtering, course inspection, section inspection, and current section status display. | `ML-E026`, `RC-E011`, `RC-E012` |
| `A-UI-002` | Section information shall be a user-visible capability, but P1 shall not choose a standalone `GET /api/sections`, a course-embedded route, a screen structure, or another interface solely because historical documentation or a stub exists. | `PH-E023`, `PH-E024`, `PH-E025`, `RC-E021`, `RC-E023`, `TH-E002` |
| `A-UI-003` | A and B shall use one ordinary-user React WebUI; deployment-specific configuration may differ without creating a second ordinary-user visual product. Final A/B UI treatment is deferred to the mandatory UI design phase. | `ML-E024`, `ML-E026`, `ML-E032`, `ML-E044`, `ML-E074` |
| `A-UI-004` | Later P7.2 UI implementation shall cover loading, empty, error, and interactive states; later P7.3 shall independently polish the integrated UI's interaction, motion, responsiveness, input, reduced-motion, accessibility, and performance behavior. | `ML-E070`, `ML-E071`, `ML-E074` |


### 5.5 Active watches, realtime status, and browser audio

| Requirement | Candidate requirement | Evidence |
|---|---|---|
| `A-WATCH-001` | Watching shall be an explicit action in an open WebUI session. One live session may actively watch at most nine sections. | `ML-E012`, `ML-E020`, `ML-E027`, `ML-E052`, `TH-E005` |
| `A-WATCH-002` | Active watch state shall belong to the live connection and be released when the tab/browser closes, the connection is confirmed closed, a disconnect occurs, or a later-designed liveness timeout expires. Lock, backgrounding, or suspension shall be treated as making reliable audio and monitoring unavailable and may lead to disconnect or timeout, but shall not by itself be specified as immediate watch-state release. Locally remembered choices may aid selection but shall remain inactive until the user explicitly starts watching. Exact heartbeat, timeout, reconnect, and replacement behavior remains P3-P5 design work. | `ML-E012`, `ML-E016`, `ML-E027`, `ML-E052`, `ML-E054`, `ML-E090` |
| `A-WATCH-003` | A watch shall be keyed by a section identity sufficient for the supported Rutgers scope. Index-only identity shall not be assumed until uniqueness is verified; otherwise term, campus, or other context must be included. The exact key is unresolved. | `ML-E028`, `ML-E052`, `ML-E090`, `TH-E002` |
| `A-WATCH-004` | The local composition shall use the accepted WebSocket realtime path for live watch changes and fresh status delivery. Exact message fields, heartbeat, reconnect, and wire compatibility rules remain P3-P5 design work. | `ML-E047`, `ML-E054`, `ML-E090` |
| `A-WATCH-005` | For every fresh received status snapshot, each watched section reported Open shall generate a status message that causes browser audio. Triggering shall not require a Closed-to-Open transition. | `ML-E015`, `ML-E029`, `ML-E051`, `ML-E054`, `TH-E005` |
| `A-WATCH-006` | Audio behavior shall have no debounce and shall provide user controls for volume and one-shot versus continuous playback. | `ML-E014`, `ML-E029`, `TH-E005` |
| `A-WATCH-007` | Browser audio shall have no email fallback. Audio is not guaranteed after tab/browser close, disconnect, device/browser lock, backgrounding, or suspension. | `ML-E009`, `ML-E016`, `ML-E025`, `ML-E027`, `TH-E007` |
| `A-WATCH-008` | Realtime delivery shall be latest-value or lag-aware so a slow or reconnected browser does not accumulate and later play a queue of stale one-second snapshots. | `ML-E051`, `ML-E054` |
| `A-WATCH-009` | The implementation shall treat near-one-second status-to-alert behavior as a target to measure under responsible Rutgers polling, not as an unmeasured guarantee or a P1 implementation claim. | `ML-E017`, `ML-E030`, `ML-E081`, `ML-E090`, `TH-E005` |

### 5.6 Local configuration, data, and privacy

| Requirement | Candidate requirement | Evidence |
|---|---|---|
| `A-CFG-001` | A shall expose local configuration for the course-directory refresh interval, with ten minutes as the default. Exact configuration format, validation, and safe bounds remain later design work. | `ML-E020`, `ML-E030`, `ML-E047`, `TH-E006` |
| `A-CFG-002` | A shall keep application data, configuration, and logs in an appropriate per-user Windows location. The accepted evidence does not fix the exact location or mechanics; the eventual implementation shall define that location and any upgrade/migration behavior explicitly. | `ML-E047` |
| `A-CFG-003` | A shall not expose email, SMTP, SendGrid, mail-worker, or mail-configuration settings in v1. | `ML-E008`, `ML-E009`, `ML-E025`, `ML-E056`, `TH-E007` |
| `A-CFG-004` | Remembering section selections locally may be offered for convenience, but persisted selection shall never be represented as an active watch until a live WebUI session starts it. | `ML-E027`, `ML-E052` |
| `A-CFG-005` | Configuration, data, packages, logs, screenshots, and public documentation shall not contain deployment credentials, private keys, API tokens, billing data, or secret material. | `ML-E038`, `ML-E075`, `ML-E078` |

### 5.7 Failure handling, recovery, and diagnostics

The accepted evidence fixes several safety outcomes but does not specify every
error string, retry constant, log schema, or recovery control. Rows explicitly
marked **candidate inference** require confirmation at P1 Review.

| Requirement | Candidate requirement | Treatment | Evidence |
|---|---|---|---|
| `A-ERR-001` | The BAT launcher shall wait for a healthy local service before opening the browser and shall not present a failed startup as success. | Accepted health-gated launch behavior. | `ML-E046`, `ML-E048` |
| `A-ERR-002` | If startup does not become healthy, the ordinary user should receive an actionable failure indication and a pointer to the per-user diagnostic log rather than a silent exit. Exact wording, timeout, and retry controls remain later design work. | **Candidate inference** from health gating, local logs, honest claims, and required error states. | `ML-E005`, `ML-E046`, `ML-E047`, `ML-E048`, `ML-E070` |
| `A-ERR-003` | A failed catalog validation shall not replace the live catalog with partial staging data. Retaining and retrying from the last successful catalog is the candidate recovery interpretation; exact retention and rollback semantics require later confirmation. | First sentence accepted; second sentence **candidate inference**. | `ML-E050` |
| `A-ERR-004` | Upstream timeouts, 429 responses, and 5xx responses shall use retry/backoff and explicit freshness/health reporting; a failed request or stale cached Open value shall not generate a fresh alert. | Accepted failure/freshness behavior. | `ML-E051`, `ML-E053` |
| `A-ERR-005` | Tab/browser close, confirmed connection close, disconnect, or a later-designed liveness timeout shall end active-watch state. Lock, backgrounding, or suspension makes reliable audio and monitoring unavailable and may lead to disconnect or timeout, but does not by itself establish immediate watch-state release. The UI should make inactive, disconnected, stale, and recovering states distinguishable rather than imply that monitoring remains live. Exact heartbeat, timeout, reconnect, and replacement policy remains unresolved. | Confirmed connection-close behavior is accepted; the suspension consequence and visible-state wording are **candidate inference** from the accepted audio limitation and honest UI/error-state requirements. | `ML-E016`, `ML-E027`, `ML-E052`, `ML-E054`, `ML-E070`, `ML-E090` |
| `A-ERR-006` | Local diagnostics shall support health and freshness investigation and shall be written under the per-user log location without credentials. The cited sources do not fix the exact mechanics; format, retention, redaction, and support bundle behavior remain later design choices. | Supported outcome with unresolved mechanics. | `ML-E038`, `ML-E046`, `ML-E047`, `ML-E078` |
| `A-ERR-007` | The finished UI shall include truthful loading, empty, error, and interactive states for the shipped flow; documentation shall not instruct users to rely on behavior the package cannot perform. | Accepted P7 UI and historical truth-in-claims requirement. | `ML-E005`, `ML-E006`, `ML-E070`, `RC-E006`, `RC-E015` |

### 5.8 Documentation, validation, and release expectations

| Requirement | Candidate requirement | Evidence |
|---|---|---|
| `A-VAL-001` | User documentation shipped with A shall accurately describe the actual package, BAT startup, ordinary course-to-watch flow, local configuration, relevant failure/diagnostic path, and known alert limitations. Unsupported capabilities and obsolete prerequisites shall not be advertised. | `ML-E005`, `ML-E006`, `ML-E048`, `RC-E006`, `RC-E015`, `PH-E019`, `PH-E029` |
| `A-VAL-002` | A shall be validated on Windows as an unpack-and-start package on a clean machine without preinstalled Node, npm, Rust, SQLite, native build tooling, or a separate web server. | `ML-E003`, `ML-E043`, `ML-E047`, `PH-E019`, `PH-E020`, `PH-E021`, `PH-E022`, `RC-E007` |
| `A-VAL-003` | Validation shall exercise the promised ordinary-user flow, including startup/health, Rutgers catalog acquisition and atomic refresh, search/filter, course and section inspection, active-watch limits and lifecycle, fresh-Open audio semantics and controls, upstream failure/freshness behavior, and local diagnostics. | `ML-E005`, `ML-E026`, `ML-E029`, `ML-E046`, `ML-E050`, `ML-E051`, `ML-E052`, `ML-E053`, `ML-E072` |
| `A-VAL-004` | The near-one-second aspiration shall be measured later under defined conditions; release claims shall report observed behavior and limitations rather than assert an unverified sub-second guarantee. | `ML-E030`, `ML-E081`, `ML-E090`, `TH-E005` |
| `A-VAL-005` | P7.4 shall run functional, contract, browser, package, security, and capacity checks, then audit the Windows package and its documentation and checksums. Package/release audit shall include a secret and private-artifact check. | `ML-E072`, `ML-E075`, `ML-E077`, `ML-E078` |
| `A-VAL-006` | P7 work shall produce clean, substantive commits on the approved remote path. P7.2 UI implementation and P7.3 polish shall remain separate sequential tasks, completion records, and commit series. | `ML-E074`, `ML-E077` |
| `A-VAL-007` | The audited Windows package is a mandatory P7 output. GitHub Release publication is conditional on feasibility, safety, and P6 approval; absence of publication does not remove the package requirement. | `ML-E064`, `ML-E072`, `ML-E073`, `ML-E075` |
| `A-VAL-008` | P7 shall build and audit a new Windows package against the accepted target. **Candidate provenance/validation inference:** historical release archives, names, tags, or deleted GitHub assets shall not be reused as proof that the new candidate is release-ready. | `ML-E072`, `ML-E075`, `PH-E016`, `PH-E018` |
| `A-VAL-009` | P1 and P6 shall return to the current discussion line at their gates, and final product/release acceptance remains with that line. | `ML-E066`, `ML-E067`, `ML-E068`, `ML-E076` |

## 6. Implementation observations that motivate correction

The following records describe historical or current project state. They are
not requirements by themselves and are not a P2 keep/remove decision.

| Observation | What the evidence demonstrates | Requirements-facing significance | Evidence |
|---|---|---|---|
| `OBS-A-001` | The tracked Windows launcher exits when Node is absent and opens the Node download page; the one-click script can install npm dependencies, rebuild `better-sqlite3`, and request Microsoft C++ Build Tools. | This conflicts with ordinary-user unpack-and-use intent and motivates clean-machine/no-separate-runtime validation; it does not dictate the new packaging implementation. | `PH-E019`, `PH-E020`, `PH-E021`, `PH-E022` |
| `OBS-A-002` | A documented detailed section endpoint exists beside an implementation that always returns an empty result. | A must not advertise a non-working section surface, but P1 does not choose whether section information uses that endpoint or another accepted surface. | `PH-E023`, `PH-E024`, `PH-E025`, `TH-E002` |
| `OBS-A-003` | The mounted historical UI includes mail settings, while tracked notification documents disagree about email-only versus local sound behavior. | This is drift evidence only; later accepted decisions remove mail and retain active-WebUI audio. | `PH-E026`, `PH-E029`, `PH-E030`, `TH-E007` |
| `OBS-A-004` | Historical package forms diverged in layout and content, including scheduled-refresh material absent from the then-canonical tree; none was safe to promote as canonical. | A newly audited package is required. The old scheduled-refresh implementation is not automatically revived, even though current accepted requirements independently require directory refresh. | `PH-E016`, `PH-E017`, `TH-E006` |
| `OBS-A-005` | The existing browser-local sound hook demonstrates an audio path but not the accepted fresh-message trigger, volume, continuous mode, or final UX. | Implementation presence cannot substitute for validation against the later accepted audio contract. | `PH-E028`, `ML-E029`, `TH-E005` |
| `OBS-A-006` | The broad task-015 matrix was unmerged and never passed provenance review. | It remains a discovery aid only and cannot supply feature membership or validate this candidate. | `PH-E014`, `PH-E015`, `RC-E018` |

## 7. Explicit candidate inferences

These inferences connect accepted outcomes where the evidence does not provide
a fully specified requirement. They are intentionally visible for P1 Review.

| Inference | Candidate synthesis | Supporting evidence |
|---|---|---|
| `INF-A-001` | The best concise statement of A's historical purpose is an ordinary-user local data-to-alert workflow inside a complete and honest Windows release. This summarizes evidence; it is not an independently accepted feature matrix. | `RC-E031`, `TH-E001`, `ML-E003`, `ML-E005` |
| `INF-A-002` | Atomic staging implies that a failed refresh should leave the last successful catalog usable and retry later, but exact retention, rollback, and retry policy must be designed and validated later. | `ML-E050` |
| `INF-A-003` | Health-gated startup plus per-user diagnostics implies an ordinary-user failure message that identifies where to find logs; exact launcher UX is not yet accepted. | `ML-E046`, `ML-E047`, `ML-E048`, `ML-E070` |
| `INF-A-004` | The accepted no-separate-runtime contract and observed Node/native-build failures imply a clean-machine Windows test, even though the evidence does not prescribe a specific VM image or test harness. | `ML-E043`, `ML-E047`, `PH-E019`, `PH-E020`, `PH-E021`, `PH-E022` |
| `INF-A-005` | Honest active-session behavior implies that the UI should distinguish disconnected/stale/recovering state from actively monitored state; exact labels and reconnect interaction remain UI/realtime design work. | `ML-E027`, `ML-E052`, `ML-E054`, `ML-E070` |

## 8. Conflicts and later corrections

| Conflict or correction | Evidence sides | Candidate treatment |
|---|---|---|
| Historical platform breadth versus current A | Older history asked for Windows and macOS unpack-and-use (`PH-E004`); the current direct and accepted target is Windows-local (`ML-E003`, `ML-E043`, `ML-E047`). | A is currently Windows-local. macOS is not claimed as a validated A deliverable; the older expectation remains visible history rather than being silently erased. |
| "All functions" versus an honest all-and-only surface | Broad completeness appears in `PH-E003`; later historical wording requires inappropriate things to be absent and fuzzy candidates to be investigated (`PH-E005`, `PH-E008`, `PH-E009`, `PH-E031`); current wording assigns item membership to P2 (`ML-E006`, `ML-E062`). | Preserve the complete-release ambition without promoting every historical idea. P2, after P1 Review, decides the all-and-only surface. |
| Ordinary-user packaging versus tracked prerequisites | Ordinary unpack/use intent appears in `PH-E004` and current accepted packaging in `ML-E043`/`ML-E047`; tracked launch paths require Node/npm/native build support (`PH-E019`-`PH-E022`). | The accepted no-separate-runtime packaging constraint controls; old prerequisites are drift to correct and regression-test, not release requirements. |
| Historical email surfaces versus current v1 | Historical docs/UI contain email and disagree on behavior (`PH-E029`, `PH-E030`, `RC-E016`, `RC-E022`); accepted 0A/0C removes mail (`ML-E025`, `ML-E056`, `TH-E007`). | Current A/B v1 has browser audio and no email/mail configuration. Historical mail remains provenance context only. |
| Earlier Node/OCI proposal versus accepted architecture | An earlier assistant proposal used OCI and Node/Fastify; accepted 0B/0C use Vultr for B and a shared Rust modular monolith (`ML-E082`). | Rust/shared-core/local-composition decisions control. OCI, Node/Fastify, and old hosting assumptions are not A requirements. |
| Earlier hard one-second wording versus later aspiration | Earlier wording was stricter; later direct User wording and accepted 0A make the status-to-alert goal aspirational (`ML-E081`). | Do not claim a guarantee. Define and run later validation, then report measured behavior. |
| Section contract versus implementation | Documentation describes detailed section results while the route is empty (`PH-E025`, `RC-E021`); accepted evidence requires the user outcome but leaves interface shape open (`TH-E002`). | Require section inspection; do not select or advertise an endpoint in P1. |
| P7 deployment scope | A side-branch expanded P7 into Vultr hardening/deployment; the User corrected the workflow so P7 ends with packages and the current line owns production deployment (`ML-E083`). | Production Vultr deployment is outside this A candidate and outside the NGAT P7 package phase. |
| Workflow status header | The workflow file says "draft for discussion," while the source register and accepted context treat its content as authoritative (`ML-E084`). | Use the accepted workflow content for this candidate and retain the stale header discrepancy for current-line review. |

## 9. Exclusions and deferred candidates

### 9.1 Excluded from current A/B v1

| Exclusion | Boundary | Evidence |
|---|---|---|
| `EX-A-001` | Email reminders, SMTP, SendGrid, mail workers, mail configuration, and email fallback are excluded. | `ML-E008`, `ML-E009`, `ML-E025`, `ML-E056`, `TH-E007` |
| `EX-A-002` | Push, standalone app, invented system-notification products, browser-direct Rutgers polling, and per-user Rutgers pollers are excluded. | `ML-E010`, `ML-E021`, `ML-E056` |
| `EX-A-003` | User accounts and persistent server-side subscriptions are outside v1; active watches remain connection-scoped. | `ML-E027`, `ML-E052`, `ML-E056` |
| `EX-A-004` | Redis, PostgreSQL, brokers, microservices, Kubernetes, mandatory Docker, multiple B replicas, Tauri/Electron solely for packaging, and a universal binary that can leak A-only routes into B are outside the accepted v1 architecture. | `ML-E056`, `ML-E057` |
| `EX-A-005` | Production Vultr hardening/deployment, domain/TLS choice, Caddy/Cloudflare cutover, and B capacity promises are not Deliverable A requirements and are not P1 work. | `ML-E039`, `ML-E040`, `ML-E042`, `ML-E083` |

### 9.2 Not promoted by P1

| Deferred or unaccepted candidate | P1 disposition | Evidence |
|---|---|---|
| Calendar, Compact view, saved views, share links, and presets | Historical UI candidates without admitted direct acceptance. Leave for current-line/P2/UI review; do not claim or remove them here. | `PH-E027`, `PH-E032`, `RC-E026`, `TH-E003` |
| Standalone `GET /api/sections` | The product outcome is course and section inspection; route selection is later design. | `PH-E023`, `PH-E024`, `PH-E025`, `RC-E023`, `TH-E002` |
| Exact section identity/key | Unresolved until Rutgers uniqueness is verified. | `ML-E028`, `ML-E052`, `ML-E090`, `TH-E002` |
| Old scheduled-refresh implementation | Do not promote stale package/branch code. The independently accepted capability is a ten-minute default directory refresh configurable in A. | `PH-E017`, `RC-E025`, `TH-E006` |
| Discord or other historical notification candidates | No accepted current-v1 membership; do not substitute them for browser audio. | `PH-E032`, `ML-E010`, `TH-E007` |
| Historical release archives, deleted tags/assets, and task-015/`5714a8f` classifications | Historical state or discovery aids only; none is a canonical package or accepted requirement matrix. | `PH-E014`, `PH-E015`, `PH-E016`, `PH-E018`, `RC-E018` |
| Exact visual treatment | Mandatory later P7.2 implementation and P7.3 polish remain separate; P1 does not design their screens or aesthetics. | `ML-E032`, `ML-E070`, `ML-E071`, `ML-E074` |

## 10. Unresolved questions for named later gates

| Unresolved ID | Question intentionally left open | Owning later gate | Evidence |
|---|---|---|---|
| `UNR-A-001` | What exact section identity is unique and stable for the supported term/campus scope? | P2/P3 data and interface design | `ML-E028`, `ML-E052`, `ML-E090`, `TH-E002` |
| `UNR-A-002` | What are the exact WebSocket message schema, versioning, heartbeat, liveness, reconnect, and replacement semantics? | P3-P5 contract design | `ML-E032`, `ML-E054`, `ML-E090` |
| `UNR-A-003` | What openSections cadence, adaptive policy, timeout/backoff constants, and upstream-rate evidence are responsible while pursuing the latency aspiration? | P3/P5 design and P7 validation | `ML-E032`, `ML-E053`, `ML-E090` |
| `UNR-A-004` | Under what defined start/end timestamps, machine conditions, network conditions, and load will status-to-alert latency be measured, and what observed distribution is acceptable? | P6 approval and P7 validation | `ML-E030`, `ML-E081`, `ML-E090`, `TH-E005` |
| `UNR-A-005` | What A resource/capacity thresholds and B shared-core load thresholds must pass? The 50-user/450-watch B figures are a hypothesis, not an A promise. | P4-P6 planning and P7 capacity testing | `ML-E036`, `ML-E055`, `ML-E090` |
| `UNR-A-006` | What final A/B UI treatment, local-configuration placement, accessibility detail, and responsive behavior will the shared WebUI use? | P7.2 UI implementation and P7.3 polish | `ML-E032`, `ML-E070`, `ML-E071`, `ML-E074` |
| `UNR-A-007` | What exact crate/module split, checked frontend-contract mechanism, and internal ownership boundaries implement the accepted modular monolith? The cited sources select the direction but do not fix those mechanics. | P3-P5 design | `ML-E045`, `ML-E046` |
| `UNR-A-008` | What are the final archive name, executable name, package tree, per-user Windows directory, upgrade/migration policy, launcher timeout, and diagnostic/log format? The cited sources select approximate outcomes but do not fix those mechanics. | P3 design, P6 review, and P7 packaging validation | `ML-E047`, `ML-E048` |
| `UNR-A-009` | Which optional historical product candidates belong in the all-and-only release surface? | Current-line P1 Review followed by P2 | `PH-E032`, `PH-E033`, `TH-E003`, `ML-E062` |

## 11. Compact evidence traceability index

Every material requirement row above names one or more stable evidence IDs.
The companion record for each ID contains the evidence classification, exact
source identity, and a precise message/line, section/line, JSON pointer, or
immutable commit/blob locator.

| Evidence prefix | Resolving reviewed companion | Locator-bearing section | Use in this candidate |
|---|---|---|---|
| `ML-E001`-`ML-E090` | [`p1-a-recovery/02-mainline-evidence.md`](p1-a-recovery/02-mainline-evidence.md) | Sections 2-7; each `ML-E*` row includes a source and precise locator | Current direct User evidence, accepted 0A-0C/workflow decisions, conflicts, inference, and unresolved current questions |
| `PH-E001`-`PH-E033` | [`p1-a-recovery/03-project-history-evidence.md`](p1-a-recovery/03-project-history-evidence.md) | Section 4; each `PH-E*` row includes an exact transcript locator or full commit/blob/line locator | Original direct history, historical summaries, implementation observations, drift conflicts, inference, and unresolved membership |
| `RC-E001`-`RC-E031` | [`p1-a-recovery/04-recovery-corpus-evidence.md`](p1-a-recovery/04-recovery-corpus-evidence.md) | Section 3; each `RC-E*` row includes an exact registered file and line/JSON locator | Lower-authority recovery context, observations, conflicts, gaps, and cautious synthesis |
| `TH-E001`-`TH-E008` | [`p1-a-recovery/05-targeted-history-gap-closure.md`](p1-a-recovery/05-targeted-history-gap-closure.md) | Section 2; each `TH-E*` row resolves to named upstream evidence records | Verified no-access synthesis, accepted gap closures, and remaining unaccepted design/UI questions |

### 11.1 Topic-to-evidence shortcut

| Review topic | Primary evidence IDs |
|---|---|
| Historical purpose, ordinary users, completeness, and honest correction | `ML-E003`, `ML-E004`, `ML-E005`, `ML-E006`, `PH-E001`, `PH-E002`, `PH-E004`, `PH-E008`, `TH-E001` |
| Windows package, BAT launch, local composition, and data paths | `ML-E043`, `ML-E044`, `ML-E047`, `ML-E048`, `TH-E004` |
| Course data, refresh, search, and section inspection | `ML-E026`, `ML-E046`, `ML-E050`, `ML-E053`, `RC-E010`, `RC-E011`, `RC-E012`, `TH-E002`, `TH-E006` |
| Active watches, realtime status, and audio | `ML-E027`, `ML-E029`, `ML-E051`, `ML-E052`, `ML-E054`, `TH-E005` |
| Failure, diagnostics, documentation, and truth in claims | `ML-E005`, `ML-E046`, `ML-E048`, `ML-E051`, `ML-E053`, `ML-E070`, `RC-E006`, `RC-E015` |
| Validation, packaging audit, commits, and release handoff | `ML-E072`, `ML-E073`, `ML-E074`, `ML-E075`, `ML-E077`, `ML-E078`, `PH-E016`, `PH-E022` |
| Exclusions, conflicts, and unresolved later decisions | `ML-E025`, `ML-E028`, `ML-E032`, `ML-E056`, `ML-E081`, `ML-E082`, `ML-E083`, `ML-E090`, `PH-E032`, `TH-E003` |

## 12. P1 handoff statement

This is a **P1 candidate awaiting mainline review**, not a final A
specification. It preserves the historical complete-release purpose, applies
the later accepted Windows-local/shared-Rust/no-mail/watch/audio/refresh
constraints, records implementation drift without turning it into intent,
labels inference, and keeps conflicts and unresolved design questions visible.
No statement in this document authorizes P2 or any implementation phase.
(Evidence: `ML-E058`, `ML-E061`, `ML-E067`, `ML-E076`, `ML-E089`.)
