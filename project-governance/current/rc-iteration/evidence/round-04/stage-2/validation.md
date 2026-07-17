# RC I Round 4 UI Stage 2 Validation

## Identity

| Field | Value |
|---|---|
| Review method | `emil-design-eng` applied after integrated Stage 1 evidence |
| Product language | Swiss Industrial Print |
| Node | 24.18.0, project cache runtime |
| npm | 11.16.0, offline project cache |
| Browser | Installed Chrome via Playwright 1.61.1 |
| Remote action | None |

## Implemented review findings

| Before | After | Why |
|---|---|---|
| Four Section facts in a three-column grid left two black cavities. | `Permission to add` spans the complete row; browser geometry verifies both edges. | Restores intentional grid rhythm without changing the semantic definition list. |
| English filter labels exposed technical vocabulary and abbreviations. | Course level, Prerequisites, Class format, Meeting timing, In person, Synchronous, Asynchronous. | Uses student-facing language while raw contract values remain unchanged. |
| Selected unpublished/unknown term metadata visibly explained publication beside disabled Pull. | QueryScope exposes no visible publication explanation, tooltip, or `title`; Pull remains visible and disabled, with its reason retained only through a hidden `aria-describedby` target. | Implements the latest direct user ruling across the whole visual matrix without dropping the frozen accessibility requirement. |
| UI-motion rules were reviewed manually per component. | Static duration/easing audit plus rendered reduced-motion assertion. | Makes the accepted polish constraints regression-resistant. |
| A rapid Chinese Figure 1 recapture could precede font/paint settling. | Capture waits for fonts plus two animation frames. | Prevents black-block screenshot artifacts from contaminating evidence. |

## Gates

```text
npm run verify                                      PASS
import/target guard                                 PASS (83/83)
Vitest                                              PASS (22 files, 170/170)
TypeScript                                          PASS
Local build + capability verification               PASS
Public build + zero-local-surface verification      PASS (76 assertions)
capture-composition-matrix.mjs                      PASS (22/22)
capture-course-section-flow.mjs                     PASS (7/7)
```

The composition set exercises Local/Public × English/Chinese at 390, 768, 1440, 1920, and 2560 pixels, plus 320-pixel stress evidence. It includes axe, keyboard traversal, fixed focus, reduced motion, semantic narrow-screen order, exact matrix cells, published, unpublished and publication-unknown Figure 1 Pull states, and overflow checks.

The Course/Section set verifies V3 combined filters, the collapsed-by-default result disclosure, direct typed Section routing, full-row Section fact geometry, styled scrollbar metrics, wheel, touch, sequential Tab, Home/End, first/last option reachability, and no content clipping.

For each locale, the unpublished and publication-unknown Figure 1 PNGs are byte-identical, proving that no publication explanation changes the rendered pixels. Browser assertions independently verify the distinct hidden `aria-describedby` text, disabled state, ordinary Pull accessible name, absent `title`, and zero visible publication copy.

## Evidence layout

- `composition/`: 52 images.
- `course-section/`: 9 images.
- Stage 1 remains intact at `../stage-1/`; Stage 2 did not replace it.

No commit, push, tag, upload, deployment, remote CI, or other remote operation was performed by this stage.
