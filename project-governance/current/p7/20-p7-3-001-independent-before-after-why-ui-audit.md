# P7.3-001 — Independent Before / After / Why UI audit

- Task: `P7.3-001`
- Parent: `f6f0db475e4ef1c8916c86b4178971c49db08a17`
- Branch: `codex/p7-implementation`
- Skill used: `$emil-design-eng`
- Scope: integrated UI polish only; no product capability or protocol expansion
- Next task after PostPush: `P7.3-002`

## Baseline

The P7.2 baseline is reproducible: the real-Chrome local/public × `en-US`/`zh-CN`
× desktop/mobile matrix passes 8/8, the frontend suite passes 89/89, both target
builds pass, and public local-surface assertions pass 72/72. PostPush run
`29371127037` completed successfully for the exact parent revision.

The audit used those eight full-page screenshots, keyboard interaction paths, and
the integrated source. It found product-facing polish defects rather than a need
for another validation framework.

## Findings to implement one-for-one

| Before | After | Why |
| --- | --- | --- |
| **A01 · P0 — The Watch rail always says “A watched Section is open,” even with 0 selected, 0 active, and 0 Open episodes.** | Use the neutral localized Watch-desk explanation in the shell; reserve Open wording for a real episode, alert, or toast. | A false positive in the product’s most trust-sensitive status undermines every later notification. |
| **A02 · P0 — Local form focus styles reference undefined `--bcsp-focus`, so the higher-specificity rule can erase the visible keyboard ring.** | Define the focus token with the accent as a fallback and test computed focus visibility on Saved Views and Settings controls. | Keyboard operation must have a visible location; an axe-clean DOM does not prove a rendered focus indicator. |
| **A03 · P0 — At 390 px the shell spends most of the first viewport on six one-column nav rows, copyright, and single-column status cells.** | Keep three navigation columns at 390 px, use two at the narrowest supported width, keep status summaries 2 × 2 until the narrowest breakpoint, and move the mobile copyright after the workspace. | Users should reach the current task in the first viewport without losing 44 px targets or the industrial hierarchy. |
| **A04 · P1 — SPA navigation changes the route while focus and deep scroll remain at the previous control; active links expose only `data-active`.** | On a forward route change, instantly focus and reveal the workspace heading; preserve browser back/forward restoration; add `aria-current="page"` to the active link. | Route changes must be announced and usable without making keyboard users search for the new page. |
| **A05 · P1 — Search/detail/pagination and destructive inline confirmations can unmount the focused trigger; invalid-filter correction always smooth-scrolls.** | Move focus to the new result/detail/confirmation target, restore it on Back or Cancel, and use instant validation scroll followed by focus on the invalid control. | These are repeated keyboard workflows; focus loss and forced animation make the interface feel broken and slow. |
| **A06 · P1 — Local pages repeat the same title and intro in the shell and inner hero; Watch repeats “Watch desk” immediately inside the page.** | Keep one canonical page title and turn inner blocks into task-level labels or compact context only. | Removing false hierarchy recovers substantial mobile space without deleting information. |
| **A07 · P1 — Settings begins as “ready to save” with an enabled Save action even when nothing changed.** | Model `unchanged → unsaved → invalid → saving → saved/failed`; disable Save while unchanged and announce the result in place. | Users need to know whether a change exists and whether it reached persistent local state. |
| **A08 · P1 — Hover styles run on touch, press feedback is inconsistent, several targets are 35–38 px, checked Watch cards are visually weak, and disabled accent buttons remain prominent red blocks.** | Gate hover to fine pointers; give pressables a 100–160 ms transform-only response that reduced-motion removes; make targets at least 44 px; strengthen native checked/focus-within states; render disabled actions neutrally. | The UI should visibly acknowledge a tap, remain thumb-friendly, and never make an unavailable action look primary. |
| **A09 · P1 — At 390 px both four-cell status summaries collapse to long single columns, Watch policy cards become cramped, and zero-selection batch actions consume space while unusable.** | Retain 2 × 2 summaries at 390 px, stack policy cards only where their text needs it, and show selection-dependent batch actions only when a selection exists. | The same information becomes scan-friendly and the empty Watch path tells the user what to do next. |
| **A10 · P2 — Watch toasts appear and disappear abruptly; transient status notices can occupy the viewport until manually dismissed.** | Give toasts an interruptible 160–200 ms transform/opacity entrance and faster exit, opacity-only under reduced motion; auto-retire non-alert status notices while keeping alert notices manual and pausing timers during hover, focus, or a hidden document. | Occasional feedback benefits from spatial continuity, but routine status must not become permanent obstruction. |

## Constraints to preserve

- Add no motion library and animate no layout property; use CSS transform/opacity.
- Keep interaction motion below 200 ms and remove movement under reduced motion.
- Retain the square, flat Swiss-industrial visual language and both locale catalogs.
- Do not change Rust/API/WebSocket contracts or local/public capability boundaries.
- P7.3-002 must link every code change to `A01`–`A10`; unrelated redesign is out of scope.

## P7.3-002 verification target

Use focused product checks: 390 × 844 and 320 × 568 first-view screenshots; empty,
selected, and Open Watch states; the five Settings states; keyboard route,
search/detail/Back, pagination, and confirmation flows; pointer/touch/reduced-motion
interaction; axe, contrast, overflow, both production builds, and the existing
public boundary. P7.3-003 will then regenerate the final full browser evidence.
