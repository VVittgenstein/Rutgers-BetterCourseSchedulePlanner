export const SEARCH_CONTROL_POLISH_CSS = String.raw`
.bcsp-search-workspace__header {
  position: relative;
  gap: var(--bcsp-space-3);
  padding: var(--bcsp-space-4);
  background: var(--bcsp-paper-raised);
}
.bcsp-search-workspace__titleline {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--bcsp-space-3);
  align-items: end;
}
.bcsp-search-workspace__title-index {
  color: var(--bcsp-accent);
  font-family: var(--bcsp-font-display);
  font-size: clamp(2.8rem, 8cqi, 4.75rem);
  line-height: .72;
}
.bcsp-search-workspace__titleline > div { display: grid; gap: .18rem; }
.bcsp-search-workspace__title-kicker {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: .66rem;
  font-weight: 800;
  letter-spacing: .08em;
  text-transform: uppercase;
}
.bcsp-search-workspace__header h3 {
  margin: 0;
  font-family: var(--bcsp-font-display);
  font-size: clamp(1.7rem, 4.8cqi, 2.75rem);
  letter-spacing: -.045em;
  line-height: .92;
}
.bcsp-search-workspace__header > p {
  max-width: 43rem;
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: .82rem;
  line-height: 1.55;
}
.bcsp-search-workspace__scope {
  padding: var(--bcsp-space-3);
  border-bottom: 4px solid var(--bcsp-ink);
  background: var(--bcsp-paper);
}
.bcsp-search-workspace__scope > [role='alert'] { margin-bottom: var(--bcsp-space-2); }

.query-scope { gap: 0; border-top: 4px solid var(--bcsp-ink); }
.query-scope__matrix { background: var(--bcsp-paper-raised); }
.query-scope__cell {
  position: relative;
  min-height: 5rem;
  padding: .78rem var(--bcsp-space-3);
}
.query-scope__term::before {
  position: absolute;
  inset: 0 auto 0 0;
  width: 4px;
  background: transparent;
  content: '';
}
.query-scope__term[data-selected='true']::before { background: var(--bcsp-accent); }
.query-scope__term[data-selected='true'] { background: color-mix(in srgb, var(--bcsp-accent) 4%, var(--bcsp-paper-raised)); }
.query-scope__term[data-readiness='none'] { background: var(--bcsp-paper); }
.query-scope__option { gap: .7rem; }
.query-scope__option-copy strong { font-size: .9rem; line-height: 1.15; }
.query-scope__term-meta,
.query-scope__campus small {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: .65rem;
  line-height: 1.35;
}
.query-scope__campus-copy { display: grid; min-width: 0; gap: .08rem; }
.query-scope__campus-copy strong {
  display: inline-grid;
  width: max-content;
  min-width: 2.2rem;
  padding: .12rem .28rem;
  border: 1px solid var(--bcsp-line);
  place-items: center;
  font-family: var(--bcsp-font-data);
  font-size: .68rem;
  letter-spacing: .08em;
}
.query-scope__campus-copy small {
  overflow: hidden;
  padding: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.query-scope__action,
.query-scope__search { gap: .45rem; }
.query-scope__action .bcsp-action,
.query-scope__search .bcsp-action { min-height: 2.8rem; }
.query-scope__status {
  color: var(--bcsp-ink-muted);
  font-size: .68rem;
  line-height: 1.35;
  overflow-wrap: anywhere;
}

.filter-panel {
  border-top: 4px solid var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
}
.filter-panel__matrix-head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: .35rem var(--bcsp-space-3);
  padding: var(--bcsp-space-3) var(--bcsp-space-4);
  border-bottom: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}
.filter-panel__matrix-head p,
.filter-panel__matrix-head h3,
.filter-panel__matrix-head span { margin: 0; }
.filter-panel__matrix-head p {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: .66rem;
  font-weight: 800;
  letter-spacing: .08em;
  text-transform: uppercase;
}
.filter-panel__matrix-head h3 {
  grid-column: 1;
  font-family: var(--bcsp-font-display);
  font-size: clamp(1.4rem, 3.5cqi, 2.25rem);
  letter-spacing: -.045em;
  line-height: .94;
}
.filter-panel__matrix-head span {
  grid-column: 2;
  grid-row: 1 / span 2;
  max-width: 18rem;
  align-self: end;
  color: var(--bcsp-ink-muted);
  font-size: .72rem;
  line-height: 1.45;
}
.filter-panel__active {
  grid-template-columns: minmax(6.75rem, auto) minmax(0, 1fr) auto;
  padding: .75rem var(--bcsp-space-3);
  background: var(--bcsp-ink);
  color: var(--bcsp-paper-raised);
}
.filter-panel__active-title { color: var(--bcsp-paper-raised); }
.filter-panel__active .filter-panel__empty { color: color-mix(in srgb, var(--bcsp-paper) 68%, transparent); }
.filter-panel__active > .bcsp-action {
  border-color: color-mix(in srgb, var(--bcsp-paper) 42%, transparent);
  color: var(--bcsp-paper-raised);
}
.filter-panel__active .filter-panel__chip {
  border-color: color-mix(in srgb, var(--bcsp-paper) 48%, transparent);
  color: var(--bcsp-paper-raised);
  background: transparent;
}
.filter-panel__active .filter-panel__chip-pin { color: var(--bcsp-paper-raised); }
.filter-panel__row,
.filter-panel__row:nth-child(odd) { background: var(--bcsp-paper-raised); }
.filter-panel__legend {
  width: calc(100% - (2 * var(--bcsp-space-3)));
  padding: 0 0 .55rem;
  border-bottom: 1px solid var(--bcsp-line-soft);
  background: var(--bcsp-paper-raised);
  color: var(--bcsp-ink);
}
.filter-panel__ordinal { color: var(--bcsp-ink-muted); }
.filter-panel__label { font-size: .9rem; line-height: 1.15; }
.filter-panel__scope { font-size: .6rem; }
.filter-panel__control { padding-top: .65rem; }
.filter-panel__checks { grid-template-columns: repeat(auto-fit, minmax(8rem, 1fr)); }
.filter-panel__check { font-size: .7rem; }
.filter-panel__subject-list,
.filter-panel__dictionary-options { max-height: min(16rem, 42vh); }
.filter-panel__footer {
  padding: var(--bcsp-space-3);
  border-top: 4px solid var(--bcsp-ink);
  background: var(--bcsp-paper);
}
.filter-panel__footer-note { font-family: var(--bcsp-font-data); font-size: .68rem; }

@media (hover: hover) and (pointer: fine) {
  .query-scope__term:hover,
  .query-scope__campus:hover:not([data-ready='false']) { background: color-mix(in srgb, var(--bcsp-accent) 6%, var(--bcsp-paper-raised)); }
  .filter-panel__row:hover { background: color-mix(in srgb, var(--bcsp-accent) 3%, var(--bcsp-paper-raised)); }
  .filter-panel__active > .bcsp-action:hover:not(:disabled) { color: var(--bcsp-accent-ink); background: var(--bcsp-accent); }
}

@container (max-width: 42rem) {
  .bcsp-search-workspace__title-index { font-size: clamp(2.6rem, 11cqi, 4.5rem); }
  .filter-panel__matrix-head { grid-template-columns: minmax(0, 1fr); }
  .filter-panel__matrix-head span { grid-column: 1; grid-row: auto; max-width: none; }
  .filter-panel__active { grid-template-columns: 1fr; }
}

@media (max-width: 27rem) {
  .bcsp-search-workspace__header,
  .bcsp-search-workspace__scope { padding-inline: var(--bcsp-space-2); }
  .bcsp-search-workspace__titleline { grid-template-columns: 3rem minmax(0, 1fr); }
}
`;

export function SearchControlPolishStyles() {
  return <style data-bcsp-search-control-polish="">{SEARCH_CONTROL_POLISH_CSS}</style>;
}
