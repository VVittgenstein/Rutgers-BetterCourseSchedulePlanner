# RC Iteration Round 5 Implementation Record

## Control

| Field | Value |
|---|---|
| Governing design | `08-rc-iteration-round-05-final-discussion-and-design.md` |
| Product relationship | RC3 recent baseline + valid RC4 contracts − incorrect RC4 workspace execution |
| Historical parent | Reconciled RC4 `e7d6098cf44482a5114eef50976a93697ca2282a` |
| Product commits authorized | One RC5 commit |
| Raw HumanTest evidence | Local protected evidence; excluded from public Git |
| Runtime parity | Local Windows and Public Linux must use the same final RC5 SHA |

This record contains only sanitized implementation evidence. It does not reproduce the Round 5 raw conversation, the three HumanTest images, machine-local paths, database material, or session artifacts.

## Pre RC 5 reconciliation

The isolated exact-tree reconciliation completed before any product edit:

| Identity | Original | Reconciled | Tree | Parents |
|---|---|---|---|---|
| Recovered baseline | `21cb28a0219f8b6b588479ce0fd8e4099464869d` | `0b0cd4aa8c47358311ac3139c54366196de2ec48` | `32e3df17dea75d69cec66f2b00dfa0766c97172a` | old `main` `9c93170…`, RC3/P7 `dfabbfd…` |
| RC4 | `1f75f244e8fe59ff798202630bf416dce4d31b85` | `e7d6098cf44482a5114eef50976a93697ca2282a` | `4b21c79e5aa8866454f960ba5071d1b6b3afab06` | reconciled baseline |

Both tree equalities, ancestry from old `main` and RC3/P7, the old-main-only nine commits, the RC3/P7-only 196 commits, and `git fsck --strict --full` passed. The current repository imported only the verified objects, created local `main` at reconciled RC4, retained an unpushed safety ref to the original local RC4, and still had no remote.

The protected-assets manifest covers 29 files totaling 1,873,848,872 bytes. Its SHA-256 is `2b3281765b4385c31428b8b5c9eca45c87a953124d6b51e2100a17ad044095a4`; protected-path count in the proposed public tree was zero.

## Product implementation

- `QueryScopeControl` is again a descendant of the left search-control rail and is rendered before `FilterPanel`.
- The ordinary Course route always renders both work areas. Candidate edits, Apply, Search, idle, loading, results, empty, error, and detail do not replace the parent grid.
- The RC3 idle `StatePanel` is restored in the right work area. Apply to a new scope clears the prior submission and returns that area to idle; Search changes only the right-side content.
- The Local `2 × 5` and Public `2 × 3 + Search` matrices retain the RC4 cells and state machine, but they now describe only the internal QueryScope arrangement.
- The RC3 `47.999rem` workspace boundary is restored. QueryScope collapses to semantic one-column order only at the same narrow workspace boundary.
- SearchSession again stores and restores `filterScrollTop`; the RC4 window-page-scroll replacement was removed.
- The desktop rail is sticky, dynamically viewport-bounded, independently scrollable, focusable, named, and product-styled. It supports mouse wheel, touch pan, Home, End, PageUp, and PageDown without intercepting keys from descendant controls.
- Long filter option collections retain their separate styled scrolling. No content is hidden or height-clipped.
- Query V3, dynamic course-number bands, neutral filters, per-field incomplete-data semantics, NB/NK/CM, Local/Pubic term windows, Pull rules, performance serving, and Public zero-surface are unchanged.
- No backend wire contract, Query schema, or Service Status contract changed in RC5.

## Mandatory UI Stage 1

The main implementer read and applied `industrial-brutalist-ui` and `design-taste-frontend` immediately before UI source work. Direct user authority kept the existing Swiss Industrial Print language and prohibited generic rounded cards, gradients, icon libraries, decorative motion, and framework replacement.

The integrated Stage 1 reference is stored under `evidence/round-05/stage-1`:

- composition matrix: 56 PNGs; 22/22 Local/Public × locale × viewport scenarios passed;
- Course/Section flow: 9 PNGs; 7/7 scenarios passed;
- fixed viewports: 390, 768, 1440, 1920, 2560; 320 is additional stress only;
- assertions include exact matrix cells, QueryScope ancestry, persistent idle right surface, Public zero-surface, axe, focus, keyboard traversal, reduced motion, no horizontal overflow, styled scroll metrics, wheel/touch reachability for long lists, and collapsed result Sections.

The 1440 search-idle images capture the complete left/right composition before Local navigation or Public Watch exercises. Course-result images capture the same skeleton after Apply and Search.

