# Deliverable A Recovery-Corpus Evidence

Status: recovery-corpus evidence pass complete; requirements remain candidates
for later synthesis and P1 Review

Scope: historical goals, constraints, purpose, expectations, user-attributed
statements, contradictions, and uncertainty concerning Deliverable A, the
Windows-local release

Authority: `L3 HISTORICAL_SUMMARY` unless an evidence record explicitly uses a
different classification for an observation, conflict, inference, or gap

## 1. Method and hard boundary

This pass used the schema and access rules in
`docs/p1-a-recovery/01-source-register.md`. Before content use, the raw-byte
SHA-256 of each of the 20 declared recovery-corpus files was recomputed; all 20
matched the registered value. Every declared file was then read end-to-end.
The three large JSON candidate indexes were parsed in full, and their complete
candidate arrays and schemas were checked rather than sampled.

No raw `.codex` or `.claude` session JSONL was opened. Paths to raw sessions
below are discovery metadata only and remain conditional candidates for
task-064. Embedded `user_previews`, `assistant_previews`, and mention previews
in the JSON indexes were not promoted into claims. No archived NGAT runtime or
discarded requirements artifact was opened. Recovery-corpus statements that
were themselves derived from those prohibited surfaces are excluded from
substantive use.

Line locators in this artifact are one-based and inclusive. JSON array indexes
in JSON Pointers are zero-based. `Included` means that at least some content is
admissible for the stated limited purpose; it does not elevate a recovery
summary to direct-user evidence.

### Classification result

- `DIRECT_USER`: none. The corpus contains user-attributed summaries and
  discovery-only preview strings, but no admissible exact user-authored record
  with a message/line locator.
- `ACCEPTED_MAINLINE_DECISION`: none. Current-line decisions are outside this
  source family and must retain their own evidence IDs.
- Recovery narratives remain `HISTORICAL_SUMMARY`, even when they say that a
  user requested, rejected, or accepted something.
- Index facts and recovery-state fields may be
  `IMPLEMENTATION_OBSERVATION`; they locate evidence but do not establish a
  product requirement.
- Investigator synthesis is explicitly `INFERENCE`; disagreements and missing
  answers are explicitly `CONFLICT` or `UNRESOLVED`.

## 2. Complete source coverage ledger

