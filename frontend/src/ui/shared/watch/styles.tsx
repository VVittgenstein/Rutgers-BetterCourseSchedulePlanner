/*
 * Watch desk, readiness bar, toasts and the section-selection toggle
 * (design spec v2: sections 4.1, 4.7, 4.9, 4.10, 4.13, 5.3, 5.5, 11.1).
 *
 * Rules for this string: no literal colours (tokens only), weights 400/600
 * only, no uppercase or tracking, every option layout fills its row (flex
 * wrap with growing items or an explicit column count), and the only
 * animation is the 2s opacity pulse on a live dot, removed under
 * prefers-reduced-motion.
 */
export const WATCH_WORKSPACE_CSS = String.raw`
@keyframes bcsp-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}

@keyframes bcsp-telemetry-turn {
  to { transform: rotate(1turn); }
}

/* ---- Desk frame ---- */
.watch-workspace { display: grid; gap: var(--bcsp-space-4); }
.watch-workspace__command-grid { display: grid; grid-template-columns: minmax(18rem, 5fr) minmax(0, 7fr); gap: var(--bcsp-space-4); }
.watch-workspace__panel,
.watch-workspace > section:not(.watch-workspace__command-grid) { min-width: 0; padding: var(--bcsp-space-4); border: 1px solid var(--bcsp-line); border-radius: var(--bcsp-radius-3); background: var(--bcsp-paper-raised); }
.watch-workspace > section:not(.watch-workspace__command-grid) { display: grid; align-content: start; gap: var(--bcsp-space-3); }

/* ---- Type ---- */
.watch-workspace__kicker { margin: 0 0 0.125rem; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-meta); font-weight: 400; letter-spacing: 0; line-height: var(--bcsp-lh-meta); text-transform: none; }
.watch-workspace__title { margin: 0; font-size: var(--bcsp-text-title); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-title); text-transform: none; }
.watch-workspace__lede { max-width: 60ch; margin: var(--bcsp-space-1) 0 0; color: var(--bcsp-ink-muted); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); }
.watch-workspace__section-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--bcsp-space-2); min-height: var(--bcsp-control-h); margin: 0; padding: 0; border: 0; }
.watch-workspace__section-title { margin: 0; font-size: var(--bcsp-text-title); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-title); text-transform: none; }
.watch-workspace__count { display: inline-flex; align-items: center; height: 1.75rem; padding: 0 0.625rem; border-radius: var(--bcsp-radius-pill); color: var(--bcsp-ink-2); background: var(--bcsp-surface-2); font-family: var(--bcsp-font-sans); font-feature-settings: "tnum" 1; font-size: var(--bcsp-text-body); font-variant-numeric: tabular-nums; font-weight: 600; letter-spacing: 0; }
.watch-workspace__meta { margin: 0.25rem 0 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-meta); line-height: var(--bcsp-lh-meta); overflow-wrap: anywhere; }
.watch-workspace__meta[data-state='ATTENTION'] { color: var(--bcsp-danger); }
.watch-workspace__inline-status { margin: 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-meta); line-height: var(--bcsp-lh-meta); }
.watch-workspace__inline-status[role='alert'] { color: var(--bcsp-danger); }

/* ---- Stat tiles: 2 x 2, every row filled (spec 4.13, 11.1) ---- */
.watch-workspace__status-strip { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--bcsp-space-2); margin-top: var(--bcsp-space-4); }
.watch-workspace__status-strip > * { min-width: 0; }
.watch-workspace__status-strip .bcsp-status-signal[data-state='ready'] .bcsp-status-signal__mark { animation: bcsp-pulse 2s ease-in-out infinite; }

/* ---- Disclosures (spec 4.11) ---- */
.watch-workspace__diagnostics { margin-top: var(--bcsp-space-2); color: var(--bcsp-ink-2); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); }
.watch-workspace__diagnostics summary { position: relative; display: flex; align-items: center; min-height: var(--bcsp-control-h); padding: 0 2rem 0 0.5rem; border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink-2); font-weight: 400; letter-spacing: 0; list-style: none; text-transform: none; cursor: pointer; }
.watch-workspace__diagnostics summary::-webkit-details-marker { display: none; }
.watch-workspace__diagnostics summary::after { content: ''; position: absolute; top: calc(50% - 0.25rem); right: 0.875rem; width: 0.375rem; height: 0.375rem; border-right: 1.5px solid currentColor; border-bottom: 1.5px solid currentColor; transform: rotate(-45deg); transition: transform 160ms var(--bcsp-ease-out); }
.watch-workspace__diagnostics[open] summary::after { transform: rotate(45deg); }
.watch-workspace__diagnostic-facts { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2) var(--bcsp-space-4); margin: var(--bcsp-space-1) 0 0; padding: var(--bcsp-space-2); border-radius: 0.5rem; background: var(--bcsp-surface-2); }
.watch-workspace__diagnostic-facts > .watch-telemetry__fact { flex: 1 1 9rem; }
.watch-workspace__diagnostic-facts dd { font-family: var(--bcsp-font-mono); font-size: var(--bcsp-text-data); }

/* ---- Notification form (spec 4.2, 4.3) ---- */
.watch-workspace__form { display: grid; gap: var(--bcsp-space-3); }
.watch-workspace__mode { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-1); min-width: 0; margin: 0; padding: 0; border: 0; }
.watch-workspace__mode legend { padding: 0; margin-bottom: var(--bcsp-space-2); font-size: var(--bcsp-text-title); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-title); text-transform: none; }
.watch-workspace__mode label { display: grid; flex: 1 1 14rem; min-width: 0; grid-template-columns: auto minmax(0, 1fr); align-items: start; gap: 0.625rem; min-height: var(--bcsp-control-h); padding: var(--bcsp-space-2) 0.875rem; border: 1px solid var(--bcsp-line-strong); border-radius: var(--bcsp-radius-3); color: var(--bcsp-ink); background: var(--bcsp-paper-raised); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); cursor: pointer; transition: background-color var(--bcsp-dur-1) var(--bcsp-ease-out), border-color var(--bcsp-dur-1) var(--bcsp-ease-out); }
.watch-workspace__mode label input { margin-top: 0.1875rem; }
.watch-workspace__mode label:has(input:checked) { border-color: var(--bcsp-accent-text); background: var(--bcsp-accent-tint); }
.watch-workspace__mode label input:checked + * { font-weight: 600; }
.watch-workspace__mode label:has(input:focus-visible) { outline: 2px solid var(--bcsp-focus); outline-offset: 2px; }
.watch-workspace__field { display: grid; gap: var(--bcsp-space-1); }
.watch-workspace__field label, .watch-workspace__field-title { color: var(--bcsp-ink); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-transform: none; }
.watch-workspace__input, .watch-workspace__select { width: 100%; height: var(--bcsp-control-h); min-height: var(--bcsp-control-h); padding: 0 var(--bcsp-space-2); border: 1px solid var(--bcsp-line-strong); border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink); background: var(--bcsp-paper-raised); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); transition: border-color var(--bcsp-dur-1) var(--bcsp-ease-out); }
.watch-workspace__input[aria-invalid='true'] { border-color: var(--bcsp-danger); }
.watch-workspace__input:focus-visible, .watch-workspace__select:focus-visible { border-color: var(--bcsp-focus); outline: 2px solid var(--bcsp-focus); outline-offset: 0; }
.watch-workspace__input:disabled, .watch-workspace__select:disabled { color: var(--bcsp-ink-muted); background: var(--bcsp-surface-3); opacity: 1; }
.watch-workspace__select { appearance: none; padding-right: 2.25rem; }
.watch-workspace__select-control { position: relative; display: block; }
.watch-workspace__select-control::after { content: ''; position: absolute; top: calc(50% - 0.3125rem); right: 0.875rem; width: 0.375rem; height: 0.375rem; border-right: 1.5px solid currentColor; border-bottom: 1.5px solid currentColor; transform: rotate(45deg); pointer-events: none; }
.watch-workspace__field input[type='range'] { width: 100%; }
.watch-workspace__confirm { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: var(--bcsp-space-2); align-items: center; min-height: var(--bcsp-control-h); margin: 0; padding: 0.625rem var(--bcsp-space-2); border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink); background: var(--bcsp-surface-2); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); cursor: pointer; }
.watch-workspace__confirm input { margin: 0; }
.watch-workspace__confirm:has(input:focus-visible) { outline: 2px solid var(--bcsp-focus); outline-offset: 2px; }
.watch-workspace__confirm--warn { padding: var(--bcsp-space-2) var(--bcsp-space-3); border: 1px solid var(--bcsp-warn-line); border-radius: 0.5rem; background: var(--bcsp-warn-tint); }

/* ---- Inline banner (spec 4.9). A policy that is not confirmed yet is a warning,
   not a failure: loose red text is reserved for something that actually broke. ---- */
.watch-workspace__notice { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: var(--bcsp-space-2); align-items: center; min-height: var(--bcsp-control-h); margin: 0; padding: var(--bcsp-space-2) var(--bcsp-space-3); border: 1px solid var(--bcsp-warn-line); border-radius: 0.5rem; color: var(--bcsp-ink); background: var(--bcsp-warn-tint); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); }
.watch-workspace__notice::before { content: ''; width: 1rem; height: 1rem; border-radius: var(--bcsp-radius-pill); background: var(--bcsp-warn); }
.watch-workspace__notice--danger { border-color: var(--bcsp-danger-line); background: var(--bcsp-danger-tint); }
.watch-workspace__notice--danger::before { background: var(--bcsp-danger); }
.watch-workspace__notice--info { border-color: var(--bcsp-info-line); background: var(--bcsp-info-tint); }
.watch-workspace__notice--info::before { background: var(--bcsp-info); }
.watch-workspace__actions { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2); align-items: center; }
.watch-workspace__list ~ .watch-workspace__actions { min-height: 3rem; justify-content: flex-end; }

/* ---- Selected sections (spec 4.10) ---- */
.watch-workspace__list { display: grid; gap: 0; margin: 0; padding: 0; list-style: none; }
.watch-workspace__item { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-3); align-items: center; min-height: 4rem; padding: var(--bcsp-space-2) 0; border-bottom: 1px solid var(--bcsp-line-soft); }
.watch-workspace__item:last-child { border-bottom: 0; }
.watch-workspace__item:focus-within { outline: 2px solid var(--bcsp-focus); outline-offset: -2px; border-radius: var(--bcsp-radius-2); }
.watch-workspace__identity { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-1); align-items: center; }
.watch-workspace__index { font-family: var(--bcsp-font-mono); font-feature-settings: "tnum" 1; font-size: var(--bcsp-text-subtitle); font-variant-numeric: tabular-nums slashed-zero; font-weight: 600; line-height: var(--bcsp-lh-subtitle); }

/* ---- Badges: one tone map (spec 4.7) ---- */
.watch-workspace__badge, .watch-readiness__badge { display: inline-flex; align-items: center; gap: 0.375rem; height: 1.375rem; padding: 0 var(--bcsp-space-1); border: 1px solid var(--bcsp-line); border-radius: var(--bcsp-radius-pill); color: var(--bcsp-ink-2); background: var(--bcsp-surface-2); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-micro); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-micro); text-transform: none; white-space: nowrap; }
.watch-workspace__badge[data-state='OPEN'], .watch-workspace__badge[data-state='READY'], .watch-workspace__badge[data-state='WATCHING'], .watch-workspace__badge[data-state='FRESH'],
.watch-readiness__badge[data-level='READY'] { color: var(--bcsp-ok); border-color: var(--bcsp-ok-line); background: var(--bcsp-ok-tint); }
.watch-workspace__badge[data-state='STALE'], .watch-workspace__badge[data-state='UNKNOWN'], .watch-workspace__badge[data-state='PREPARING'], .watch-workspace__badge[data-state='STOPPING'], .watch-workspace__badge[data-state='ATTENTION'],
.watch-readiness__badge[data-level='DEGRADED'] { color: var(--bcsp-warn); border-color: var(--bcsp-warn-line); background: var(--bcsp-warn-tint); }
.watch-workspace__badge[data-state='ERROR'], .watch-workspace__badge[data-state='BLOCKED'], .watch-workspace__badge[data-state='OUT_OF_RANGE'], .watch-workspace__badge[data-state='ERROR_NO_DATA'] { color: var(--bcsp-danger); border-color: var(--bcsp-danger-line); background: var(--bcsp-danger-tint); }
.watch-workspace__item .watch-workspace__badge[data-state='READY']::before, .watch-workspace__badge[data-state='WATCHING']::before, .watch-readiness__badge[data-level='READY']::before { content: ''; width: 0.375rem; height: 0.375rem; border-radius: var(--bcsp-radius-pill); background: currentColor; animation: bcsp-pulse 2s ease-in-out infinite; }

/* ---- Empty rows and alert center ---- */
.watch-workspace__empty { display: flex; align-items: center; min-height: 2.5rem; margin: 0; padding: 0.5rem var(--bcsp-space-2); border: 1px dashed var(--bcsp-line-strong); border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink-muted); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); }
.watch-workspace__empty[role='alert'] { gap: var(--bcsp-space-2); border: 1px solid var(--bcsp-danger-line); border-radius: 0.5rem; color: var(--bcsp-ink); background: var(--bcsp-danger-tint); }
.watch-workspace__empty[role='alert']::before { content: ''; flex: none; width: 1rem; height: 1rem; border-radius: var(--bcsp-radius-pill); background: var(--bcsp-danger); }
.watch-workspace__alerts { display: grid; gap: var(--bcsp-space-2); }
.watch-workspace__alert { position: relative; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-3); align-items: center; padding: var(--bcsp-space-3) var(--bcsp-space-3) var(--bcsp-space-3) 2.25rem; border: 1px solid var(--bcsp-ok-line); border-radius: 0.5rem; background: var(--bcsp-ok-tint); }
.watch-workspace__alert::before { content: ''; position: absolute; top: 1.4375rem; left: var(--bcsp-space-3); width: 0.375rem; height: 0.375rem; border-radius: var(--bcsp-radius-pill); background: var(--bcsp-ok); }
.watch-workspace__alert h4 { margin: 0; font-size: var(--bcsp-text-subtitle); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-subtitle); text-transform: none; }
.watch-workspace__alert p { margin: 0.25rem 0 0; color: var(--bcsp-ink); font-size: var(--bcsp-text-body); line-height: var(--bcsp-lh-body); }
.watch-workspace__alert p.watch-workspace__meta { color: var(--bcsp-ink-muted); font-size: var(--bcsp-text-meta); line-height: var(--bcsp-lh-meta); }
.watch-workspace__alert[data-alert-visibility='DISMISSED'] { border-color: var(--bcsp-line); background: var(--bcsp-surface-2); opacity: 0.8; }
.watch-workspace__alert[data-alert-visibility='DISMISSED']::before { background: var(--bcsp-ink-muted); }

/* ---- Telemetry (spec 5.5, 4.10) ---- */
.watch-telemetry { gap: var(--bcsp-space-3); }
.watch-telemetry__resources { display: grid; gap: 0; }
.watch-telemetry__resource { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: center; min-width: 0; min-height: var(--bcsp-control-h); padding: 0.625rem 0; border-bottom: 1px solid var(--bcsp-line-soft); }
.watch-telemetry__resource:last-child { border-bottom: 0; }
/* Tone map (spec 4.7): ok = current, warn = stale / last-known-good / never observed,
   danger = a real error, busy = a read in flight. A card is tinted only when the tone
   is warn or danger; "reading the current state" is neither, so it keeps the quiet row
   and says so with a spinner instead of a red surface. */
.watch-telemetry__resource[data-tone='warn'] { padding: 0.625rem var(--bcsp-space-2); border: 1px solid var(--bcsp-warn-line); border-radius: 0.5rem; background: var(--bcsp-warn-tint); }
.watch-telemetry__resource[data-tone='danger'] { padding: 0.625rem var(--bcsp-space-2); border: 1px solid var(--bcsp-danger-line); border-radius: 0.5rem; background: var(--bcsp-danger-tint); }
.watch-telemetry__resource + .watch-telemetry__resource[data-tone='warn'], .watch-telemetry__resource + .watch-telemetry__resource[data-tone='danger'] { margin-top: var(--bcsp-space-1); }
.watch-telemetry__resource h4 { margin: 0; font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-transform: none; }
.watch-telemetry__resource p { display: flex; align-items: center; gap: 0.4375rem; margin: 0.125rem 0 0; }
.watch-telemetry__resource p::before { content: ''; flex: none; width: 0.5rem; height: 0.5rem; border-radius: var(--bcsp-radius-pill); background: var(--bcsp-ink-muted); }
.watch-telemetry__resource[data-tone='ok'] p::before { background: var(--bcsp-ok); }
.watch-telemetry__resource[data-tone='warn'] p::before { background: var(--bcsp-warn); }
.watch-telemetry__resource[data-tone='danger'] p::before { background: var(--bcsp-danger); }
.watch-telemetry__resource[data-tone='busy'] p { color: var(--bcsp-ink-muted); }
.watch-telemetry__resource[data-tone='busy'] p::before { width: 0.75rem; height: 0.75rem; border: 2px solid var(--bcsp-line-strong); border-top-color: transparent; background: transparent; animation: bcsp-telemetry-turn 700ms linear infinite; }
.watch-telemetry__resource-actions { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-1); align-items: center; }
.watch-telemetry__resource-actions .watch-workspace__diagnostics { margin-top: 0; }
.watch-telemetry__resource-actions .bcsp-action { padding: 0 var(--bcsp-space-2); font-size: var(--bcsp-text-data); }
.watch-telemetry__grid { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-3); }
.watch-telemetry__batch { flex: 1 1 22rem; min-width: 0; padding: var(--bcsp-space-3); border-radius: 0.5rem; background: var(--bcsp-surface-2); }
.watch-telemetry__batch[data-freshness='STALE'], .watch-telemetry__batch[data-freshness='UNKNOWN'] { background: var(--bcsp-warn-tint); }
.watch-telemetry__batch-head { display: flex; justify-content: space-between; gap: var(--bcsp-space-2); align-items: center; min-height: 2.5rem; margin-bottom: var(--bcsp-space-2); }
.watch-telemetry__batch-head h4 { margin: 0; font-family: var(--bcsp-font-mono); font-size: var(--bcsp-text-data); font-weight: 600; letter-spacing: 0; line-height: var(--bcsp-lh-data); overflow-wrap: anywhere; }
.watch-telemetry__facts { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2) var(--bcsp-space-4); margin: 0; }
.watch-telemetry__fact { flex: 1 1 10rem; min-width: 0; }
.watch-telemetry__fact dt { color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-meta); letter-spacing: 0; line-height: var(--bcsp-lh-meta); text-transform: none; }
.watch-telemetry__fact dd { margin: 0.125rem 0 0; overflow-wrap: anywhere; color: var(--bcsp-ink); font-family: var(--bcsp-font-sans); font-feature-settings: "tnum" 1; font-size: var(--bcsp-text-body); font-variant-numeric: tabular-nums; font-weight: 400; line-height: var(--bcsp-lh-body); }
.watch-telemetry__sections { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2); }
.watch-telemetry__section { flex: 1 1 14rem; min-width: 0; padding: var(--bcsp-space-3); border-radius: 0.5rem; background: var(--bcsp-surface-2); }
.watch-telemetry__section[data-freshness='STALE'], .watch-telemetry__section[data-freshness='UNKNOWN'] { background: var(--bcsp-warn-tint); }
.watch-telemetry__section p { margin: 0.25rem 0 0; }

/* ---- Readiness bar: sticky under the app bar (spec 4.9, 5.3) ---- */
.watch-readiness { position: sticky; top: var(--bcsp-navigation-height, 3.5rem); z-index: var(--bcsp-z-readiness); display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2); align-items: center; justify-content: space-between; min-height: 3rem; margin: var(--bcsp-space-1) 0 var(--bcsp-space-3); padding: var(--bcsp-space-1) var(--bcsp-space-3); border: 1px solid var(--bcsp-ok-line); border-radius: 0.5rem; color: var(--bcsp-ink); background: var(--bcsp-ok-tint); }
.watch-readiness[data-bcsp-watch-readiness='DEGRADED'] { border-color: var(--bcsp-warn-line); background: var(--bcsp-warn-tint); }
.watch-readiness[data-bcsp-watch-readiness='DEGRADED'][data-broken-ring='CONNECTION'] { border-color: var(--bcsp-danger-line); background: var(--bcsp-danger-tint); }
.watch-readiness[data-broken-ring='CONNECTION'] .watch-readiness__badge { color: var(--bcsp-danger); border-color: var(--bcsp-danger-line); background: var(--bcsp-danger-tint); }
.watch-readiness__summary { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2); align-items: center; min-width: 0; margin: 0; }
.watch-readiness__reason { color: var(--bcsp-ink); font-size: var(--bcsp-text-data); line-height: var(--bcsp-lh-data); overflow-wrap: anywhere; }
.watch-readiness__action { display: inline-flex; align-items: center; justify-content: center; gap: var(--bcsp-space-1); min-height: var(--bcsp-control-h); padding: 0 var(--bcsp-space-3); border: 1px solid var(--bcsp-line-strong); border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink); background: var(--bcsp-paper-raised); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-body); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-transform: none; white-space: nowrap; cursor: pointer; transition: background-color var(--bcsp-dur-1) var(--bcsp-ease-out), border-color var(--bcsp-dur-1) var(--bcsp-ease-out), transform var(--bcsp-dur-1) var(--bcsp-ease-out); }
.watch-readiness__action:active:not(:focus-visible) { background: var(--bcsp-surface-3); transform: translateY(1px); }

/* ---- Toasts (spec 4.9) ---- */
.watch-toast-region { position: fixed; z-index: var(--bcsp-z-toast); right: var(--bcsp-space-3); bottom: calc(var(--bcsp-space-3) + env(safe-area-inset-bottom)); display: grid; width: min(24rem, calc(100vw - 2rem)); gap: var(--bcsp-space-1); pointer-events: none; }
.watch-toast { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: start; padding: var(--bcsp-space-2) var(--bcsp-space-2) var(--bcsp-space-2) var(--bcsp-space-3); border: 1px solid var(--bcsp-line); border-radius: var(--bcsp-radius-3); color: var(--bcsp-ink); background: var(--bcsp-paper-raised); box-shadow: var(--bcsp-elev-2); opacity: 1; transform: translateY(0); transition: transform var(--bcsp-dur-2) var(--bcsp-ease-out), opacity var(--bcsp-dur-2) var(--bcsp-ease-out); pointer-events: auto; }
@starting-style { .watch-toast { opacity: 0; transform: translateY(0.5rem); } }
.watch-toast[data-state='EXITING'], .watch-toast[data-state='exiting'], .watch-toast[data-exiting='true'] { opacity: 0; transform: translateY(0.5rem); transition-duration: var(--bcsp-dur-1); }
.watch-toast[data-tone='ALERT'] { border-left: 3px solid var(--bcsp-accent); }
.watch-toast > div { padding-top: 0.625rem; }
.watch-toast__title { margin: 0; font-size: var(--bcsp-text-body); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-transform: none; }
.watch-toast__detail { margin: 0.25rem 0 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-meta); line-height: var(--bcsp-lh-meta); overflow-wrap: anywhere; }
.watch-toast__dismiss { display: inline-flex; align-items: center; justify-content: center; width: var(--bcsp-control-h); height: var(--bcsp-control-h); padding: 0; border: 0; border-radius: var(--bcsp-radius-pill); color: var(--bcsp-ink-2); background: transparent; font-size: 1.25rem; line-height: 1; cursor: pointer; transition: background-color var(--bcsp-dur-1) var(--bcsp-ease-out), color var(--bcsp-dur-1) var(--bcsp-ease-out); }

/* ---- Section selection toggle in result rows (spec 4.1) ---- */
.watch-selection-action { position: relative; display: inline-flex; align-items: center; justify-content: center; gap: var(--bcsp-space-1); min-height: var(--bcsp-control-h); padding: 0 var(--bcsp-space-2); border: 1px solid var(--bcsp-line-strong); border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink); background: var(--bcsp-paper-raised); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-data); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-transform: none; white-space: nowrap; cursor: pointer; transition: background-color var(--bcsp-dur-1) var(--bcsp-ease-out), border-color var(--bcsp-dur-1) var(--bcsp-ease-out), color var(--bcsp-dur-1) var(--bcsp-ease-out), transform var(--bcsp-dur-1) var(--bcsp-ease-out); }
.watch-selection-action[aria-pressed='true'] { color: var(--bcsp-accent-text); border-color: var(--bcsp-accent-tint-strong); background: var(--bcsp-accent-tint); }
.watch-selection-action[aria-pressed='true']::before { content: ''; width: 0.3125rem; height: 0.5625rem; margin: 0 0.125rem 0.125rem 0; border-right: 2px solid currentColor; border-bottom: 2px solid currentColor; transform: rotate(45deg); }
.watch-selection-action:active:not(:disabled):not(:focus-visible) { transform: translateY(1px); }
.watch-selection-action:disabled { color: var(--bcsp-ink-muted); border-color: var(--bcsp-line); background: var(--bcsp-surface-3); cursor: not-allowed; opacity: 1; }
.watch-selection-control { display: grid; justify-items: stretch; gap: 0.375rem; }
.watch-selection-control__link { display: inline-flex; align-items: center; justify-content: center; min-height: var(--bcsp-control-h); padding: 0 var(--bcsp-space-2); border-radius: var(--bcsp-radius-2); color: var(--bcsp-ink-2); font-family: var(--bcsp-font-sans); font-size: var(--bcsp-text-data); font-weight: 600; letter-spacing: 0; line-height: 1.25rem; text-align: center; text-decoration: none; text-transform: none; transition: background-color var(--bcsp-dur-1) var(--bcsp-ease-out), color var(--bcsp-dur-1) var(--bcsp-ease-out); }

/* ---- Locale: 13px floor for meta and badges under zh-CN ---- */
.bcsp-shell[data-bcsp-locale='zh-CN'] :is(.watch-workspace__meta, .watch-workspace__inline-status, .watch-workspace__kicker, .watch-telemetry__fact dt, .watch-toast__detail) { font-size: 0.8125rem; line-height: 1.25rem; }
.bcsp-shell[data-bcsp-locale='zh-CN'] :is(.watch-workspace__badge, .watch-readiness__badge) { font-size: 0.8125rem; }

@media (hover: hover) and (pointer: fine) {
  .watch-workspace__mode label:hover { background: var(--bcsp-surface-2); }
  .watch-workspace__mode label:has(input:checked):hover { background: var(--bcsp-accent-tint-strong); }
  .watch-workspace__diagnostics summary:hover { background: var(--bcsp-surface-2); }
  .watch-workspace__input:hover:not(:disabled):not(:focus-visible), .watch-workspace__select:hover:not(:disabled):not(:focus-visible) { border-color: var(--bcsp-ink-muted); }
  .watch-readiness__action:hover { border-color: var(--bcsp-ink-muted); background: var(--bcsp-surface-2); }
  .watch-toast__dismiss:hover { color: var(--bcsp-ink); background: var(--bcsp-surface-2); }
  .watch-selection-action:hover:not(:disabled):not([aria-pressed='true']) { border-color: var(--bcsp-ink-muted); background: var(--bcsp-surface-2); }
  .watch-selection-action[aria-pressed='true']:hover:not(:disabled) { background: var(--bcsp-accent-tint-strong); }
  .watch-selection-control__link:hover { color: var(--bcsp-ink); background: var(--bcsp-surface-2); }
}
@media (max-width: 63.999rem) {
  .watch-workspace__command-grid { grid-template-columns: minmax(0, 1fr); }
}
@media (max-width: 47.999rem) {
  .watch-workspace__panel, .watch-workspace > section:not(.watch-workspace__command-grid) { padding: var(--bcsp-space-3); }
  .watch-readiness { flex-direction: column; align-items: stretch; }
  .watch-telemetry__resource, .watch-workspace__item, .watch-workspace__alert { grid-template-columns: minmax(0, 1fr); }
  .watch-workspace__actions { align-items: stretch; }
  .watch-workspace__actions > * { flex: 1 1 100%; }
  .watch-toast-region { right: var(--bcsp-space-2); bottom: calc(var(--bcsp-space-2) + env(safe-area-inset-bottom)); width: calc(100vw - 1.5rem); }
}
@media (max-width: 20.999rem) {
  .watch-workspace__status-strip { grid-template-columns: minmax(0, 1fr); }
}
@media (prefers-reduced-motion: reduce) {
  .watch-workspace__mode label, .watch-toast__dismiss, .watch-selection-action, .watch-selection-control__link, .watch-readiness__action, .watch-workspace__diagnostics summary::after { transition: none; }
  .watch-workspace__badge::before, .watch-readiness__badge::before, .watch-workspace__status-strip .bcsp-status-signal__mark, .watch-telemetry__resource[data-tone='busy'] p::before { animation: none; }
  .watch-selection-action:active:not(:disabled):not(:focus-visible), .watch-readiness__action:active:not(:focus-visible) { transform: none; }
  .watch-toast { transform: none; transition: opacity var(--bcsp-dur-1) var(--bcsp-ease-out) !important; }
  @starting-style { .watch-toast { opacity: 0; transform: none; } }
  .watch-toast[data-state='EXITING'], .watch-toast[data-state='exiting'], .watch-toast[data-exiting='true'] { opacity: 0; transform: none; }
}
@media (forced-colors: active) {
  .watch-readiness, .watch-workspace__alert, .watch-workspace__confirm, .watch-workspace__notice, .watch-toast, .watch-workspace__mode label { border: 1px solid CanvasText; }
  .watch-selection-action, .watch-readiness__action, .watch-toast__dismiss { border: 1px solid ButtonText; }
}
`;

export function WatchWorkspaceStyles() {
  return <style data-bcsp-watch-workspace="">{WATCH_WORKSPACE_CSS}</style>;
}
