# RC Iteration Round 4 Implementation Record

## Control

| Field | Value |
|---|---|
| Status | `IMPLEMENTATION_COMPLETE` |
| Implementation authorization | User: “PLEASE IMPLEMENT THIS PLAN” |
| Authorized scope | Local implementation, verification and Windows HumanTest candidate only |
| Recovered baseline commit | `21cb28a0219f8b6b588479ce0fd8e4099464869d` |
| Product commits authorized | One local RC4 commit, after the recovered baseline commit |
| Remotes | Zero; must remain zero |
| Remote actions | `NOT_AUTHORIZED` |
| Reference date | 2026-07-17 |

This record implements the decisions in `03-rc-iteration-round-04-final-discussion-and-design.md`, except only where the two later direct user corrections below explicitly supersede it. It does not replace, paraphrase, or modify the HumanTest original text or its five stable images.

## Post-plan user correction — Figure 5 scroll behavior

> 图 5 的问题是原生滚动条与整体 UI 风格不协调，并非要求删除所有内部滚动。请隐藏这些突兀的滚动条，或替换为统一风格，同时保留完整的滚轮、触控和键盘操作能力，避免内容被裁切。

The user clarified during implementation that Figure 5 objects to native scrollbars that visually conflict with the product, not to internal scrolling itself. This later direct decision supersedes the earlier over-broad “no internal vertical scrolling” implementation wording in the plan and in §5.6/§9.2/§10.7 of `03` only to the extent those passages prohibit every internal scroll container.

The implemented acceptance contract is therefore:

- document/window remains the primary page scroll context and the removed RC3 filter rail is not restored;
- a long option collection may use an internal scroll container when that materially preserves a usable control size;
- any such scrollbar is either visually hidden or rendered in the same flat Swiss Industrial Print language—never left as the conspicuous native scrollbar seen in Figure 5;
- wheel, touch, Tab and keyboard list navigation remain fully operational, first and last content remain reachable, and no content is clipped;
- visual and interaction evidence must test reachability and overflow behavior rather than treating the mere presence of `overflow: auto` as a failure.

## Post-plan user correction — disabled Pull presentation

> 未发布或发布状态未知，“拉取”按钮保持显示，但为不可用状态。按钮附近不显示“尚未发布”“发布状态未知”等原因说明。这项最新裁决覆盖之前“在按钮外说明原因”的设计。

The user subsequently ruled that a Local-term `拉取` / `Pull` action remains visible when publication is `UNPUBLISHED` or `UNKNOWN`, but is disabled without an adjacent explanation. This later direct decision supersedes the earlier requirement in the implementation plan and `03` to show an out-of-button reason.

The implemented acceptance contract is therefore:

- `UNPUBLISHED` and `UNKNOWN` each retain the contextual `拉取` / `Pull` button in its matrix position;
- the button is unavailable, while its ordinary accessible name remains `拉取` / `Pull`;
- no nearby visible text, tooltip, or `title` says “尚未发布”, “发布状态未知”, or the English equivalent;
- the reason remains available to assistive technology through a hidden `aria-describedby` target, preserving the plan's accessibility requirement without restoring the superseded visible explanation;
- `PUBLISHED` remains eligible for an enabled Pull when the term has missing pullable targets; this ruling does not alter publication derivation, target eligibility, or Pull execution semantics.

## Stage 0 — recovered baseline and protected evidence

- The recovered project tree was committed locally as `21cb28a` without restoring historical Git, `.ngagent`, `AGENTS.md`, or NGAT state.
- `04a-rc-iteration-round-04-source-baseline-manifest.json` records 1,088 staged source files; the two generated manifests are self-excluded.
- `04b-rc-iteration-round-04-protected-assets-manifest.json` records 25 local chatlog, HumanTest, and preserved-ZIP files excluded from Git.
- The repository had and retains zero remotes.
- No RBCSP process was running before database capture.
- The HumanTest SQLite group was copied to ignored cache storage and verified byte-for-byte before use:

