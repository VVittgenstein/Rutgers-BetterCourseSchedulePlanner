# RC Iteration Round 5 final discussion and design decision record

```text
document_status: DESIGN_FROZEN
round_05_product_status: NOT_STARTED
pre_rc5_reconciliation_status: COMPLETE
remote_write_status: NOT_STARTED
```

## 1. Authority and purpose

Round 5 repairs the rejected Round 4 search-page composition without discarding the valid Round 4 product contracts. Its authoritative relationship is:

```text
RC3 recent product baseline
+ valid RC4 intent and contracts
- incorrect RC4 interpretation and execution
= RC5
```

The latest HumanTest overrides the Round 4 automated PASS wherever they conflict. The original conversation and three original HumanTest images remain protected local evidence and are intentionally not copied into the public Git tree. This record contains only the decisions required to implement and audit the repair.

## 2. HumanTest findings and supersession

The rejected RC4 candidate changed a page-level layout contract when it should only have changed the contents of the left search controller:

- QueryScope was promoted above the search columns and stretched across the workspace.
- The right waiting/results workspace disappeared during the initial state.
- The left filter controller lost its independent sticky scroll relationship.
- SearchSession replaced the RC3 `filterScrollTop` contract with page-scroll restoration.

Round 5 supersedes those changes:

1. The normal search route always has the RC3 fixed left controller and right waiting/results workspace on desktop.
2. Local `2 x 5` and Public `2 x 3 + Search` describe QueryScope cells inside the left controller, not the whole page.
3. Apply changes candidate/applied state and may reset right-side content; Search changes right-side content. Neither operation reconstructs the page skeleton.
4. The left controller regains sticky, viewport-bounded independent scrolling and `filterScrollTop` session restoration.
5. The right RC3 idle StatePanel, including its localized prompt and centered minimum-height relationship, is restored. This latest waiting-window decision supersedes the RC4 removal of that right-side idle surface.
6. The old large left-side QueryScope explanation/hero remains removed. The compact RC4 QueryScope is retained and embedded correctly.

## 3. Contracts retained from Round 4

The repair does not roll the product back to RC3. It retains:

- Query Contract V3 and all V3 normalization, migration and uncertainty semantics;
- Local previous two/current/next two terms and Public current/next terms;
- the NB/NK/CM allowlist and Public zero-surface boundary;
- Local term-level Pull and the shared candidate, Apply, Applied and Search flow;
- disabled Pull without visible publication-reason copy for unpublished or unknown future terms;
- dynamic complete course-number bands;
- the continuously visible 03-18 filter sequence;
- neutral empty selections and the three independent include-incomplete switches;
- version-bound query serving, the measured performance budgets and refresh/search concurrency guarantees.

RC3's old Campus set, Query V2 UI, old filter meanings and the two collapsed black filter groups must not return.

## 4. Local and Public composition

Both products use the same SearchWorkspace, QueryScope component, SearchSession and action handlers.

- Local desktop: the left QueryScope contains the exact five-row matrix of five real terms opposite NB, NK, CM, scope action and Search.
- Public desktop: the left QueryScope contains current/NB, next/NK, CM/scope action, followed by the two-column Search row.
- Public does not synthesize terms and exposes no manual Pull in source, DOM, API, capability, bundle or package.
- At the RC3 narrow layout boundary, the search workspace becomes one column and QueryScope follows semantic order: terms, campuses, scope action, Search, filters, results.

Necessary internal scroll regions remain operable by wheel, touch and keyboard. Their visible scrollbars use the product's paper/ink treatment; content must not be hidden or clipped to remove native scrollbar chrome.

## 5. Required UI process

Round 5 UI work has two ordered implementation stages:

1. Read and apply `industrial-brutalist-ui` plus `design-taste-frontend` before changing UI source. Restore structure, responsive behavior, scrolling and accessibility against the real RC3 implementation.
2. Only after the integrated first-stage validation, read and apply `emil-design-eng`. Produce and implement a real `Before | After | Why` audit using the RC3 structural reference, the rejected RC4 DOM/screenshots and the running RC5 Local/Public output.

The skills guide execution quality; they cannot override the HumanTest decision or invent a different search-page skeleton.

## 6. Pre RC5 history reconciliation

The cleaned local repository was reconciled before product work without merging file contents:

| Snapshot | Old commit | Reconciled commit | Tree |
|---|---|---|---|
| recovered baseline | `21cb28a0219f8b6b588479ce0fd8e4099464869d` | `0b0cd4aa8c47358311ac3139c54366196de2ec48` | `32e3df17dea75d69cec66f2b00dfa0766c97172a` |
| RC4 | `1f75f244e8fe59ff798202630bf416dce4d31b85` | `e7d6098cf44482a5114eef50976a93697ca2282a` | `4b21c79e5aa8866454f960ba5071d1b6b3afab06` |

The reconciled baseline has old remote main `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` as its first parent and old P7/RC3 `dfabbfdcbea0cb90021ed59d11ccb38c29b19fa7` as its second parent. Both historical lines are therefore ancestors of reconciled RC4. Tree equivalence, ancestor checks, protected-path exclusion and strict object verification passed before the local `main` branch was created.

This reconciliation is history construction, not an additional Round 5 product commit.

## 7. Round completion and remote model

Round 5 ultimately retains one product commit relative to reconciled RC4. Its public author and committer identity is:

```text
VVittgenstein <158061732+VVittgenstein@users.noreply.github.com>
```

The sequence is fixed:

```text
implementation and local gates
-> one RC5 commit
-> detached Windows build and verification
-> fast-forward public main
-> Linux-only workflow at the exact RC5 commit
-> Windows/Linux joint verification
-> remove the old P7 branch pointer
-> next HumanTest
```

No force push, PR, tag, Release, deployment, Vultr, DNS or production operation belongs to this round. Final artifact hashes and workflow identifiers are post-commit evidence and remain in the local candidate handoff ledger so that recording them cannot change the packaged source identity.
