export const SEARCH_WORKSPACE_CSS = String.raw`
/* Course search workspace (spec v2 section 5.4): a sticky rail card at the
   left, results at the right. The rail width literal below is pinned by
   tests/course-workspace.test.tsx. */
.bcsp-search-workspace {
  --bcsp-rail-width: clamp(26rem, 28vw, 34rem);
  display: grid;
  grid-template-columns: var(--bcsp-rail-width) minmax(0, 1fr);
  column-gap: var(--bcsp-space-4);
  align-items: start;
  margin-top: var(--bcsp-space-1);
}

.bcsp-search-workspace__filters,
.bcsp-search-workspace__results {
  min-width: 0;
}

.bcsp-search-workspace[data-detail-route='true'] {
  grid-template-columns: minmax(0, 1fr);
}

.bcsp-search-workspace[data-detail-route='true'] .bcsp-search-workspace__results {
  width: 100%;
  max-width: var(--bcsp-detail-max);
  margin: 0 auto;
}

/* ---- Rail: a raised card that scrolls on its own under the app bar ---- */
.bcsp-search-workspace__filters {
  container-type: inline-size;
  position: sticky;
  top: calc(var(--bcsp-navigation-height, 3.5rem) + var(--bcsp-readiness-height, 0px) + 1rem);
  align-self: start;
  display: flex;
  flex-direction: column;
  max-height: calc(100vh - var(--bcsp-navigation-height, 3.5rem) - var(--bcsp-readiness-height, 0px) - 2rem);
  max-height: calc(100dvh - var(--bcsp-navigation-height, 3.5rem) - var(--bcsp-readiness-height, 0px) - 2rem);
  overflow: auto;
  overscroll-behavior: contain;
  touch-action: pan-y;
  border: 1px solid var(--bcsp-line);
  border-radius: var(--bcsp-radius-3);
  background: var(--bcsp-paper-raised);
  box-shadow: var(--bcsp-elev-1);
  scrollbar-width: none;
  scrollbar-gutter: auto;
}

.bcsp-search-workspace__filters > * { flex: none; }

/* Scrollbar hidden until needed (spec 11.2): no reserved gutter, thin overlay on hover / focus-within. */
.bcsp-search-workspace__filters::-webkit-scrollbar {
  width: 0;
  height: 0;
}

.bcsp-search-workspace__filters:hover,
.bcsp-search-workspace__filters:focus-within {
  scrollbar-width: thin;
  scrollbar-color: var(--bcsp-line-strong) transparent;
}

.bcsp-search-workspace__filters:hover::-webkit-scrollbar,
.bcsp-search-workspace__filters:focus-within::-webkit-scrollbar { width: 8px; }

.bcsp-search-workspace__filters::-webkit-scrollbar-thumb {
  border: 2px solid transparent;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-line-strong);
  background-clip: padding-box;
}

.bcsp-search-workspace__filters::-webkit-scrollbar-track { background: transparent; }

.bcsp-search-workspace__filters:focus-visible {
  outline: 2px solid var(--bcsp-focus);
  outline-offset: -2px;
}

/* Rail header: title + two-line intro, not sticky. */
.bcsp-search-workspace__header {
  display: grid;
  gap: 0.25rem;
  padding: var(--bcsp-space-3) var(--bcsp-space-4) var(--bcsp-space-2);
}

.bcsp-search-workspace__header h3,
.bcsp-search-workspace__header p {
  margin: 0;
}

.bcsp-search-workspace__header h3 {
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-subtitle);
  font-weight: 600;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-subtitle);
  text-transform: none;
}

.bcsp-search-workspace__header p {
  color: var(--bcsp-ink-muted);
  font-size: 0.8125rem;
  line-height: 1.25rem;
}

.bcsp-search-workspace__scope {
  display: grid;
  gap: var(--bcsp-space-2);
  min-width: 0;
  padding: 0 var(--bcsp-space-4) var(--bcsp-space-3);
}

/* Inline danger banner (spec 4.9): tone dot, tinted container, ink text. */
.bcsp-search-workspace__scope-error {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.75rem;
  align-items: start;
  margin: 0;
  padding: 0.75rem 1rem;
  border: 1px solid var(--bcsp-danger-line);
  border-radius: 0.5rem;
  color: var(--bcsp-ink);
  background: var(--bcsp-danger-tint);
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
  overflow-wrap: anywhere;
}

.bcsp-search-workspace__scope-error::before {
  width: 1rem;
  height: 1rem;
  margin-top: 0.1875rem;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-danger);
  content: '';
}

/* ---- Sticky submit footer: last direct child of the rail ---- */
.bcsp-search-workspace__submit {
  position: sticky;
  bottom: 0;
  z-index: var(--bcsp-z-rail-footer);
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--bcsp-space-2);
  align-items: center;
  min-height: 4.5rem;
  margin-top: auto;
  padding: var(--bcsp-space-2) var(--bcsp-space-4);
  padding-bottom: max(var(--bcsp-space-2), env(safe-area-inset-bottom));
  border-top: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
  box-shadow: var(--bcsp-elev-up);
}

.bcsp-search-workspace__submit-summary {
  min-width: 0;
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  font-variant-numeric: tabular-nums;
  line-height: var(--bcsp-lh-meta);
  overflow-wrap: anywhere;
}

.bcsp-search-workspace__submit-cell {
  display: grid;
  min-width: 0;
  justify-items: stretch;
}

.bcsp-search-workspace__submit .query-scope__cell--search {
  display: grid;
  gap: 0.375rem;
  min-width: 0;
}

.bcsp-search-workspace__submit .bcsp-action { min-width: 7rem; }

.bcsp-search-workspace__submit .query-scope__status {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
  overflow-wrap: anywhere;
}

/* ---- Results column ---- */
.bcsp-search-workspace__results {
  padding: 0;
  background: transparent;
}

.bcsp-search-workspace__state {
  display: grid;
  min-width: 0;
  align-content: start;
}

.bcsp-search-workspace__state:not([data-query-state='courses']) {
  min-height: 26rem;
  align-content: center;
}

/* Compact state panels (spec 4.13): centred column, 40px glyph circle, 16px title. */
.bcsp-search-state {
  display: grid;
  max-width: 28rem;
  justify-items: center;
  gap: var(--bcsp-space-3);
  margin: 0 auto;
  padding: var(--bcsp-space-6) var(--bcsp-space-4);
  text-align: center;
}

.bcsp-search-state__marker {
  display: inline-flex;
  width: 2.5rem;
  height: 2.5rem;
  align-items: center;
  justify-content: center;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-2);
  background: var(--bcsp-surface-2);
  font-family: var(--bcsp-font-sans);
  font-size: 1.125rem;
  font-weight: 600;
  line-height: 1;
}

.bcsp-search-state--error .bcsp-search-state__marker {
  color: var(--bcsp-danger);
  background: var(--bcsp-danger-tint);
}

.bcsp-search-state--loading .bcsp-search-state__marker {
  color: var(--bcsp-info);
  background: var(--bcsp-info-tint);
}

.bcsp-search-state__copy {
  display: grid;
  gap: var(--bcsp-space-1);
  min-width: 0;
  justify-items: center;
}

.bcsp-search-state__copy h4 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.5rem;
  text-transform: none;
  text-wrap: balance;
}

.bcsp-search-state__copy > span {
  max-width: 58ch;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body-lg);
  line-height: var(--bcsp-lh-body-lg);
}

@keyframes bcsp-search-state-turn {
  to { transform: rotate(360deg); }
}

@media (prefers-reduced-motion: no-preference) {
  .bcsp-search-state--loading .bcsp-search-state__marker {
    animation: bcsp-search-state-turn 1.4s linear infinite;
  }
}

/* Detail route chrome. */
.bcsp-search-workspace__detail-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-2);
  margin-bottom: var(--bcsp-space-3);
  padding-bottom: var(--bcsp-space-2);
  border-bottom: 1px solid var(--bcsp-line-soft);
}

.bcsp-search-workspace__route-meta {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-mono);
  font-size: var(--bcsp-text-meta);
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
  text-transform: none;
}

.bcsp-search-workspace__back {
  display: inline-flex;
  min-height: var(--bcsp-control-h);
  align-items: center;
  gap: var(--bcsp-space-1);
  padding: 0 0.75rem;
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink-2);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  letter-spacing: 0;
  text-decoration: none;
  text-transform: none;
}

/* Empty-result diagnosis: a quiet card with 44px action rows. */
.bcsp-search-diagnosis {
  max-width: 28rem;
  margin: 0 auto var(--bcsp-space-4);
  padding: var(--bcsp-space-3) var(--bcsp-space-4);
  border: 1px solid var(--bcsp-line);
  border-radius: var(--bcsp-radius-3);
  background: var(--bcsp-paper-raised);
}

.bcsp-search-diagnosis h5 {
  margin: 0 0 0.25rem;
  font-size: var(--bcsp-text-subtitle);
  font-weight: 600;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-subtitle);
  text-transform: none;
}

.bcsp-search-diagnosis p {
  margin: 0 0 var(--bcsp-space-2);
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
}

.bcsp-search-diagnosis p:last-child { margin-bottom: 0; }

.bcsp-search-diagnosis__list {
  display: grid;
  gap: 0.25rem;
  margin: 0;
  padding: 0;
  list-style: none;
}

.bcsp-search-diagnosis__action {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--bcsp-space-2);
  align-items: center;
  width: 100%;
  min-height: var(--bcsp-control-h);
  padding: 0.5rem 0.75rem;
  border: 0;
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink);
  background: transparent;
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
  text-align: left;
  cursor: pointer;
}

.bcsp-search-diagnosis__action > span { overflow-wrap: anywhere; }

.bcsp-search-diagnosis__action data {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-mono);
  font-size: var(--bcsp-text-data);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.bcsp-search-diagnosis__action:disabled {
  color: var(--bcsp-ink-muted);
  cursor: not-allowed;
}

@media (hover: hover) and (pointer: fine) {
  .bcsp-search-workspace__back:hover {
    color: var(--bcsp-ink);
    background: var(--bcsp-surface-2);
  }

  .bcsp-search-diagnosis__action:not(:disabled):hover {
    background: var(--bcsp-surface-2);
  }
}

[data-bcsp-locale='zh-CN'] .bcsp-search-workspace__submit-summary,
[data-bcsp-locale='zh-CN'] .bcsp-search-workspace__submit .query-scope__status,
[data-bcsp-locale='zh-CN'] .bcsp-search-workspace__route-meta {
  font-size: 0.8125rem;
  line-height: 1.25rem;
}

@container (max-width: 20rem) {
  .bcsp-search-workspace__submit { grid-template-columns: minmax(0, 1fr); }
  .bcsp-search-workspace__submit .bcsp-action { width: 100%; }
}

@media (max-width: 67.999rem) {
  .bcsp-search-workspace {
    grid-template-columns: minmax(22rem, 26rem) minmax(0, 1fr);
  }
}

/* Workspace boundary: below this point the rail is an in-flow card above the results. */
@media (max-width: 47.999rem) {
  .bcsp-search-workspace {
    grid-template-columns: minmax(0, 1fr);
    row-gap: var(--bcsp-space-4);
  }

  .bcsp-search-workspace__filters {
    position: static;
    top: auto;
    max-height: none;
    overflow: visible;
    box-shadow: none;
  }

  /* The rail no longer scrolls, so its sticky bands sit under the app bar instead. */
  .bcsp-search-workspace__filters .filter-panel__active {
    top: calc(var(--bcsp-navigation-height, 3.5rem) + var(--bcsp-readiness-height, 0px));
  }

  .bcsp-search-workspace__filters .filter-panel__group-head {
    top: calc(var(--bcsp-navigation-height, 3.5rem) + var(--bcsp-readiness-height, 0px) + var(--bcsp-rail-strip-h, 3.25rem));
  }

  .bcsp-search-workspace__filters .filter-panel__row {
    scroll-margin-top: calc(var(--bcsp-navigation-height, 3.5rem) + var(--bcsp-readiness-height, 0px) + var(--bcsp-rail-strip-h, 3.25rem) + 3rem);
  }

  .bcsp-search-workspace__submit {
    border-radius: 0 0 var(--bcsp-radius-3) var(--bcsp-radius-3);
  }

  .bcsp-search-workspace__state:not([data-query-state='courses']) {
    min-height: 18rem;
  }
}
`;

export function SearchWorkspaceStyles() {
  return <style data-bcsp-search-workspace-styles="">{SEARCH_WORKSPACE_CSS}</style>;
}