| File | Bytes | SHA-256 |
|---|---:|---|
| `rbcsp.sqlite` | 26,599,424 | `6394f1ee4b7e9abbbd5c5a5795eb156b887242063477a703755cb9ec7b3cf8b2` |
| `rbcsp.sqlite-wal` | 849,927,192 | `eac6831e5425354f0c38e794d527b82a095e8627664370a95f1dffd2757c1236` |
| `rbcsp.sqlite-shm` | 1,671,168 | `bab56016309382085a580db2f36c1c49e34b4ebd6fed181255883b51ece68d90` |

All later database tests operate on cache copies. Opening the first copy consolidated the WAL into the copied database and removed the copied sidecars; the protected originals were not touched.

## Stage 1 — untouched RC3/V2 baseline

### Frontend source gate

The recovered baseline passed the locked Node 24.18.0 / npm 11.16.0 frontend gate:

- import and target-build guard: pass;
- Vitest: 22 files, 161 tests passed;
- TypeScript build: pass;
- Local and Public production builds: pass.

The legacy `capture-composition-matrix.mjs` timed out waiting for the old RC3 Local READY selector. This is recorded as a baseline evidence-harness defect; Round 4 must update the fixture and state assertions to V3 and the new Local/Public layouts rather than treating the timeout as a visual pass.

The recovered commit was also replayed from a detached clean worktree with the locked offline frontend toolchain. `evidence/round-04/baseline/validation.md` binds the inspected RC3 DOM and test surfaces to their Git blobs, records the 83/83 guard and 161/161 Vitest pass, and distinguishes the passing jsdom axe/keyboard/scroll tests from the Chrome matrix that the stale READY wait prevented from completing. Together with the five immutable HumanTest images, it is the explicit Before audit for DOM structure, keyboard paths, accessibility coverage and scroll-container behavior.

### Rust source gate

The recovered baseline compiled under Rust/Cargo 1.97.0. The full test run exposed one recovery-environment failure: `bcsp-operational-storage` compared embedded migration bytes with a Windows checkout that had CRLF line endings. Round 4 adds a Git `*.sql text eol=lf` contract and must prove the final clean checkout passes the byte-for-byte migration test.

### HumanTest database performance baseline

The enhanced baseline runner used the untouched recovered commit and a byte-identical cache copy of the protected database group. The tracked record is `evidence/round-04/performance/v2-baseline-enhanced.json` (62,158 bytes; SHA-256 `9a426597d8b56643b6620531007f8bf6518cd860c1ea01dc06057dc59aeabd69`). It contains no absolute path, account/host identity, URL, credential, or raw Course/Section payload. Measurements are end-to-end in-process product-route times; warm cohorts are reported separately from the first cold request.

| Fixture | Cold | Warm p50 | Warm p95 | Warm max | Repetitions |
|---|---:|---:|---:|---:|---:|
| Service status | 3.486 ms | 3.103 ms | 3.300 ms | 3.441 ms | 20 |
| Three-Campus filter options | 2,584.352 ms | 2,435.501 ms | 2,489.254 ms | 2,489.254 ms | 10 |
| Single-Campus neutral Search | 2,052.935 ms | 2,021.600 ms | 2,081.023 ms | 2,081.023 ms | 10 |
| Three-Campus complex Search | 2,503.276 ms | 2,495.986 ms | 2,569.675 ms | 2,569.675 ms | 10 |

Status/options/single/complex held the global storage mutex for 99.02%/99.997%/99.962%/99.994% of warm route time respectively, with maximum lock wait no greater than 0.0004 ms. The bottleneck was therefore work performed while holding the mutex, not contention while acquiring it. The single-Campus witness remained `total=4,432` with 20 Course rows and 25 materialized Sections on page one; the complex witness remained `total=1`, with one Course and one Section on page one and an empty page two. Every warm response hash was stable.