| Registered source | Exact file and precise locator | Coverage decision | Contribution or exclusion reason |
|---|---|---|---|
| `SRC-REC-00-STATUS` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\00-status.md`, lines 1-29 | **Included — provenance/uncertainty only** | Records the recovery snapshot, candidate counts, unaccepted matrix status, and then-current operational uncertainty. Its runtime advice is historical, not an A requirement. |
| `SRC-REC-01-SITE-MODEL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\01-site-model.md`, lines 1-19 | **Included — boundary context only; excluded from A claims** | Establishes that protected tool state and raw sessions were evidence surfaces, not ordinary project files. It supplies no A product requirement. |
| `SRC-REC-02-SOURCE-INVENTORY` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\02-source-inventory.md`, lines 1-26 | **Included — provenance only** | Accounts for inputs used by the earlier recovery. Assertions sourced from archived NGAT runtime (lines 18-19) are excluded under the current quarantine. |
| `SRC-REC-03-CODEX-SESSIONS` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\03-codex-sessions.md`, lines 1-37; A-facing summary at lines 19-27 | **Included — substantive historical summary** | Supplies the recovered product baseline, complete-local-release direction, negative scope statements, and execution/provenance caution. Raw session paths at lines 5 and 14 are pointers only. |
| `SRC-REC-04-CLAUDE-SESSIONS` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\04-claude-sessions.md`, lines 1-26 | **Included — historical/candidate context only** | Identifies deployment, memory, review, and packaging-history candidates. No candidate was followed, and unrelated-project near-misses at lines 24-26 are excluded. |
| `SRC-REC-05-PROJECT-FILES` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\05-project-files.md`, lines 1-38; A-facing summary at lines 22-26 and 34-37 | **Included — substantive historical summary plus provenance caution** | Describes the then-observed product surface, documentation drift, and failed provenance of the unmerged matrix. Archive-derived runtime assertions at lines 28-32 are excluded. |
| `SRC-REC-06-TIMELINE` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\06-timeline.md`, lines 1-31; A-facing statement at line 12 | **Included — substantive historical summary** | Dates the complete-local-release, honest-surface, Windows-validation, and macOS-claim expectations. June runtime/archive statements at lines 23-26 are excluded from substantive use. |
| `SRC-REC-07-CONFLICTS` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\07-conflicts-and-corrections.md`, lines 1-12 | **Included — conflicts and attributed-summary context** | Preserves endpoint, email, provenance, and process contradictions. The user attribution at line 9 is a summary, not direct dialogue. |
| `SRC-REC-08-CURRENT-STATE` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\08-current-state.md`, lines 1-49; capability summary at lines 22-40 | **Included — substantive historical summary** | Supplies the clearest recovered A capability list and four explicit product questions. Runtime/package claims at lines 42-47 are excluded. |
| `SRC-REC-09-NEXT-STEPS` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\09-next-steps.md`, lines 1-34 | **Included — provenance/gap context only** | Records stale continuation advice and a warning not to treat the matrix as accepted. Its raw/archive pointers are not followed, and its instructions are not current authority. |
| `SRC-REC-10-CRITICAL-DIALOGUES` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\10-critical-dialogues.md`, lines 1-15 | **Included — candidate metadata only** | Correlates selected session IDs, paths, spans, read levels, and uncertainties. All JSONL entries remain unopened task-064 candidates; Git/runtime rows are not imported as requirement evidence. |
| `SRC-REC-CLAUDE-INDEX-JSON` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\claude-session-candidate-index.json`, `/counts`, `/candidates/0`, `/candidates/1`, `/candidates/2`, `/candidates/3`, `/candidates/7`, `/candidates/8`; full array `/candidates` parsed | **Included — metadata only; previews excluded** | Accounts for 64 Claude candidates and gives exact pointers for narrative-named sessions. Preview fields do not establish requirements. |
| `SRC-REC-CLAUDE-INDEX-MD` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\claude-session-candidate-index.md`, lines 1-86 | **Included — metadata only** | Human-readable inventory of all 64 Claude candidates; only path/session/time/CWD/match metadata is usable. |
| `SRC-REC-CODEX-INDEX-JSON` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\codex-session-candidate-index.json`, `/counts`, `/candidates/0` through `/candidates/3`; full array `/candidates` parsed | **Included — metadata only; previews excluded** | Accounts for 95 Codex candidates and precisely locates the four Codex sessions selected by the narratives. Preview fields do not establish requirements. |
| `SRC-REC-CODEX-INDEX-MD` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\codex-session-candidate-index.md`, lines 1-117 | **Included — metadata only** | Human-readable inventory of all 95 Codex candidates. Every listed JSONL remains unopened. |
| `SRC-REC-FORBIDDEN-ACTIONS` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\forbidden-actions.md`, lines 1-33 | **Included — boundary context only; excluded from A claims** | Corroborates that sessions/tool state were protected and writes were confined to the recovery report. It supplies no product requirement. |
| `SRC-REC-MANIFEST` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\manifest.json`, `/phase`, `/finalized`, `/sources`, `/session_index`, `/verified_claims` (text lines 7-36) | **Included — provenance/quality metadata only** | Shows that the report was in synthesis and not finalized, and records recovery/index counts. Archive-derived verified-claim fields are excluded. |
| `SRC-REC-READ-LOG` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\read-log.jsonl`, lines 1-25 | **Included — provenance audit only** | Enumerates what the earlier recovery read. Lines 22-24 reveal runtime/archive ancestry, so the resulting runtime assertions are excluded; this pass did not open those underlying files. |
| `SRC-REC-SESSION-INDEX-JSON` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\session-candidate-index.json`, `/counts`, `/candidates/0` through `/candidates/3`, `/candidates/95` through `/candidates/103`; full array `/candidates` parsed | **Included — metadata only; previews excluded** | Combined inventory of 159 candidates. Its path set exactly equals the 95 Codex plus 64 Claude paths, with no missing, extra, or duplicate path and zero recorded parse errors. |
| `SRC-REC-SESSION-INDEX-MD` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\session-candidate-index.md`, lines 1-23 | **Included — metadata only** | Confirms the combined counts and delegates candidate detail to the two source-specific indexes. |

Coverage result: **20 of 20 declared sources accounted for**. Eleven
narratives were read in full, nine index/governance files were read in full,
and the substantive/metadata/exclusion boundary is explicit for every source.

## 3. Atomic evidence ledger

The wording in the fourth column is either a short source phrase or is
explicitly marked as a faithful paraphrase. A phrase quoted from a recovery
narrative is the narrative author's wording; it is not thereby a user quote.

| Evidence ID | Classification | Source and precise locator | Quotation or faithful paraphrase | Relevance to A | Confidence / contradiction notes |
|---|---|---|---|---|---|
| `RC-E001` | `HISTORICAL_SUMMARY` | `SRC-REC-03-CODEX-SESSIONS`, `03-codex-sessions.md` lines 19-24; corroborated by `SRC-REC-05-PROJECT-FILES`, lines 34-37 | Faithful paraphrase: the recovered product ran a local SOC-to-SQLite-to-API-to-React flow with subscriptions, open-seat polling, and sound/email notification paths. | Establishes the inherited end-to-end surface that A was expected to reconcile. | L3 summary of implementation/history, not proof of user intent or current behavior. |
| `RC-E002` | `HISTORICAL_SUMMARY` | `SRC-REC-03-CODEX-SESSIONS`, lines 23-25; `SRC-REC-08-CURRENT-STATE`, lines 22-24 | Source phrase: “complete local release.” | States the central recovered goal for A. | Repeated across two narratives, but both belong to the same L3 recovery corpus; seek L1/L2 provenance. |
| `RC-E003` | `HISTORICAL_SUMMARY` | `SRC-REC-03-CODEX-SESSIONS`, line 24 | Faithful paraphrase: the target was not a minimal MVP. | Sets a historical completeness expectation. | L3 only; “complete” still needs an accepted, testable feature boundary. |
| `RC-E004` | `HISTORICAL_SUMMARY` | `SRC-REC-03-CODEX-SESSIONS`, line 24; `SRC-REC-04-CLAUDE-SESSIONS`, lines 15-17 | Faithful paraphrase: the then-opened phase targeted a local release rather than cloud deployment; an earlier Claude deployment discussion was background. | Separates historical A delivery from a cloud-hosting direction. | L3 chronology; later current-line A/B decisions supersede it and retain higher authority. |
| `RC-E005` | `HISTORICAL_SUMMARY` | `SRC-REC-03-CODEX-SESSIONS`, line 24 | Faithful paraphrase: the release was not to be a blind continuation of the current UI. | Preserves the expectation that A's WebUI surface had to be deliberately reviewed. | L3 only; it does not identify which UI features were required. |
| `RC-E006` | `HISTORICAL_SUMMARY` | `SRC-REC-06-TIMELINE`, line 12 | Source phrase: “honest feature surface.” | Requires release claims and visible features to match what actually works. | L3 summary; exact acceptance tests were not preserved here. |
| `RC-E007` | `HISTORICAL_SUMMARY` | `SRC-REC-06-TIMELINE`, line 12 | Faithful paraphrase: the package was expected to be validated on Windows. | Directly bears on A's release platform and validation expectation. | L3 only; package mechanics and validation matrix are absent from this corpus. |
| `RC-E008` | `HISTORICAL_SUMMARY` | `SRC-REC-06-TIMELINE`, line 12 | Faithful paraphrase: macOS could be prepared but must not be claimed as validated. | Preserves historical truth-in-claims discipline and a non-Windows limit. | L3 historical scope; current Deliverable A is Windows-local, so this is context rather than a current A deliverable. |
| `RC-E009` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, lines 22-27 | Faithful paraphrase: A should support local setup and start. | First recovered ordinary-user capability. | L3 summary; no unpacking, launcher, privilege, or dependency-install detail is supplied. |
| `RC-E010` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, lines 24-27 | Faithful paraphrase: A should fetch Rutgers SOC data and import it into SQLite. | Recovers the data-acquisition and local persistence purpose. | L3 only; cadence, failure handling, and initial-versus-refresh semantics are unstated. |
| `RC-E011` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, line 28 | Faithful paraphrase: A should provide course and filter browsing. | Recovers a core WebUI use case. | L3 only; exact filters and UX behavior are not enumerated. |
| `RC-E012` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, line 29 | Faithful paraphrase: A should expose section-level registration information. | Recovers section detail as a user-visible outcome. | L3 only; it does not decide which API/UI route supplies that information. |
| `RC-E013` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, line 30 | Faithful paraphrase: A should support subscriptions and open-seat polling. | Recovers the watch-and-detect behavior at the center of the alert workflow. | L3 only; watch limits, persistence, polling cadence, and session semantics are unstated. |
| `RC-E014` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, line 31; contextual baseline at `SRC-REC-03-CODEX-SESSIONS`, line 22 | Faithful paraphrase: A should provide local sound notifications. | Recovers the local alert outcome. | L3 only; trigger repetition, volume, modes, and latency are not specified in this corpus. |
| `RC-E015` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, line 33; `SRC-REC-06-TIMELINE`, line 12 | Faithful paraphrase: A should ship an honest release package and documentation. | Connects product behavior to distributable, accurate user guidance. | L3 only; the exact archive contents and launch contract are absent. |
| `RC-E016` | `HISTORICAL_SUMMARY` | `SRC-REC-08-CURRENT-STATE`, lines 31-38 | Faithful paraphrase: email was conditional on explicit user acceptance of SendGrid setup. | Preserves the historical optional-notification question without making it a requirement. | L3 and explicitly conditional; later accepted current-line scope excludes email from v1. |
| `RC-E017` | `HISTORICAL_SUMMARY` | `SRC-REC-05-PROJECT-FILES`, lines 34-37 | Faithful paraphrase: recovery observed drift in database defaults, packaging, `/api/sections`, subscription docs, and validation commands. | Identifies areas where A's “honest” release claim needed reconciliation. | L3 summary of observations; underlying files must provide L4 evidence before asserting present behavior. |
| `RC-E018` | `HISTORICAL_SUMMARY` | `SRC-REC-00-STATUS`, lines 21-25; `SRC-REC-05-PROJECT-FILES`, lines 22-26 | Faithful paraphrase: the task-015 feature matrix looked useful but was unmerged and failed provenance review. | Prevents a convenient but unaccepted matrix from becoming A's requirements by repetition. | High confidence in the corpus's warning; none of the matrix's substantive claims are imported here. |
| `RC-E019` | `HISTORICAL_SUMMARY` | `SRC-REC-03-CODEX-SESSIONS`, line 21 | Faithful paraphrase: the initial request in the summarized Codex session was read-only understanding, not NGAT execution. | Preserves a user-attributed process boundary and separates reconnaissance from later A planning. | Not direct dialogue and not an A product requirement. |
| `RC-E020` | `HISTORICAL_SUMMARY` | `SRC-REC-07-CONFLICTS`, lines 8-9 | Faithful paraphrase: the corpus says the user rejected switching away from the selected Codex/tmux execution plane. | Preserves the other explicit user-attributed statement without converting it into a product feature. | Summary only; process preference, not Deliverable A behavior. |
| `RC-E021` | `CONFLICT` | `SRC-REC-07-CONFLICTS`, line 11 | The recovery table reports that `/api/sections` was documented as real while scans/matrix material characterized it as empty or stubbed. | A must not advertise a non-working section surface, and the chosen route affects section-level information (`RC-E012`). | Both sides are reported by one L3 summary and are not independently verified here; do not resolve silently. |
| `RC-E022` | `CONFLICT` | `SRC-REC-07-CONFLICTS`, line 12 | The recovery table reports broad email/SMTP documentation while the dispatcher allegedly wired only SendGrid. | Shows why historical email scope could not be accepted from documentation alone. | L3 conflict; current accepted no-email v1 scope supersedes the product choice, but the historical discrepancy remains. |
| `RC-E023` | `UNRESOLVED` | `SRC-REC-08-CURRENT-STATE`, lines 35-38; supported by `RC-E012` and `RC-E021` | The corpus asks whether standalone `GET /api/sections` should ship or section detail should come through `/api/courses?include=sections`. | Leaves the historical section-delivery interface undecided. | No conclusion in the corpus. P1 Review owns any still-current product choice. |
| `RC-E024` | `UNRESOLVED` | `SRC-REC-08-CURRENT-STATE`, lines 32 and 35-38; supported by `RC-E016` and `RC-E022` | The corpus asks whether SendGrid email should ship or be removed/deferred. | Records the historical notification-scope gap. | Unresolved historically, but later accepted current-line v1 scope excludes email; no raw-history read is justified for the current choice. |
| `RC-E025` | `UNRESOLVED` | `SRC-REC-08-CURRENT-STATE`, line 39; closest capability statement `RC-E010` | The corpus asks whether scheduled/automatic data refresh should be recovered. | Refresh behavior affects whether A can keep its local course directory current without manual intervention. | No cadence or decision in the corpus. Later accepted current-line refresh decisions have higher authority. |
| `RC-E026` | `UNRESOLVED` | `SRC-REC-08-CURRENT-STATE`, line 40; supported by `RC-E005` | The corpus asks whether Calendar, Compact view, saved views, and share links belong to A or later UX work. | Preserves the unresolved historical boundary around optional WebUI expansion. | No resolution or direct-user wording in this corpus. |
| `RC-E027` | `UNRESOLVED` | User-attributed summaries at `SRC-REC-03-CODEX-SESSIONS`, lines 19-27, and `SRC-REC-07-CONFLICTS`, line 9; discovery-only previews at `SRC-REC-CODEX-INDEX-JSON` `/candidates/0/user_previews` | The corpus does not provide an admissible exact user-authored message locator for an A requirement. | Prevents L3 paraphrases and index previews from being mislabeled `DIRECT_USER`. | High confidence about this corpus's evidentiary limit. Exact wording must come from already-admitted dialogue or a properly gated task-064 read. |
| `RC-E028` | `IMPLEMENTATION_OBSERVATION` | `SRC-REC-SESSION-INDEX-JSON`, `/counts` and full `/candidates`; corroborated by `SRC-REC-SESSION-INDEX-MD`, lines 6-23 | The combined metadata contains 159 candidate paths (95 Codex, 64 Claude), 91 direct-CWD matches, and zero recorded parse errors; it exactly matches the two source-index path sets. | Establishes the discovery surface available if a named A gap survives the first three evidence passes. | Metadata only. Counts, paths, and match types do not prove any requirement. |
| `RC-E029` | `IMPLEMENTATION_OBSERVATION` | `SRC-REC-MANIFEST`, `/phase` and `/finalized` (text lines 7-8) | The recovery snapshot says it was in `synthesis_after_session_index` and `finalized` was `false`. | Lowers confidence in treating the corpus as a completed or accepted handoff. | Exact metadata observation; it says nothing about user intent. |
| `RC-E030` | `IMPLEMENTATION_OBSERVATION` | `SRC-REC-READ-LOG`, lines 22-24; inventory references at `SRC-REC-02-SOURCE-INVENTORY`, lines 18-19 | The earlier recovery logged reads of active/archived NGAT runtime material. | Identifies a prohibited provenance branch whose resulting runtime assertions must not contaminate A evidence. | This pass read only the declared log/inventory, not the underlying runtime or archive. Substantive archive-derived claims are excluded. |
| `RC-E031` | `INFERENCE` | Supporting evidence `RC-E001`, `RC-E002`, and `RC-E009` through `RC-E015` | Inference: the historical purpose of A was an ordinary local data-to-alert workflow—start locally, acquire SOC data, browse courses/sections, watch openings, alert by sound, and distribute the result honestly. | Provides a concise synthesis for the later candidate-requirements task. | Medium confidence because every supporting product claim here is L3; this is not an accepted requirement set and cannot override L1/L2 evidence. |

## 4. Preserved user-language disposition

The corpus preserves two user-attributed statements: the initial read-only
request (`RC-E019`) and rejection of switching the selected execution plane
(`RC-E020`). Neither source reproduces a message with an exact user-role
locator, so both remain `HISTORICAL_SUMMARY`. The JSON indexes also contain
preview strings under every candidate record. The source-register contract
makes those strings discovery aids only; none is quoted or used as evidence
here.

Accordingly, this artifact intentionally contains no `DIRECT_USER` evidence
record. That absence is a provenance result (`RC-E027`), not an inference that
the user expressed no requirements elsewhere.

## 5. Conditional raw-session candidate register

Every path in this section is **unopened**, remains `CONDITIONAL_RAW`, and is
recorded only as a candidate for task-064. A pointer does not authorize access.
Task-064 must first show that higher-authority mainline and project/Git evidence
failed to close a named material gap, then add one exact hash-pinned file and a
minimal locator to its TaskSpec.

| Candidate ID | Registered metadata locator | Unopened raw pointer | Candidate use and disposition |
|---|---|---|---|
| `RC-CAND-001` | `SRC-REC-CODEX-INDEX-JSON` `/candidates/0` (also combined `/candidates/0`): session `019e1ad0-48f6-7153-a0ae-51dd99a342e8`, 2026-05-12T06:12:19.282Z-12:36:18.707Z, direct CWD | `Z:\.codex\sessions\2026\05\12\rollout-2026-05-12T02-11-58-019e1ad0-48f6-7153-a0ae-51dd99a342e8.jsonl` | Primary candidate only for `RC-GAP-001` through `RC-GAP-003`. Current metadata supplies no message offset, so the six-hour span is not yet a minimally justified raw-read window. |
| `RC-CAND-002` | `SRC-REC-CLAUDE-INDEX-JSON` `/candidates/0` (also combined `/candidates/95`): session `b03479c8-6599-4844-b3ac-0aa69fb18bd4`, 2026-05-12T04:38:08.499Z-04:40:54.663Z, direct CWD | `Z:\.claude\projects\Z--Project-Rutgers-BetterCourseSchedulePlanner\b03479c8-6599-4844-b3ac-0aa69fb18bd4.jsonl` | Candidate for historical deployment/free-hosting discussion only. Later mainline A/B decisions make a raw read unnecessary unless a provenance-specific gap survives. |
| `RC-CAND-003` | `SRC-REC-CODEX-INDEX-JSON` `/candidates/1`: session `019e1c13-a377-7070-b81e-eb1443bf6682` | `Z:\.codex\sessions\2026\05\12\rollout-2026-05-12T08-05-09-019e1c13-a377-7070-b81e-eb1443bf6682.jsonl` | Demoted task-015 attempt-003 supervision candidate. It concerns execution failure, not A product intent; do not open for the current gaps. |
| `RC-CAND-004` | `SRC-REC-CODEX-INDEX-JSON` `/candidates/2`: session `019e1bff-5955-77c1-895c-2c41b604d593` | `Z:\.codex\sessions\2026\05\12\rollout-2026-05-12T07-42-59-019e1bff-5955-77c1-895c-2c41b604d593.jsonl` | Demoted task-015 retry-supervision candidate. It concerns execution/provenance, not A product intent; do not open for the current gaps. |
| `RC-CAND-005` | `SRC-REC-CODEX-INDEX-JSON` `/candidates/3`: session `019e1b8b-eeb4-7e10-8da4-725f266d0a83` | `Z:\.codex\sessions\2026\05\12\rollout-2026-05-12T05-36-55-019e1b8b-eeb4-7e10-8da4-725f266d0a83.jsonl` | Demoted task-015 collision-supervision candidate. It concerns execution/provenance, not A product intent; do not open for the current gaps. |
| `RC-CAND-006` | `SRC-REC-CLAUDE-INDEX-JSON` `/candidates/1`: session `fdd62656-ab1e-4611-8829-d55cde1b1500` | `Z:\.claude\projects\Z--Project-Rutgers-BetterCourseSchedulePlanner\fdd62656-ab1e-4611-8829-d55cde1b1500.jsonl` | Demoted memory-system setup candidate. It may explain document creation but is not a product-requirement gap. |
| `RC-CAND-007` | `SRC-REC-CLAUDE-INDEX-JSON` `/candidates/2`, `/candidates/3`, `/candidates/7`, `/candidates/8`; session IDs `e4e2590b-b852-4611-b627-03346bf191d5`, `02854001-64eb-4c49-a056-c791e839fedf`, `5c0fdf7a-9512-4310-ae8b-1689f94c3675`, `7f51daf0-4936-440d-9773-6c4f7a546c82` | Exact paths are the corresponding `path` fields at those four JSON Pointers. | Demoted Stage A review candidates. They may prove review activity, not A user intent; do not open for the current gaps. |

The other 149 combined-index entries are accounted for by the fully parsed
metadata sources but are not selected: the narratives identify them as older
operational sessions, worktree workers, duplicate scans, path-only near-misses,
or unrelated-project mentions. Their previews were not used to manufacture
additional candidates.

## 6. Named recovery-corpus gaps for task-064

These are gaps in this L3 corpus, not permission to crawl history. `Open` means
task-064 may consider one exact candidate only if the mainline and project/Git
artifacts still lack the answer. `Closed for raw` means newer accepted context
already supplies the operative decision, so historical curiosity is
insufficient.

| Gap ID | Existing evidence that fails to answer it | Unresolved question | Candidate and privacy/minimization assessment | Task-064 disposition |
|---|---|---|---|---|
| `RC-GAP-001` | `RC-E002` through `RC-E008`, `RC-E027` | What exact user-authored wording and acceptance state established “complete local release,” not-MVP, local-not-cloud, deliberate-UI, honest-surface, and Windows-validation expectations? | `RC-CAND-001` is likely relevant, but its six-hour span and unlocated preview strings create high unrelated-content risk. First use any already-admitted exported dialogue and exact message locators. | **Open only if still material after the other evidence passes.** Raw access is not yet sufficiently minimized. |
| `RC-GAP-002` | `RC-E012`, `RC-E021`, `RC-E023` | Was standalone `/api/sections` historically required, or was section detail through the course surface sufficient? | `RC-CAND-001` is the only narrative-selected product-planning candidate. No message/time locator is available from metadata. | **Open only if current P1 Review needs historical intent and higher-authority evidence is silent.** A fresh product decision is preferable to a broad raw read. |
| `RC-GAP-003` | `RC-E005`, `RC-E026` | Were Calendar, Compact view, saved views, or share links ever explicitly required for A? | `RC-CAND-001` is a weak broad candidate; no exact index metadata ties it to those features. | **Open only if an already-admitted source yields a narrow locator.** Otherwise leave unresolved rather than crawl. |
| `RC-GAP-004` | `RC-E007`, `RC-E009`, `RC-E015` | The corpus does not state A's unpacking, launcher, privilege, or separately-installed-runtime contract. | `RC-CAND-001` is too broad. The governing current-line context already supplies the operative Windows package/BAT/no-separate-runtime decision. | **Closed for raw.** Preserve the gap only to avoid attributing the newer decision to this corpus. |
| `RC-GAP-005` | `RC-E013`, `RC-E014` | The corpus does not establish watch limits, fresh-Open trigger semantics, sound volume/mode controls, or alert-latency status. | No recovery index supplies a precise candidate locator. Current accepted context supplies the operative behavior and labels latency aspirational. | **Closed for raw.** Do not search history merely to backfill newer accepted decisions. |
| `RC-GAP-006` | `RC-E010`, `RC-E025` | What refresh cadence and A configurability were historically intended? | No minimally located raw candidate. Current accepted context supplies the ten-minute/default configurability direction. | **Closed for raw.** Historical ambiguity does not override the accepted decision. |
| `RC-GAP-007` | `RC-E016`, `RC-E022`, `RC-E024` | Was optional SendGrid email historically desired? | `RC-CAND-001` might contain the discussion, but current accepted v1 scope excludes email, SMTP, SendGrid, and mail configuration. | **Closed for raw.** The current product choice is resolved; preserve only the historical conflict. |
| `RC-GAP-008` | `RC-E004`; candidate metadata `RC-CAND-002` | What exact historical dialogue preceded the local-versus-cloud split? | `RC-CAND-002` has a narrow 2m46s span, but later accepted A/B decisions already govern. | **Closed for raw unless an audit-specific provenance defect is identified.** |

## 7. Requirements-facing handoff

The recovery corpus supports one cautious L3 synthesis: historical A aimed at a
complete, honestly described Windows-local course-schedule and open-seat alert
release, spanning local start, SOC data acquisition, SQLite persistence,
course/section browsing, subscriptions, polling, sound alerts, documentation,
and packaging (`RC-E031`). It does **not** establish an accepted feature matrix,
exact user wording, launcher mechanics, endpoint shape, optional UX expansion,
or detailed watch/audio/refresh semantics.

Downstream synthesis must preserve the individual `RC-E*` IDs and their L3
limits. It must not cite the recovery corpus as `DIRECT_USER`, reuse the failed
task-015 matrix as accepted truth, import archive-derived runtime assertions,
or claim that a session pointer was read. Any surviving material gap goes to
task-064 under Section 6; all other ambiguity remains for P1 Review.

## 8. Boundary attestation

- Declared recovery-corpus files read and coverage-decided: **20/20**.
- Raw `.codex` / `.claude` session JSONL files opened: **0**.
- Archived NGAT runtime files opened: **0**.
- Discarded requirements artifacts opened: **0**.
- External files modified, copied into the repository, staged, or packaged:
  **0**.
- Repository artifacts written by this task: only
  `docs/p1-a-recovery/04-recovery-corpus-evidence.md`.