## Mandatory UI Stage 2 — `emil-design-eng`

Stage 2 began only after the Stage 1 product integration, tests, browser matrix, and visual inspection completed. The review compared the RC3 code/DOM structure, the RC4 defect state described by the protected HumanTest evidence, and actual RC5 Stage 1 screenshots.

| Before | After | Why |
|---|---|---|
| RC4 placed `.bcsp-search-workspace__scope` before both columns and spanned `grid-column: 1 / -1`. | QueryScope is a stable descendant of `.bcsp-search-workspace__filters`; the desktop rail and right work area are siblings. | `2 × 5` and `2 × 3 + Search` define the control’s internal grid, not the page skeleton. |
| RC4 hid the right result surface while idle and expanded the filter side across the workspace. | The right work area is present from startup and renders the RC3 localized idle `StatePanel`; Apply and Search preserve both column nodes. | Restores the most recent inherited interaction model and prevents layout movement when state changes. |
| RC4 persisted window `pageScroll` and removed the independent filter rail. | SearchSession persists `filterScrollTop`; the desktop rail is sticky and viewport-bounded, while narrow screens return it to normal document flow. | Users can inspect results without losing their place in a long control console. |
| The restored rail initially had a styled scrollbar but only implicit touch behavior. | The rail explicitly uses `touch-action: pan-y`, flat thin scrollbar styling shared with long option lists, and `overscroll-behavior: contain`. | Makes the visual and input contract intentional without hiding, cancelling, or clipping scrolling. |
| A focusable scroll region was assumed to provide reliable Home/End behavior. Chrome moved only one viewport on the first Stage 2 `End` assertion. | Home/End/PageUp/PageDown are handled when and only when the rail itself owns focus; wheel and CDP touch-pan remain native. | Closes a measured keyboard gap without stealing keys from checkboxes, radios, selects, or text fields. |
| The first restoration used only `100vh`. | `100vh` remains the compatibility fallback and is followed by RC3-style `100dvh`. | Keeps the rail within the actually visible browser viewport when browser chrome changes. |
| A container-width rule collapsed QueryScope inside every desktop-sized RC3 rail. | The internal matrix collapses only at the inherited `47.999rem` page-workspace breakpoint. | Preserves the exact Local/Public desktop matrices at 768 and wider while retaining the frozen narrow semantic order. |
| UI review could have introduced motion as generic polish. | No QueryScope, rail, idle/result swap, or keyboard action animation was added; existing reduced-motion assertions still report effectively zero interactive transition time. | High-frequency search controls should respond immediately and preserve the project’s static industrial character. |

Every accepted finding was implemented before Stage 2 evidence. `evidence/round-05/stage-2` preserves a separate 56-image composition set and 9-image Course/Section set; it did not overwrite Stage 1.

Stage 2 browser results:

- composition matrix: 22/22;
- Course/Section flow: 7/7;
- Local and Public filter rails each passed styled-scroll, dynamic-height, wheel, touch, Home/End/PageUp/PageDown, axe, reduced-motion, keyboard, and overflow assertions;
- the five frozen viewports and the 320 stress viewport passed in English and Chinese where required;
- the source was visually inspected at Local idle, Public idle, Local results, and narrow first-view states.

## Locked source gates

- Rust/Cargo 1.97.0, project cache, locked and offline:
  - `cargo fmt --all -- --check`: PASS;
  - workspace all-target test replay: PASS — 603 ordinary tests, zero failures; evidence-only runners remained ignored unless invoked below;
  - workspace all-target Clippy with `-D warnings`: PASS;
  - cargo-deny 0.20.2 advisories, bans, licenses, and sources: PASS.
- Architecture:
  - architecture self-tests: PASS — 2/2;
  - Rust graph: PASS — 15 members, Local/Public binaries, 18/18 Public SOURCE denies;
  - Public Rust zero-surface: PASS — SOURCE/API/STORAGE/PACKAGE each 18/18, 12-crate closure, zero features.
- Frontend, clean offline install using Node 24.18.0 and npm 11.16.0:
  - `npm ci --offline`: PASS — 119 packages, zero vulnerabilities;
  - import/build guard: PASS — 83/83;
  - Vitest: PASS — 22 files, 170/170;
  - TypeScript: PASS;
  - Local production build/capabilities: PASS;
  - Public production build/zero-surface: PASS — 76 assertions.
- Packaging contract checks reproducible on Windows:
  - Bash syntax: PASS — 10 deployment/Linux scripts;
  - ShellCheck 0.11.0 at error severity: PASS;
  - Node syntax: PASS — metadata generator and both browser candidate scripts;
  - PowerShell parser: PASS — Windows build, Windows verify, and joint-release verify scripts.