The runner recorded process lifecycle memory and storage effects rather than inferring them: peak Working Set was 543,379,456 bytes and peak private/pagefile usage was 565,276,672 bytes. The input DB/WAL/SHM sizes and hashes matched the protected manifest. Closing the copied database consolidated the sidecars into a 274,649,088-byte database; the subsequent passive checkpoint reported `busy/log/checkpointed = 0/0/0`. Stages that the old V2 code did not expose—SQLite load, projection, corpus, predicate, sort, pagination and materialization—are explicitly marked unavailable in the JSON rather than estimated.

The observed results reproduce the HumanTest concern: status is already well inside budget, while options and Search repeatedly reload/project enough data to miss the Round 4 budgets by a wide margin.

## Stage 2 — term, Campus, state and QueryScope contract

- `America/New_York` and the Rutgers teaching calendar determine current/next rather than the latest published SOC selector. At the fixed `2026-07-17` clock, Local is exactly `02026 → 12026 → 72026 → 92026 → 02027`, current is Summer `72026`, and next is Fall `92026`; Winter uses its ending year and every label is formatted solely from the active locale.
- NB/NK/CM is the product Campus allowlist in status, discovery projection, refresh demand, query, watch and UI. `ONLINE_*` is no longer a Campus; delivery format remains independently filterable.
- Publication is derived only from a successful discovery no older than six hours. Complete target Snapshot/work state remains separate, so READY/LKG usability is not erased by a later UNKNOWN publication state. Term readiness is the explicit `0/3` through `3/3` aggregate and is never conveyed by color alone.
- A fresh candidate is current term with no Campus selected. Candidate edits do not mutate applied scope. Apply validates the complete candidate, rejects all stale dynamic values without silently dropping them, preserves the old applied/result state on failure, and clears old submitted results without auto-searching on success.
- Local manual Pull is term-level, covers only missing NB/NK/CM targets, deduplicates active work and never auto-selects, applies or searches. Current/next and every Public term have zero Pull surface. Public consumes the same QueryScope selection/apply/search implementation with two real terms and its distinct frozen layout.
- The contextual action evaluator distinguishes Apply, Applied, Pull, active work and retryable terminal failure. The later user ruling in this record controls only the visible disabled-Pull explanation for UNPUBLISHED/UNKNOWN; backend publication and eligibility semantics are unchanged.

### Stage 2 verification state

`VERIFIED`. Fixed-clock term/status, Pull, Campus allowlist and Public-zero-surface tests passed both their focused suites and the final locked workspace replay. The final architecture gates report 15 Rust workspace members, all 18/18 Public SOURCE denies, and Public zero-surface `SOURCE/API/STORAGE/PACKAGE = 18/18` with a 12-crate Cargo closure and zero features.

## Stage 3 — Query V3, dynamic bands and Local migration

- Product query endpoints accept `contractVersion: 3` in the retained `contractVersion + values` envelope. `courseNumbers` became normalized numeric `courseNumberBands`; each non-negative 100-multiple means exactly `N–N+99`, including a real `000-level` band when Catalog data contains one.
- Filter schema/options, Rust and TypeScript contracts, fixtures and golden files use V3. Course-number bands and every other dynamic option are generated from the complete applied term plus selected-Campus Catalog vector, not the current result page or an unselected Campus.
- Empty arrays and `ANY` are neutral. Prerequisite is `ANY/HAS/NONE_REPORTED`; modality and synchronicity are same-field OR. Their independent `includeIncomplete` switches admit only that field's `UNCERTAIN` descendants when the corresponding filter is active and never admit a definite `NO_MATCH`.
- Raw technical values and uncertainty reasons remain in detail/evidence but are not exposed as ordinary filter choices. Same-section witnessing, ordered pagination and descendant materialization remain three-valued and deterministic.
- The V2 decoder exists only in the Local migration adapter. Lossless neutral records migrate; unsafe current filters and Saved Views retain their original JSON and become `REVIEW_REQUIRED` or `INCOMPATIBLE` with field-level reasons. Unsupported-Campus watches stop scheduling without deleting history.
- Exact selected-value validation is Catalog-vector-bound and returns every invalid dynamic value. Apply and Saved View review no longer use the former “silently remove missing values” behavior.

