const LOCAL_PERSONAL_CSS = String.raw`
.local-personal { display: grid; gap: var(--bcsp-space-5); margin-top: var(--bcsp-space-5); }
.local-personal__hero { display: grid; grid-template-columns: minmax(0, 1fr) minmax(12rem, 0.32fr); gap: var(--bcsp-space-4); align-items: end; padding-bottom: var(--bcsp-space-4); border-bottom: 1px solid var(--bcsp-line); }
.local-personal__kicker, .local-personal__meta, .local-personal__status, .local-personal__label { margin: 0; font-family: var(--bcsp-font-data); font-size: 0.68rem; font-weight: 750; letter-spacing: 0.075em; line-height: 1.45; text-transform: uppercase; }
.local-personal__hero h3, .local-personal__section-title { margin: 0.25rem 0 0; font-size: clamp(1.6rem, 4vw, 3.5rem); font-weight: 850; letter-spacing: -0.05em; line-height: 0.95; text-transform: uppercase; }
.local-personal__hero p:last-child { max-width: 38rem; margin: 0; color: var(--bcsp-ink-muted); line-height: 1.55; }
.local-personal__section { border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.local-personal__section-head { display: flex; flex-wrap: wrap; align-items: end; justify-content: space-between; gap: var(--bcsp-space-3); padding: var(--bcsp-space-3); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); }
.local-personal__section-title { font-size: clamp(1.25rem, 2.6vw, 2.2rem); }
.local-personal__form, .local-personal__grid, .local-personal__list { display: grid; margin: 0; padding: 0; list-style: none; }
.local-personal__form { grid-template-columns: minmax(0, 1fr) auto; }
.local-personal__field { display: grid; gap: 0.45rem; min-width: 0; padding: var(--bcsp-space-3); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); }
.local-personal__field > span, .local-personal__field label { font-family: var(--bcsp-font-data); font-size: 0.68rem; font-weight: 750; letter-spacing: 0.06em; text-transform: uppercase; }
.local-personal input, .local-personal select { min-width: 0; min-height: 2.75rem; padding: 0.65rem 0.75rem; border: 1px solid var(--bcsp-line); border-radius: 0; color: var(--bcsp-ink); background: var(--bcsp-paper-raised); font: inherit; }
.local-personal input:focus-visible, .local-personal select:focus-visible { outline: 3px solid var(--bcsp-focus); outline-offset: 2px; }
.local-personal__form > .bcsp-action { min-width: 12rem; border-bottom: 1px solid var(--bcsp-line); }
.local-personal__list { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.local-personal__card { display: grid; align-content: start; gap: var(--bcsp-space-3); min-width: 0; padding: var(--bcsp-space-3); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); }
.local-personal__card h4 { margin: 0; overflow-wrap: anywhere; font-size: 1.15rem; line-height: 1.1; text-transform: uppercase; }
.local-personal__card p { margin: 0; color: var(--bcsp-ink-muted); line-height: 1.5; }
.local-personal__identity { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--bcsp-space-2); }
.local-personal__badge { display: inline-flex; width: fit-content; padding: 0.25rem 0.45rem; border: 1px solid var(--bcsp-line); font-family: var(--bcsp-font-data); font-size: 0.62rem; font-weight: 800; letter-spacing: 0.06em; text-transform: uppercase; }
.local-personal__badge[data-state='CLEAN'], .local-personal__badge[data-state='READY'] { color: var(--bcsp-paper-raised); background: var(--bcsp-ink); }
.local-personal__badge[data-state='MODIFIED'], .local-personal__badge[data-state='DANGER'] { color: var(--bcsp-accent-ink); border-color: var(--bcsp-accent); background: var(--bcsp-accent); }
.local-personal__actions { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2); align-items: center; }
.local-personal__notice { margin: 0; padding: var(--bcsp-space-3); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); color: var(--bcsp-ink-muted); line-height: 1.55; }
.local-personal__notice[role='alert'] { border-left: 0.35rem solid var(--bcsp-accent); color: var(--bcsp-ink); }
.local-personal__settings { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.local-personal__danger { border-color: var(--bcsp-accent); }
.local-personal__danger .local-personal__section-head { border-top: 0.4rem solid var(--bcsp-accent); }
.local-personal__reset-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); }
.local-personal__history { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.local-personal__facts { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin: 0; border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.local-personal__facts > div { padding: var(--bcsp-space-2); border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); }
.local-personal__facts dt { color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.62rem; letter-spacing: 0.06em; text-transform: uppercase; }
.local-personal__facts dd { margin: 0.25rem 0 0; overflow-wrap: anywhere; }
.local-personal-sync { position: fixed; z-index: 40; right: 0; bottom: 0; max-width: min(28rem, 100vw); padding: 0.7rem 0.9rem; border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); color: var(--bcsp-paper-raised); background: var(--bcsp-ink); font-family: var(--bcsp-font-data); font-size: 0.7rem; font-weight: 750; letter-spacing: 0.04em; }
.local-personal-sync:empty { display: none; }
.local-personal-sync[data-error='true'] { color: var(--bcsp-accent-ink); border-color: var(--bcsp-accent); background: var(--bcsp-accent); }
@media (max-width: 61.999rem) { .local-personal__reset-grid { grid-template-columns: 1fr; } }
@media (max-width: 47.999rem) { .local-personal__hero, .local-personal__settings, .local-personal__list, .local-personal__history { grid-template-columns: 1fr; } }
@media (max-width: 31.999rem) { .local-personal__form { grid-template-columns: 1fr; } .local-personal__form > .bcsp-action { min-width: 0; } .local-personal__facts { grid-template-columns: 1fr; } }
@media (prefers-reduced-motion: reduce) { .local-personal *, .local-personal *::before, .local-personal *::after { scroll-behavior: auto !important; transition-duration: 0.01ms !important; } }
`;

export function LocalPersonalStyles() {
  return <style data-bcsp-local-personal="">{LOCAL_PERSONAL_CSS}</style>;
}
