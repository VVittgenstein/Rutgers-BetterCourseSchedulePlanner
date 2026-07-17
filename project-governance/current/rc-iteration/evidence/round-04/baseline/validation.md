# RC I Round 4 — recovered RC3 frontend baseline audit

## Identity and limits

- Source: detached, clean worktree at recovered baseline commit `21cb28a0219f8b6b588479ce0fd8e4099464869d`.
- Runtime: locked Node `24.18.0`, npm `11.16.0`, offline npm cache.
- Human-rendered Before evidence: the five immutable files under `assets/round-04-human-test/`, especially Figure 4 for the disclosure groups and Figure 5 for the visually foreign scrollbars.
- This is a source/DOM/test audit. It does **not** claim that the recovered RC3 Chrome composition matrix passed: the legacy harness waited for a stale Local READY selector and timed out before completing its Chrome axe/keyboard matrix.

## Replayed source gate

The baseline was reinstalled with `npm ci --offline` and `npm run verify` was replayed from the detached worktree. Results:

| Gate | Baseline result |
|---|---|
| import and target graph guard | pass; 83/83 guard tests |
| Vitest | pass; 22 files, 161/161 tests |
| TypeScript build | pass |
| Local production build | pass; 21 allowed capabilities |
| Public production build | pass; 14 allowed capabilities and 76 Public artifact assertions |

The detached source worktree remained clean after the replay.

## Audited RC3 DOM and interaction structure

The following blob identities bind this audit to the recovered baseline rather than the evolving Round 4 worktree:

| Baseline file | Git blob |
|---|---|
| `QueryScopeControl.tsx` | `e3528269fbb6d1c6d8a1c8a674f2d474f9e2a269` |
| `SearchWorkspace.tsx` | `8fdab622e9de90d1da63b51c8bf460c131d75c16` |
| `FilterPanel.tsx` | `f58d730a2bbedeac2f38eaf6b50589f7794b78be` |
| `SearchSession.tsx` | `e5f66b27761af1dbbc1c87a3b9fda0cb38c3ba2c` |
| `ui-shell.test.tsx` | `7a6b04ee44067bb6f975298edb3ba1ab7c6ddfcf` |
| `capture-composition-matrix.mjs` | `57b1b2a098f452ad9cf81611b047d5e6a9971fe2` |

Observed baseline facts:

- QueryScope used native radios, checkboxes and buttons inside two semantic `fieldset` groups, but wrapped them in the oversized `01–02 / TARGET` panel shown in HumanTest Figure 2. Apply remained a separate header action and the Search hero remained elsewhere in the filter form.
- The filter DOM contained two native `<details>` groups for `03–09` and `10–18`; their independent open/focus/scroll behavior matches HumanTest Figure 4.
- `.bcsp-search-workspace__filters` was an internal `overflow: auto` rail. `SearchSession` persisted `filterScrollTop`, and `SearchWorkspace` restored it. Long subject and dictionary lists added further `overflow: auto` regions without the final Round 4 product scrollbar styling, matching HumanTest Figure 5.
- Baseline keyboard tests covered native result disclosure, searchable dictionary selection, filter-group keyboard activation, focus restoration and rail scroll restoration. These 161 jsdom tests passed in the replay.
- Baseline jsdom accessibility tests ran `axe-core` for the shell and populated Watch workspace and passed as part of the 161-test gate. The Chrome composition script also contains full-document axe, Tab traversal and horizontal-overflow assertions, but its stale READY wait prevented those browser assertions from becoming baseline pass evidence.

## Round 4 implication

This audit and the five HumanTest images are the Before side of the mandatory UI sequence. Round 4 must preserve the useful native semantics while replacing the oversized panels, removing the filter disclosure groups and filter rail state, and either hiding or product-styling necessary nested scrollbars without losing wheel, touch or keyboard reachability.