### Stage 3 verification state

`VERIFIED`. Contract/schema golden tests, normalization and three-valued query tests, migration matrices, frontend V3 tests and the prepared/reference JSON parity suite all passed. The final Rust workspace replay completed 603 tests with zero failures; six manually invoked evidence runners remained ignored in the ordinary workspace cohort and were executed separately where required below.

## Frozen performance completion gates

| Surface | p95 | max |
|---|---:|---:|
| Service status | 250 ms | 1 s |
| Filter options | 500 ms | 1.5 s |
| Single-Campus Search | 1 s | 2 s |
| Three-Campus complex Search | 2 s | 5 s |

| Evidence gate | Current state |
|---|---|
| Untouched RC3/V2 baseline | `CAPTURED` |
| Authoritative unoptimized V3 oracle and profiler | `CAPTURED` |
| Optimized four-budget result | `PASS` |
| V3 response/witness/reason equivalence | `PASS` |
| Memory, WAL/checkpoint and prepared-registry metrics | `CAPTURED / PASS` |
| Deterministic publish/Search and failure-LKG barriers | `PASS` |

The functional V3 reference must be captured before performance changes. Optimization may use an independent version-bounded SQLite read path, an immutable prepared Catalog model with a versioned Open overlay, or both, but only in response to measured stage costs. Correctness remains bound to the unoptimized V3 reference.

## Stage 3/4 boundary — superseded provisional V3 capture

An initial pure-V3 timing pass exposed the expected multi-second scans, but integration review then found two Query V3 semantic defects: active 09/12/13 `UNCERTAIN` outcomes had been collapsed to `NO_MATCH`, and dynamic selected-value validation was not yet exact. That pass is retained only in ignored scratch evidence and is not used as an equivalence oracle or completion claim. Both semantic defects were repaired before the fresh capture below; no optimization result is compared with the discarded response hashes.

## Stage 4 — authoritative unoptimized Query V3 oracle and profiler decision

The authoritative unoptimized reference was captured from the functional V3 path before any prepared-serving optimization. It preserves raw `UNCERTAIN` outcomes and reason codes, applies each incomplete-data switch only at admission/materialization, and performs exact Catalog-bound selected-value validation. Reference and profiler used separate, non-overwriting DB/WAL/SHM copies whose input hashes exactly matched Stage 0.

| Tracked artifact | SHA-256 |
|---|---|
| `evidence/round-04/performance/v3-unoptimized-reference-final.json` | `79e76b499675db5c09d62341ceb6719ec347d700a26bb6ca83a36d80b112e82b` |
| `evidence/round-04/performance/v3-unoptimized-profile-final.json` | `b45f866054469076fe1e2f2b42b57817c2f9c95a85f02c7705e64ca9f36f2841` |

Both records bind release-mode Rust/Cargo 1.97.0, locked offline dependencies, Windows x86-64, 32 reported hardware threads, processor identifier `Intel64 Family 6 Model 183 Stepping 1, GenuineIntel`, executable SHA-256 `6d32f2a28601d4e60b80dcb0210ed7e96993d3a3a972ccd1eb801801feada39a`, fixed clock `2026-07-17T12:00:00Z`, term `92026`, and the exact Stage 0 database bundle hashes. Neither tracked JSON contains an absolute local path.

### Authoritative unoptimized V3 budget result

| Fixture | Cold | Warm p50 | Warm p95 | Warm max | Budget result |
|---|---:|---:|---:|---:|---|
| Service status | 2.145 ms | 1.561 ms | 1.845 ms | 1.953 ms | pass |
| Complete three-Campus course-number-band options | 2,777.244 ms | 3,009.542 ms | 4,014.788 ms | 4,014.788 ms | fail p95 and max |
| Single-Campus neutral Search | 2,035.750 ms | 2,150.555 ms | 3,185.582 ms | 3,185.582 ms | fail p95 and max |
| Three-Campus complex Search | 2,609.958 ms | 2,635.555 ms | 3,269.470 ms | 3,269.470 ms | fail p95; max passes |

