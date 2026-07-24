# Search controls Stage 2 audit

## Before | After | Why

| Before | After | Why |
|---|---|---|
| Apply, Search, and Local Pull exposed disabled text but no machine-readable busy state. | Pending actions retain stable button copy and expose `aria-busy="true"`; adjacent status copy remains the detailed explanation. | Preserves button width and recognition while giving assistive technology an explicit operation state. |
| Mouse, keyboard, and touch shared mostly static feedback. | Fine-pointer hover uses short color transitions; pointer/touch activation uses a 1px press response; keyboard feedback remains focus-based without movement. | Makes direct manipulation feel immediate without making keyboard navigation visually jump. |
| Selected term, Campus, checkbox, and dictionary choices relied mainly on native control marks. | Selected rows receive restrained accent confirmation and focused rows receive an inset focus treatment. | Improves state legibility without changing the RC5 information hierarchy or semantics. |
| Validation and busy feedback appeared abruptly, while dictionary lists and active-filter chips are also present on initial render. | Only validation and explicit busy-state changes use one-shot 160ms emphasis; dictionary lists and chips do not animate on mount. | Preserves continuity for user-triggered feedback without turning initial page composition into an entrance animation. |
| Reduced-motion inherited tiny non-zero durations from the general motion layer. | `prefers-reduced-motion: reduce` disables animation and uses `0ms` transitions; press transforms are removed. | Meets the existing browser contract and avoids vestibular movement. |
| Busy feedback could have become a continuous progress animation. | Busy surfaces use a static state tint plus a single 160ms confirmation, with no infinite animation. | Communicates progress without persistent distraction or implied upstream timing guarantees. |

## Boundaries retained

- No information-architecture, filter-contract, QueryScope geometry, or Local/Public capability changes.
- Desktop and 768px retain the left independent control rail and right workspace; only widths below `47.999rem` use document flow.
- Local remains `2 × 5`; Public remains `2 × 3 + Search`.
- All `03–18` filters remain continuously visible.
- Motion is limited to the shared search-control surface and the Local Pull action injected into it.
