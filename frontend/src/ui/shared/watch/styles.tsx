export const WATCH_WORKSPACE_CSS = String.raw`
.watch-workspace { display: grid; gap: var(--bcsp-space-4); }
.watch-workspace__command-grid { display: grid; grid-template-columns: minmax(17rem, 0.72fr) minmax(0, 1.28fr); border: 1px solid var(--bcsp-line); background: var(--bcsp-paper-raised); }
.watch-workspace__panel { min-width: 0; padding: var(--bcsp-space-4); }
.watch-workspace__panel + .watch-workspace__panel { border-left: 1px solid var(--bcsp-line); }
.watch-workspace__kicker { margin: 0 0 var(--bcsp-space-1); color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.68rem; font-weight: 800; letter-spacing: 0.1em; text-transform: uppercase; }
.watch-workspace__title { margin: 0; font-size: clamp(1.65rem, 4vw, 3.2rem); font-weight: 850; letter-spacing: -0.055em; line-height: 0.92; text-transform: uppercase; }
.watch-workspace__lede { max-width: 60ch; margin: var(--bcsp-space-2) 0 0; color: var(--bcsp-ink-muted); }
.watch-workspace__status-strip { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); margin-top: var(--bcsp-space-4); border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.watch-workspace__status-strip > * { min-width: 0; padding: var(--bcsp-space-2); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); }
.watch-workspace__diagnostics { margin-top: var(--bcsp-space-2); border-top: 1px solid var(--bcsp-line-soft); color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.68rem; }
.watch-workspace__diagnostics summary { min-height: 2.25rem; padding: 0.7rem 0; cursor: pointer; font-weight: 800; letter-spacing: 0.06em; text-transform: uppercase; }
.watch-workspace__diagnostics summary:focus-visible { outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: 2px; }
.watch-workspace__diagnostic-facts { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); margin: 0 0 var(--bcsp-space-2); border-top: 1px solid var(--bcsp-line-soft); border-left: 1px solid var(--bcsp-line-soft); }
.watch-workspace__form { display: grid; gap: var(--bcsp-space-3); }
.watch-workspace__mode { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--bcsp-space-1); border: 0; padding: 0; margin: 0; }
.watch-workspace__mode legend { grid-column: 1 / -1; font-family: var(--bcsp-font-data); font-size: 0.7rem; font-weight: 800; letter-spacing: 0.08em; text-transform: uppercase; }
.watch-workspace__mode label { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: var(--bcsp-space-1); min-height: 2.75rem; padding: var(--bcsp-space-2); border: 1px solid var(--bcsp-line); cursor: pointer; transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1)); }
.watch-workspace__mode label:has(input:checked) { color: var(--bcsp-paper-raised); border-color: var(--bcsp-ink); background: var(--bcsp-ink); }
.watch-workspace__mode label:focus-within { outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: 2px; }
.watch-workspace__mode label:active:not(:has(input:focus-visible)) { transform: scale(0.98); }
.watch-workspace__field { display: grid; gap: 0.35rem; }
.watch-workspace__field label, .watch-workspace__field-title { font-family: var(--bcsp-font-data); font-size: 0.7rem; font-weight: 800; letter-spacing: 0.08em; text-transform: uppercase; }
.watch-workspace__input, .watch-workspace__select { width: 100%; min-height: 2.75rem; padding: 0.55rem 0.7rem; border: 1px solid var(--bcsp-line); border-radius: 0; color: var(--bcsp-ink); background: var(--bcsp-paper); font-family: var(--bcsp-font-data); }
.watch-workspace__input:focus-visible, .watch-workspace__select:focus-visible { outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: 2px; }
.watch-workspace__confirm { display: grid; min-height: 2.75rem; grid-template-columns: auto minmax(0, 1fr); gap: var(--bcsp-space-2); align-items: start; padding: var(--bcsp-space-2); border-left: 4px solid var(--bcsp-accent); background: var(--bcsp-paper); }
.watch-workspace__actions { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-1); align-items: center; }
.watch-workspace__inline-status { margin: 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.72rem; }
.watch-workspace__section-head { display: flex; justify-content: space-between; gap: var(--bcsp-space-3); align-items: end; padding-bottom: var(--bcsp-space-2); border-bottom: 1px solid var(--bcsp-line); }
.watch-workspace__section-title { margin: 0; font-size: clamp(1.25rem, 2vw, 1.8rem); letter-spacing: -0.035em; text-transform: uppercase; }
.watch-workspace__count { color: var(--bcsp-accent); font-family: var(--bcsp-font-data); font-size: 1.5rem; font-weight: 800; }
.watch-workspace__list { display: grid; gap: 0; padding: 0; margin: var(--bcsp-space-3) 0 0; border-top: 1px solid var(--bcsp-line); list-style: none; }
.watch-workspace__item { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-3); align-items: center; padding: var(--bcsp-space-3) 0; border-bottom: 1px solid var(--bcsp-line-soft); }
.watch-workspace__identity { display: flex; flex-wrap: wrap; gap: 0.35rem; align-items: baseline; }
.watch-workspace__index { font-family: var(--bcsp-font-data); font-size: 1.2rem; font-weight: 850; }
.watch-workspace__meta { color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.7rem; overflow-wrap: anywhere; }
.watch-workspace__badge { display: inline-block; padding: 0.2rem 0.4rem; border: 1px solid var(--bcsp-line); font-family: var(--bcsp-font-data); font-size: 0.62rem; font-weight: 800; letter-spacing: 0.06em; text-transform: uppercase; }
.watch-workspace__badge[data-state='OPEN'], .watch-workspace__badge[data-state='READY'] { color: var(--bcsp-paper-raised); background: var(--bcsp-ink); }
.watch-workspace__badge[data-state='STALE'], .watch-workspace__badge[data-state='ERROR'], .watch-workspace__badge[data-state='BLOCKED'] { color: var(--bcsp-accent-ink); border-color: var(--bcsp-accent); background: var(--bcsp-accent); }
.watch-workspace__empty { margin: var(--bcsp-space-3) 0 0; padding: var(--bcsp-space-4); border: 1px dashed var(--bcsp-line-soft); color: var(--bcsp-ink-muted); }
.watch-workspace__alerts { border: 1px solid var(--bcsp-line); }
.watch-workspace__alert { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-3); padding: var(--bcsp-space-3); border-bottom: 1px solid var(--bcsp-line); background: var(--bcsp-paper-raised); }
.watch-workspace__alert:last-child { border-bottom: 0; }
.watch-workspace__alert h4 { margin: 0; font-size: 1rem; text-transform: uppercase; }
.watch-workspace__alert p { margin: 0.35rem 0 0; color: var(--bcsp-ink-muted); }
.watch-telemetry { display: grid; gap: var(--bcsp-space-3); }
.watch-telemetry__resources { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.watch-telemetry__resource { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: start; min-width: 0; padding: var(--bcsp-space-2); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); background: var(--bcsp-paper-raised); }
.watch-telemetry__resource[data-availability='LKG'], .watch-telemetry__resource[data-availability='ERROR_NO_DATA'] { border-left: 4px solid var(--bcsp-accent); }
.watch-telemetry__resource h4 { margin: 0; font-family: var(--bcsp-font-data); font-size: 0.75rem; letter-spacing: 0.04em; text-transform: uppercase; }
.watch-telemetry__resource p { margin: 0.35rem 0 0; }
.watch-telemetry__resource-actions { display: grid; min-width: 10rem; gap: var(--bcsp-space-1); }
.watch-telemetry__resource-actions .watch-workspace__diagnostics { margin-top: 0; }
.watch-telemetry__grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.watch-telemetry__batch { min-width: 0; border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); background: var(--bcsp-paper-raised); }
.watch-telemetry__batch-head { display: flex; justify-content: space-between; gap: var(--bcsp-space-2); align-items: start; padding: var(--bcsp-space-3); border-bottom: 1px solid var(--bcsp-line); }
.watch-telemetry__batch-head h4 { margin: 0; font-family: var(--bcsp-font-data); font-size: 0.9rem; overflow-wrap: anywhere; }
.watch-telemetry__facts { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin: 0; }
.watch-telemetry__fact { min-width: 0; padding: var(--bcsp-space-2); border-right: 1px solid var(--bcsp-line-soft); border-bottom: 1px solid var(--bcsp-line-soft); }
.watch-telemetry__fact dt { color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.62rem; letter-spacing: 0.06em; text-transform: uppercase; }
.watch-telemetry__fact dd { margin: 0.25rem 0 0; overflow-wrap: anywhere; font-family: var(--bcsp-font-data); font-size: 0.78rem; font-variant-numeric: tabular-nums; font-weight: 750; }
.watch-telemetry__sections { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.watch-telemetry__section { min-width: 0; padding: var(--bcsp-space-3); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); }
.watch-telemetry__section p { margin: 0.35rem 0 0; }
.watch-toast-region { position: fixed; z-index: 30; right: var(--bcsp-space-3); bottom: calc(var(--bcsp-space-3) + env(safe-area-inset-bottom)); display: grid; width: min(28rem, calc(100vw - 2rem)); gap: var(--bcsp-space-1); pointer-events: none; }
.watch-toast { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: start; padding: var(--bcsp-space-3); border: 1px solid var(--bcsp-line); color: var(--bcsp-ink); background: var(--bcsp-paper-raised); opacity: 1; transform: translateY(0); transition: transform 180ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1)), opacity 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1)); pointer-events: auto; }
@starting-style { .watch-toast { opacity: 0; transform: translateY(35%); } }
.watch-toast[data-state='exiting'], .watch-toast[data-exiting='true'] { opacity: 0; transform: translateY(35%); transition-duration: 120ms; }
.watch-toast[data-tone='ALERT'] { border-left: 6px solid var(--bcsp-accent); }
.watch-toast__title { margin: 0; font-size: 0.84rem; text-transform: uppercase; }
.watch-toast__detail { margin: 0.3rem 0 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.7rem; overflow-wrap: anywhere; }
.watch-toast__dismiss { min-width: 2.75rem; min-height: 2.75rem; border: 1px solid var(--bcsp-line); border-radius: 0; color: inherit; background: transparent; cursor: pointer; transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1)); }
.watch-toast__dismiss:focus-visible { outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: 2px; }
.watch-toast__dismiss:active:not(:focus-visible) { transform: scale(0.96); }
.watch-selection-action { min-height: 2.75rem; padding: 0.45rem 0.65rem; border: 1px solid var(--bcsp-line); border-radius: 0; color: var(--bcsp-ink); background: var(--bcsp-paper); font-family: var(--bcsp-font-data); font-size: 0.68rem; font-weight: 800; letter-spacing: 0.06em; text-transform: uppercase; cursor: pointer; transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1)); }
.watch-selection-control { display: grid; justify-items: stretch; gap: 0.35rem; }
.watch-selection-control__link { min-height: 2.25rem; padding: 0.45rem 0.65rem; border-bottom: 1px solid var(--bcsp-line); color: var(--bcsp-ink); font-family: var(--bcsp-font-data); font-size: 0.64rem; font-weight: 800; letter-spacing: 0.04em; line-height: 1.25; text-align: center; text-decoration: none; text-transform: uppercase; }
.watch-selection-control__link:focus-visible { outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: 2px; }
.watch-selection-action[aria-pressed='true'] { color: var(--bcsp-accent-ink); border-color: var(--bcsp-accent); background: var(--bcsp-accent); }
.watch-selection-action:focus-visible { outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: 2px; }
.watch-selection-action:active:not(:disabled):not(:focus-visible) { transform: scale(0.97); }
.watch-selection-action:disabled { color: var(--bcsp-ink-muted); border-color: var(--bcsp-line-soft); background: var(--bcsp-paper); cursor: not-allowed; opacity: 0.72; }
@media (hover: hover) and (pointer: fine) {
  .watch-workspace__mode label:hover { background: var(--bcsp-paper); }
  .watch-workspace__mode label:has(input:checked):hover { color: var(--bcsp-paper-raised); background: var(--bcsp-ink); }
  .watch-toast__dismiss:hover, .watch-selection-action:hover:not(:disabled) { color: var(--bcsp-paper-raised); background: var(--bcsp-ink); }
  .watch-selection-control__link:hover { color: var(--bcsp-accent); border-color: var(--bcsp-accent); }
}
@media (max-width: 63.999rem) {
  .watch-workspace__command-grid { grid-template-columns: 1fr; }
  .watch-workspace__panel + .watch-workspace__panel { border-top: 1px solid var(--bcsp-line); border-left: 0; }
  .watch-telemetry__sections { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
@media (max-width: 47.999rem) {
  .watch-workspace__panel { padding: var(--bcsp-space-3); }
  .watch-workspace__status-strip { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .watch-workspace__mode, .watch-workspace__diagnostic-facts, .watch-telemetry__resources, .watch-telemetry__grid, .watch-telemetry__sections { grid-template-columns: 1fr; }
  .watch-telemetry__resource { grid-template-columns: 1fr; }
  .watch-workspace__item, .watch-workspace__alert { grid-template-columns: 1fr; }
  .watch-workspace__actions { align-items: stretch; }
  .watch-workspace__actions > * { flex: 1 1 100%; }
  .watch-toast-region { right: var(--bcsp-space-2); bottom: calc(var(--bcsp-space-2) + env(safe-area-inset-bottom)); width: calc(100vw - 1.5rem); }
}
@media (max-width: 20.999rem) {
  .watch-workspace__status-strip { grid-template-columns: 1fr; }
}
@media (prefers-reduced-motion: reduce) {
  .watch-workspace__mode label, .watch-toast__dismiss, .watch-selection-action { transition: none; }
  .watch-workspace__mode label:active:not(:has(input:focus-visible)), .watch-toast__dismiss:active:not(:focus-visible), .watch-selection-action:active:not(:disabled):not(:focus-visible) { transform: none; }
  .watch-toast { transform: none; transition: opacity 120ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1)) !important; }
  @starting-style { .watch-toast { opacity: 0; transform: none; } }
  .watch-toast[data-state='exiting'], .watch-toast[data-exiting='true'] { opacity: 0; transform: none; }
}
`;

export function WatchWorkspaceStyles() {
  return <style data-bcsp-watch-workspace="">{WATCH_WORKSPACE_CSS}</style>;
}