The ordered serving vector was identical before and after every cohort: `92026/CM = Catalog 1, Open 1`, `92026/NB = Catalog 1, Open 82`, `92026/NK = Catalog 1, Open 1`. The single-Campus result retained `total=4,432`, page size 20, stable Course order and 25 materialized page-one Sections. The complex fixture retained `total=1`, exact same-section witness `92026/NB/10052`, stable empty page two, frozen V3 envelope and response hashes. Exact validation, three-valued reasons and descendant materialization are also captured for optimized equivalence comparison.

### Profiler breakdown and one-scheme selection

The enhanced profiler recorded three release-mode warm runs per surface. Inclusive rows are not summed with their children.

| Surface/stage | Mean | Interpretation |
|---|---:|---|
| Options total / global storage mutex held | 3,850.831 / 3,850.748 ms | Entire options route ran inside the mutex |
| Options target validation / query service | 1,421.023 / 2,429.479 ms | Discovery validation and Catalog preparation were repeated |
| Options Catalog projection / Catalog SQLite load | 1,392.627 / 816.800 ms | Dominant repeated work; dictionary collection itself was 1.779 ms |
| Single Search total / mutex held | 2,995.981 / 2,994.917 ms | Violates the global-mutex architecture gate |
| Single Search target validation / query runtime | 1,234.093 / 1,760.618 ms | Two full preparation phases dominated the request |
| Single predicate / corpus / Open overlay | 3.258 / 53.922 / 11.094 ms | Filter semantics were not the main bottleneck |
| Complex Search total / mutex held | 3,936.888 / 3,936.423 ms | p95 exceeded budget and refresh would be blocked |
| Complex target validation / query runtime | 1,343.794 / 2,592.372 ms | Repeated discovery and Catalog projection dominated |
| Complex predicate / corpus / Open overlay | 48.375 / 79.582 / 15.784 ms | Multi-filter evaluation remained a minority of time |
| Exact validation total / mutex held | 3,607.618 / 3,607.517 ms | Exact Apply validation had the same architectural defect |

Uncontended lock wait rounded to zero. The evidence therefore selects one optimization scheme only: a version-bound immutable `PreparedServingSnapshot`. It must contain the allowed/published target metadata, normalized Catalog corpus, complete Catalog-only option dictionaries and course-number bands, plus the exact authoritative FTS corpus. A lightweight Open overlay binds each Search/detail request to the same Catalog vector and its Open observation sequence; a 30-second Open refresh must not rebuild the Catalog/FTS foundation.

Requests may only clone one already published immutable generation; they may not perform a per-request whole-Catalog load, projection or dictionary scan under the global storage mutex. A dedicated readonly SQLite connection and bounded temporary SQLite FTS file are permitted only as off-request builders for this one selected scheme, not as a second request path. The registry has an explicit byte bound, capacity/eviction rule, hit/miss, build-time and byte metrics; a new publish does not grant new references to the superseded generation, already-bound requests finish, and refresh failures retain valid LKG. The evidence below closes the optimized budget, equivalence, memory/WAL and deterministic barrier gates.

## Stage 4 — optimized prepared-serving result

The final implementation builds a collision-safe immutable Catalog foundation once, publishes one capacity-bounded `PreparedServingSnapshot`, and replaces only the affected Open target overlay for Open updates. A request captures one `Arc` generation and an active-target view; it cannot observe a mixed Catalog/Open vector. Hash buckets always perform full-key equality, `campuses=[]` remains an explicit product-target subset, and no unsafe or self-referential structure is used.

The frozen release runner was rebuilt after the source and Detail probes were final. Its executable SHA-256 is `563413edc4d5ab59fed74c6e1074fd67d89afe6c9ff2cb63d05e7ba436863db5`; the runner source SHA-256 is `e57ef9304600e4b46930b511cc9e1a92c82a9268fbffb164ea05e34caef2f11e`. Functional, profiler and resource runs each used a separate fresh DB/WAL/SHM copy whose three hashes matched the protected manifest before opening.

