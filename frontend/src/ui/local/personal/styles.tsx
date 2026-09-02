/*
 * Local pages: saved views, history, settings (design spec v2: sections 4.2,
 * 4.5, 4.7, 4.9, 5.6-5.8, 11.1). Tokens only, weights 400/600, no uppercase
 * or tracking; every card/tile/fact grid fills its row.
 */
const LOCAL_PERSONAL_CSS = String.raw`
.local-personal { display: grid; max-width: 64rem; gap: var(--bcsp-space-4); margin-top: var(--bcsp-space-1); }
.local-personal__meta, .local-personal__status { margin: 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-meta); font-weight: 400; letter-spacing: 0; line-height: var(--bcsp-lh-meta); text-transform: none; }
.local-personal__label { margin: 0; color: var(--bcsp-ink); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-transform: none; }

/* ---- Section cards (spec 4.5, 4.6) ---- */
.local-personal__section { display: grid; align-content: start; gap: var(--bcsp-space-3); min-width: 0; padding: var(--bcsp-space-4); border: 1px solid var(--bcsp-line); border-radius: var(--bcsp-radius-3); background: var(--bcsp-paper-raised); }
.local-personal__section-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--bcsp-space-2); min-height: var(--bcsp-control-h); }
.local-personal__section-head > p { flex: 1 1 100%; margin: 0; color: var(--bcsp-ink-muted); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); }
.local-personal__section-title { margin: 0; font-size: var(--bcsp-text-title); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-title); text-transform: none; }
.local-personal__section-title:focus-visible { outline: 2px solid var(--bcsp-focus); outline-offset: 4px; border-radius: var(--bcsp-radius-1); }

/* ---- Forms and fields (spec 4.2) ---- */
.local-personal__form, .local-personal__grid, .local-personal__list { margin: 0; padding: 0; list-style: none; }
.local-personal__form { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: end; }
.local-personal__field { display: grid; gap: var(--bcsp-space-1); min-width: 0; }
.local-personal__field > span, .local-personal__field label { color: var(--bcsp-ink); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-transform: none; }
.local-personal input:not([type='range']):not([type='checkbox']):not([type='radio']), .local-personal select { width: 100%; min-width: 0; height: var(--bcsp-control-h); min-height: var(--bcsp-control-h); padding: 0 var(--bcsp-space-2); border: 1px solid var(--bcsp-line-strong); border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink); background: var(--bcsp-paper-raised); font: inherit; font-size: var(--bcsp-text-body); transition: border-color var(--bcsp-dur-1) var(--bcsp-ease-out); }
.local-personal input:focus-visible, .local-personal select:focus-visible { border-color: var(--bcsp-focus); outline: 3px solid var(--bcsp-focus); outline-offset: 0; }
.local-personal input:disabled, .local-personal select:disabled { color: var(--bcsp-ink-muted); background: var(--bcsp-surface-3); opacity: 1; }
.local-personal input[aria-invalid='true'], .local-personal input:invalid:not(:placeholder-shown) { border-color: var(--bcsp-danger); }
.local-personal select { appearance: none; padding-right: 2.25rem; }
.local-personal__select-control { position: relative; display: block; min-width: 0; }
.local-personal__select-control::after { content: ''; position: absolute; top: calc(50% - 0.3125rem); right: 0.875rem; width: 0.375rem; height: 0.375rem; border-right: 1.5px solid currentColor; border-bottom: 1.5px solid currentColor; transform: rotate(45deg); pointer-events: none; }
.local-personal input[type='range'] { width: 100%; }
.local-personal__form > .bcsp-action { min-width: 0; }

/* ---- Card lists: flex wrap with growing items so every row is filled (spec 11.1) ---- */
.local-personal__list { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-3); }
.local-personal__list > li, .local-personal__history > li { flex: 1 1 18rem; min-width: 0; }
.local-personal__history > li { flex-basis: 20rem; }
.local-personal__card { display: grid; align-content: start; gap: var(--bcsp-space-2); min-width: 0; padding: var(--bcsp-space-3); border-radius: 0.5rem; background: var(--bcsp-surface-2); }
.local-personal__card h4 { margin: 0; overflow-wrap: anywhere; font-size: var(--bcsp-text-subtitle); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-subtitle); text-transform: none; }
.local-personal__card h4:focus-visible { outline: 2px solid var(--bcsp-focus); outline-offset: 4px; border-radius: var(--bcsp-radius-1); }
.local-personal__card > p { margin: 0; color: var(--bcsp-ink-muted); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); }
.local-personal__identity { display: flex; flex-wrap: wrap; align-items: flex-start; justify-content: space-between; gap: var(--bcsp-space-2); }
.local-personal__identity > div { min-width: 0; }
.local-personal__actions { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-1); align-items: center; }

/* ---- Badges: one tone map (spec 4.7) ---- */
.local-personal__badge { display: inline-flex; align-items: center; gap: 0.375rem; height: 1.375rem; width: fit-content; padding: 0 var(--bcsp-space-1); border: 1px solid var(--bcsp-line); border-radius: var(--bcsp-radius-pill); color: var(--bcsp-ink-2); background: var(--bcsp-surface-2); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-micro); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-micro); text-transform: none; white-space: nowrap; }
.local-personal__badge[data-state='CLEAN'], .local-personal__badge[data-state='READY'] { color: var(--bcsp-ok); border-color: var(--bcsp-ok-line); background: var(--bcsp-ok-tint); }
.local-personal__badge[data-state='MODIFIED'], .local-personal__badge[data-state='REVIEW_REQUIRED'] { color: var(--bcsp-warn); border-color: var(--bcsp-warn-line); background: var(--bcsp-warn-tint); }
.local-personal__badge[data-state='DANGER'], .local-personal__badge[data-state='INCOMPATIBLE'] { color: var(--bcsp-danger); border-color: var(--bcsp-danger-line); background: var(--bcsp-danger-tint); }

/* ---- Notes and banners (spec 4.9) ---- */
.local-personal__notice { margin: 0; color: var(--bcsp-ink-muted); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); }
.local-personal__notice p { margin: 0; }
.local-personal__notice ul { margin: var(--bcsp-space-1) 0 0; padding-left: 1.25rem; }
.local-personal__notice[role='alert'], .local-personal__notice[role='status'], .local-personal__notice[role='group'], .local-personal__notice--review { display: grid; gap: var(--bcsp-space-1); padding: var(--bcsp-space-2) var(--bcsp-space-3); border: 1px solid var(--bcsp-info-line); border-radius: 0.5rem; color: var(--bcsp-ink); background: var(--bcsp-info-tint); }
.local-personal__notice--review { border-color: var(--bcsp-warn-line); background: var(--bcsp-warn-tint); }
.local-personal__notice[role='alert'], .local-personal__notice[role='group'] { border-color: var(--bcsp-danger-line); background: var(--bcsp-danger-tint); }
.local-personal__notice[role='group'] .local-personal__actions { margin-top: var(--bcsp-space-1); }

/* ---- Settings form: explicit two-column grid, never an empty track (spec 5.8, 11.1) ---- */
.local-personal__settings { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--bcsp-space-4) var(--bcsp-space-5); }
.local-personal__settings > .local-personal__field:last-child:nth-child(odd) { grid-column: 1 / -1; }
.local-personal__reset-grid { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-3); }
.local-personal__reset-grid > * { flex: 1 1 16rem; min-width: 0; }
.local-personal__history { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-3); }

/* ---- Fact grids: explicit two columns, odd last item spans (spec 4.10, 11.1) ---- */
.local-personal__facts { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--bcsp-space-2) var(--bcsp-space-4); margin: 0; }
.local-personal__facts > div { min-width: 0; }
.local-personal__facts > div:last-child:nth-child(odd) { grid-column: 1 / -1; }
.local-personal__facts dt { color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-meta); letter-spacing: 0; line-height: var(--bcsp-lh-meta); text-transform: none; }
.local-personal__facts dd { margin: 0.125rem 0 0; overflow-wrap: anywhere; color: var(--bcsp-ink); font-feature-settings: "tnum" 1; font-size: var(--bcsp-text-body); font-variant-numeric: tabular-nums; line-height: var(--bcsp-lh-body); }

/* ---- Sync pill (spec 4.9) ----
   Only states a student can act on are drawn at all (FAILED / STALE / RETRYING,
   and RECOVERED for a moment); a routine save stays silent. It is anchored to
   the bottom LEFT, clear of the sticky rail footer's Search button above it and
   never wide enough to reach the results column. */
.local-personal-sync { position: fixed; z-index: var(--bcsp-z-sync); left: var(--bcsp-space-3); bottom: calc(6rem + env(safe-area-inset-bottom)); display: flex; flex-wrap: wrap; align-items: center; gap: var(--bcsp-space-1); max-width: min(22rem, calc(100vw - 2rem)); padding: var(--bcsp-space-1) var(--bcsp-space-1) var(--bcsp-space-1) 0.875rem; border-radius: var(--bcsp-radius-pill); color: var(--bcsp-ink-inverse); background: var(--bcsp-surface-inverse); box-shadow: var(--bcsp-elev-2); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-data); font-weight: 400; letter-spacing: 0; line-height: var(--bcsp-lh-data); }
.local-personal-sync:empty, .local-personal-sync:not([data-attention]) { display: none; }
.local-personal-sync[data-error='true'] { color: var(--bcsp-accent-ink); background: var(--bcsp-danger); }
.local-personal-sync .bcsp-action { color: inherit; border-color: currentColor; border-radius: var(--bcsp-radius-pill); background: transparent; }

/* ---- Locale: 13px floor for meta and badges under zh-CN ---- */
.bcsp-shell[data-bcsp-locale='zh-CN'] :is(.local-personal__meta, .local-personal__status, .local-personal__facts dt) { font-size: 0.8125rem; line-height: 1.25rem; }
.bcsp-shell[data-bcsp-locale='zh-CN'] .local-personal__badge { font-size: 0.8125rem; }

@media (hover: hover) and (pointer: fine) {
  .local-personal input:hover:not(:disabled):not(:focus-visible), .local-personal select:hover:not(:disabled):not(:focus-visible) { border-color: var(--bcsp-ink-muted); }
  .local-personal-sync .bcsp-action:hover:not(:disabled) { color: inherit; border-color: currentColor; background: var(--bcsp-scrim); }
}
@media (max-width: 47.999rem) {
  .local-personal__section { padding: var(--bcsp-space-3); }
  .local-personal__settings { grid-template-columns: minmax(0, 1fr); }
  .local-personal__settings > .local-personal__field:last-child:nth-child(odd) { grid-column: auto; }
}
@media (max-width: 31.999rem) {
  .local-personal__form { grid-template-columns: minmax(0, 1fr); }
  .local-personal__facts { grid-template-columns: minmax(0, 1fr); }
  .local-personal__facts > div:last-child:nth-child(odd) { grid-column: auto; }
  .local-personal-sync { left: var(--bcsp-space-2); right: var(--bcsp-space-2); bottom: calc(6rem + env(safe-area-inset-bottom)); max-width: none; }
}
@media (prefers-reduced-motion: reduce) { .local-personal *, .local-personal *::before, .local-personal *::after { scroll-behavior: auto !important; transition-duration: 0.01ms !important; } }
@media (forced-colors: active) {
  .local-personal__badge, .local-personal__card, .local-personal__notice[role='alert'], .local-personal__notice[role='group'], .local-personal-sync { border: 1px solid CanvasText; }
}
`;

export function LocalPersonalStyles() {
  return <style data-bcsp-local-personal="">{LOCAL_PERSONAL_CSS}</style>;
}