- Protected evidence replay: PASS — 29/29 paths retained exact bytes and SHA-256; zero protected paths are eligible for staging.

The first Rust workspace attempt reached the test binaries but was terminated by the command runner’s 120-second limit, which closed test output pipes. A subsequent warm, unbounded replay completed cleanly in 40.8 seconds with zero test failures; the timeout is not treated as a product result.

## Frozen performance and concurrency revalidation

RC5 changes no query, service-status, or HTTP wire contract. The optimized V3 functional runner was rebuilt and rerun in release mode against an exact copy of the original three-file oracle database bundle. The copy matched the frozen input hashes before opening. The tracked artifact is `evidence/round-05/performance/v3-optimized-reference-final.json`, SHA-256 `04e5c08a8f35a015728e945400b99a1bb15abca8a952ec98d334be88df568959`.

| Surface | Warm p95 | Warm max | Budget p95 / max | Result |
|---|---:|---:|---:|---|
| Service status | 4.607 ms | 6.482 ms | 250 / 1,000 ms | PASS |
| Complete three-Campus course-number-band options | 0.0093 ms | 0.0093 ms | 500 / 1,500 ms | PASS |
| Single-Campus neutral Search | 6.157 ms | 6.157 ms | 1,000 / 2,000 ms | PASS |
| Three-Campus complex Search | 55.353 ms | 55.353 ms | 2,000 / 5,000 ms | PASS |

The runner reports `allBudgetsPass=true` and exact status/options/Search/validation witnesses against the authoritative unoptimized V3 oracle. The workspace replay also passed the deterministic file-backed publish/Search barrier, pinned-generation/no-mixed-vector tests, exact dynamic-validation publication-race test, prepared/reference parity test, and failed-refresh LKG serviceability test.

## Detached Windows live-path correction

The pre-freeze detached Windows candidates passed archive, static-CRT, manifest, provenance, SBOM, first-run, upgrade, restart, reset, Unicode-path, and real-Chrome checks. Separate live Rutgers checks then reached `READY` with current discovery and all six current/next NB/NK/CM targets usable, but exposed shutdown defects that the intentionally network-disabled package verifier could not exercise: the authenticated exit request returned HTTP 204 and stopped the HTTP surface, while active official-refresh work kept the process alive beyond the required bound.

No remote action had occurred. Each candidate process was addressed by exact process identity and every diagnostic database remained confined to the protected build cache. Reopening the same large WAL database with the Rutgers network task disabled completed graceful exit, checkpoint, and sidecar cleanup in 1.488 seconds, isolating the delay to active live-refresh work rather than ordinary database close.

The official refresh supervisor now receives its existing graceful shutdown signal while the prepared-serving worker receives a cooperative cancellation request before its bounded drain. Full prepared rebuilds check that request at storage, FTS-document, Catalog-target, corpus identity/relationship/index loops, Open-overlay, consistency, and dictionary boundaries, so a large `spawn_blocking` projection releases its read snapshot instead of keeping process shutdown alive. The runtime also captures SQLite's thread-safe interrupt handle before starting refresh work; shutdown invokes it without waiting for the operational-storage mutex, causing an active blocking Catalog/Open statement to fail and roll back instead of extending WAL growth and the final checkpoint. After refresh, watch and HTTP shutdown complete, the Local lifecycle now drops the complete prepared runtime—operational, policy, history, mutation and route connections—before reopening one dedicated connection for the final `TRUNCATE` checkpoint; the checkpoint can no longer wait on an internal reader it still owns. Prepared-serving shutdown releases publication/storage resources before the official supervisor is joined; the supervisor retains a five-second backup drain window and is aborted if it still fails to cooperate. Focused regressions prove both that a deliberately non-cooperative supervisor has a bounded join and that a captured interrupt handle stops an active operational statement, while the existing file-backed publication-barrier test verifies that ordinary prepared replacement and pinned-generation semantics remain unchanged. This is an internal lifecycle correction: it changes no HTTP wire contract, Query V3 schema, Service Status contract, refresh cadence, publication rule, or serving behavior.

## Commit and package boundary

The source and evidence are ready for the one authorized RC5 commit, followed by detached clean-worktree Windows construction. Windows package identity, the later main push, Linux-only workflow, cross-package verification, and final remote single-`main` state execute after the commit and therefore are recorded in the local protected post-candidate handoff ledger. Final package hashes and workflow IDs cannot be embedded in the commit that defines their source SHA without changing that SHA.