| Tracked artifact | SHA-256 |
|---|---|
| `evidence/round-04/performance/v3-optimized-reference-final.json` | `841197419169594b70947246876f62c6e6fd43d12589b50f4cca2acd82bf47c1` |
| `evidence/round-04/performance/v3-optimized-profile-final.json` | `a161185285ec31fcfbdaf3d3799661f36b95079b29bc4c4fe9ab016fce56982c` |
| `evidence/round-04/performance/v3-optimized-concurrency-final.json` | `a7b4caf6d9b99833bb73ff2c41f69470ffc59d30e7caba441e5ba8ab5a036f63` |

### Final optimized budget result

| Fixture | Cold | Warm p50 | Warm p95 | Warm max | Budget result |
|---|---:|---:|---:|---:|---|
| Service status | 1.683 ms | 1.232 ms | 1.385 ms | 1.386 ms | pass |
| Complete three-Campus course-number-band options | 0.069 ms | 0.007 ms | 0.009 ms | 0.009 ms | pass |
| Single-Campus neutral Search | 3.455 ms | 2.726 ms | 3.130 ms | 3.130 ms | pass |
| Three-Campus complex Search | 29.547 ms | 28.736 ms | 30.885 ms | 30.885 ms | pass |

Every optimized status/options/Search/validation functional witness is exact against the authoritative unoptimized V3 oracle: response hashes, result count, ordering, pagination, same-section witness, uncertainty reasons and descendant materialization all match. Course and Section Detail each produced three byte-stable responses and are bound to the in-process prepared/reference JSON parity test rather than falsely claiming an unoptimized artifact hash. Every options/Search/validation/Detail profile run hit the prepared snapshot and contained none of `sqlite_catalog_load`, `catalog_projection`, `discovery_full_projection`, `corpus_build` or `open_overlay_build` on the request path.

The published snapshot estimate is 469,058,174 bytes within the 536,870,912-byte one-generation registry budget; the separately bounded SQLite cache is 16,777,216 bytes. External 20 ms process sampling recorded 202 samples over 6,629.56 ms: peak Working Set 614,289,408 bytes and peak paged/private usage 645,189,632 bytes. These whole-process peaks include the executable, `OperationalStorage` and transient off-request builder state and are not substituted for the registry estimate.

After the cache-only run, the copied WAL/SHM sidecars were closed and consolidated into a 274,649,088-byte database with SHA-256 `a8fb09a2bd41e6df5f263e3c1ecc8222b253512eddd7a610e984762353dac574`. A passive checkpoint reported `busy/log/checkpointed = 0/0/0`; size and hash remained unchanged. The protected originals were then replayed successfully.

The deterministic concurrency gate uses real temporary file-backed `OperationalStorage`, a spawned rebuild worker and a publication barrier without timing sleeps. It proves precommit forced-unavailable behavior, commit-time admission gating, Catalog foundation `Arc` reuse, old pinned-request byte equivalence after replacement, new Open state visibility, no mixed vector, no deadlock and orderly shutdown. The focused failed-refresh test separately proves Catalog/Open LKG remains serviceable and is projected stale rather than erased. The three-axis parity gate proves each `includeIncomplete` switch independently and jointly, preserves reason arrays, and never admits definite `NO_MATCH` descendants.

## UI skills and mandatory sequence

Stage 1 uses `industrial-brutalist-ui` and `design-taste-frontend`, with `redesign-existing-projects` limited to a targeted Scan → Diagnose → Fix audit of the existing React implementation. The user-frozen Swiss Industrial Print system overrides generic skill suggestions for rounded cards, new dependencies, perpetual motion, gradients, and decorative animation.

After the first integrated implementation and browser validation, Stage 2 uses `emil-design-eng` and must add a real `Before | After | Why` table based on observed code and screenshots, implement every accepted review correction, and repeat the same responsive, keyboard, axe, and reduced-motion checks.

## Stage 5 — mandatory UI Stage 1 integrated reference

The first-stage implementation used `industrial-brutalist-ui` and `design-taste-frontend`, with `redesign-existing-projects` limited to a Scan → Diagnose → Fix audit. It produced a real Local/Public browser reference under `evidence/round-04/stage-1` before the second-stage review began. The reference contains 59 files: 50 composition images and 9 Course/Section-flow images.

That integrated reference established the shared QueryScope behavior, Local `2 × 5`, Public `2 × 3 + full-width Search`, the flat 03–18 filter sequence, the removed RC3 filter rail, and the deliberately retained but product-styled long-list scroll regions. Its browser harness passed 22/22 Local/Public × locale × viewport composition scenarios and 7/7 Course/Section flow snapshots.

## Stage 6 — mandatory UI Stage 2 `emil-design-eng` review

The second stage began only after the first-stage integration and browser evidence existed. The review used all five stable HumanTest images, the real RC3 DOM/code, and `evidence/round-04/stage-1` as Before evidence. Every accepted finding below was implemented; this is not a prospective example table.

| Before | After | Why |
|---|---|---|
| HumanTest Figure 1 and the Stage 1 unpublished-term screenshot still exposed `Not published by Rutgers` / `Rutgers 尚未发布` in the selected term card. | `UNPUBLISHED` and `UNKNOWN` still produce the visible disabled `Pull` / `拉取` action, but the complete QueryScope contains no visible publication-explanation copy, tooltip, or `title`. The term card retains relative position, deterministic term ID and non-color readiness such as `Not loaded 0/3`; a hidden `aria-describedby` target preserves the disabled reason for assistive technology. | Implements the latest user ruling at the visual matrix level while retaining the independently frozen accessibility requirement and backend publication semantics. |
| The Stage 1 Section-detail facts used three columns for four Section fields. `Permission to add` occupied only the first cell of its row, leaving two large black grid cavities. | The final Section fact is explicitly marked as a full-row field and spans `grid-column: 1 / -1`; browser geometry asserts that its left and right edges match the complete fact-list width. | Removes a visually dominant accidental void while keeping the rigid Swiss Industrial grid and the same semantic `<dl>`. |
| Ordinary English filter labels still read `Course layer`, `Prerequisite presence`, `Modality`, `Synchronicity`, `On campus or in person`, `Sync`, and `Async`. | They now read `Course level`, `Prerequisites`, `Class format`, `Meeting timing`, `In person`, `Synchronous`, and `Asynchronous`; Section presentation uses the same user-facing option terms. | Replaces implementation vocabulary and abbreviations with labels that a student can scan without learning the raw data taxonomy. |
| HumanTest Figure 5 showed visually foreign native scrollbars. The initial wording risked eliminating all internal scrolling and clipping long collections. | Only genuinely long subject and dictionary collections retain internal scrolling, with flat product-styled scrollbars; wheel, touch pan, sequential Tab reachability, Home/End navigation, and first/last-item visibility are browser-verified. The document remains the primary page scroll context and the removed filter rail stays absent. | Implements the user's clarification: visual integration without sacrificing reachability or input modality support. |
| HumanTest Figures 2 and 3 showed oversized Query-range and Search hero panels that consumed space and separated related controls. | The second-stage evidence retains the Stage 1 strict Local/Public matrices, a single form-associated Search submit, and no duplicate hero panel. | Confirms that polish did not regress the frozen layout or reintroduce redundant hierarchy. |
| HumanTest Figure 4 showed two dark accordion headers and hid the 03–18 filter sequence behind disclosure state. | The final DOM has no filter `<details>` and keeps all 16 numbered rows visible; only result Sections remain collapsed by default. | Preserves the user's requested directness while keeping the one disclosure that meaningfully controls potentially large result descendants. |
| Interaction CSS had component-level reduced-motion rules, but no single regression check covered `transition: all`, abrupt standalone `ease-in`, or the rendered reduced-motion state. | A static product-CSS audit rejects `transition: all` and standalone `ease-in`; the browser matrix also verifies the emulated reduced-motion media query and records the observed ≤0.0011 ms computed interactive transitions. No animation was added to QueryScope or high-frequency filters, and no new fixed animation-duration policy was introduced. | Converts the aesthetic review into an enforceable interaction contract while respecting the Round 4 prohibition on inventing an unfrozen duration threshold. |
| Rapid consecutive state screenshots could be captured before Chinese fallback fonts and the next paint fully settled, producing a Chromium black-block artifact despite passing DOM assertions. | Figure 1 evidence now waits for `document.fonts.ready` plus two animation frames after each term change before capture. | Keeps the visual record trustworthy and prevents a renderer timing artifact from being mistaken for a product defect. |

### Stage 6 validation

- Locked Node 24.18.0 / npm 11.16.0 `npm run verify`: pass.
- Import/target guard: pass; guard suite 83/83.
- Vitest: 22 files, 170/170 tests passed.
- TypeScript project build: pass.
- Local production build and capability verification: pass.
- Public production build and zero-local-surface verification: pass, including 76 DOM/route/i18n/bundle assertions.
- Browser composition matrix: 22/22, covering Local/Public, `en-US`/`zh-CN`, 390/768/1440/1920/2560, 320 narrow stress, axe, keyboard traversal, fixed focus, responsive order, reduced motion, and no horizontal overflow.
- Course/Section browser flow: 7/7, including the five frozen viewports plus 320 stress, V3 combined filters, collapsed Sections, direct Section navigation, full-row fact geometry, styled-scroll metrics, wheel, touch, Tab, Home/End, and no clipped final option.
- Unpublished and publication-unknown Pull PNGs are byte-identical within each locale, while browser assertions verify their distinct hidden descriptions; this proves the latest ruling at both rendered-pixel and accessibility-tree inputs.
- Stage 2 evidence is preserved separately under `evidence/round-04/stage-2` (61 browser images, including separate unpublished and publication-unknown Pull captures, plus its validation record) and does not overwrite the 59-image Stage 1 reference.
- The first full-gate attempt exposed a test-only render timing race after exact dynamic validation. The test now waits for the alert to enter the accessible tree; the focused test passed 12/12 and the subsequent complete gate passed 170/170.

## Final locked source and evidence gates

- Rust/Cargo 1.97.0, locked and offline: `cargo fmt --all -- --check` passed; the workspace test replay passed 603 tests with zero failures; workspace all-target Clippy passed with `-D warnings`.
- `cargo-deny --all-features --locked --offline check advisories bans licenses sources`: advisories, bans, licenses and sources all passed.
- Rust architecture live gate and self-test passed. Public Rust zero-surface live gate and self-test passed with 72/72 `SOURCE/API/STORAGE/PACKAGE` assertions.
- Node 24.18.0 / npm 11.16.0 clean offline `npm ci` and the complete frontend `npm run verify` passed: guard 83/83, Vitest 170/170, typecheck, Local build and Public build/zero-surface checks.
- The final Stage 2 browser evidence remains 22/22 composition scenarios plus 7/7 Course/Section flows and 61 PNGs; the final source did not change after this browser pass.
- Protected manifest replay passed 25/25. Governance records `00`–`04b` and the five immutable HumanTest Before images have zero diff from the recovered baseline.
- The repository has zero remotes and no deleted paths. The root `npm test` placeholder was intentionally not invoked because the project contract defines it as a deliberate failure rather than a gate.

## Final stop condition

Tracked implementation state: `IMPLEMENTATION_COMPLETE`.

The product commit SHA and Windows ZIP SHA-256 cannot be embedded in the product commit that defines them without creating a self-reference. They are therefore recorded only after the exactly-two-commit history and clean detached-worktree package verification finish, in an ignored local attestation and the final handoff. This tracked record closes the source, UI, performance, concurrency, protected-evidence and local-gate work; it does not predeclare the package result.

After the local commit and package gates pass, the external attestation and handoff use exactly:

```text
LOCAL_CANDIDATE_READY_FOR_HUMAN_TEST
REMOTE_ACTIONS_NOT_AUTHORIZED
```
