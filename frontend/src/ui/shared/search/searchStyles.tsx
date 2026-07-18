export const SEARCH_WORKSPACE_CSS = String.raw`
.bcsp-search-workspace {
  display: grid;
  grid-template-columns: clamp(26rem, 28vw, 34rem) minmax(0, 1fr);
  margin-top: var(--bcsp-space-3);
  border-top: 1px solid var(--bcsp-line);
  border-right: 1px solid var(--bcsp-line);
  border-bottom: 1px solid var(--bcsp-line);
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-search-workspace__filters,
.bcsp-search-workspace__results {
  min-width: 0;
}

.bcsp-search-workspace__scope {
  min-width: 0;
}

.bcsp-search-workspace[data-detail-route='true'] {
  grid-template-columns: minmax(0, 1fr);
}

.bcsp-search-workspace__filters {
  container-type: inline-size;
  position: sticky;
  top: var(--bcsp-navigation-height, 3.5rem);
  align-self: start;
  max-height: calc(100vh - var(--bcsp-navigation-height, 3.5rem));
  max-height: calc(100dvh - var(--bcsp-navigation-height, 3.5rem));
  overflow: auto;
  overscroll-behavior: contain;
  touch-action: pan-y;
  border-right: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
  scrollbar-color: var(--bcsp-ink-muted) var(--bcsp-paper-raised);
  scrollbar-gutter: stable;
  scrollbar-width: thin;
}

.bcsp-search-workspace__filters::-webkit-scrollbar { width: 0.625rem; }
.bcsp-search-workspace__filters::-webkit-scrollbar-track {
  border-left: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}
.bcsp-search-workspace__filters::-webkit-scrollbar-thumb {
  border: 2px solid var(--bcsp-paper-raised);
  border-radius: 0;
  background: var(--bcsp-ink-muted);
}
.bcsp-search-workspace__filters::-webkit-scrollbar-thumb:hover {
  background: var(--bcsp-ink);
}
.bcsp-search-workspace__filters:focus-visible {
  outline: 2px solid var(--bcsp-accent);
  outline-offset: -2px;
}

.bcsp-search-workspace__results {
  padding: var(--bcsp-space-4);
  background: var(--bcsp-paper);
}

.bcsp-search-workspace__header {
  display: grid;
  gap: var(--bcsp-space-2);
  padding: var(--bcsp-space-3);
  border-bottom: 1px solid var(--bcsp-line);
}

.bcsp-search-workspace__header h3,
.bcsp-search-workspace__header p {
  margin: 0;
}

.bcsp-search-workspace__scope-error {
  margin: 0;
  padding: var(--bcsp-space-3);
  border-bottom: 1px solid var(--bcsp-line);
  border-left: 4px solid var(--bcsp-accent);
  color: var(--bcsp-ink);
  font-size: 0.78rem;
  line-height: 1.5;
}

.bcsp-search-workspace__header h3 {
  font-size: 1.1rem;
  font-weight: 850;
  letter-spacing: -0.03em;
  text-transform: uppercase;
}

.bcsp-search-workspace__header p {
  color: var(--bcsp-ink-muted);
  font-size: 0.78rem;
  line-height: 1.5;
}

.bcsp-search-workspace__state {
  display: grid;
  align-content: center;
  min-height: 26rem;
}

.bcsp-search-state {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--bcsp-space-2);
  align-items: start;
  padding: var(--bcsp-space-3);
  border: 1px solid var(--bcsp-line);
  border-left: 4px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}

.bcsp-search-state--error { border-left-color: var(--bcsp-danger, #8f1d14); }
.bcsp-search-state__marker {
  color: var(--bcsp-accent);
  font-family: var(--bcsp-font-data);
  font-weight: 800;
  line-height: 1.4;
}
.bcsp-search-state__copy { display: grid; gap: 0.25rem; min-width: 0; }
.bcsp-search-state__copy h4 { margin: 0; font-size: 0.82rem; }
.bcsp-search-state__copy > span {
  color: var(--bcsp-ink-muted);
  font-size: 0.76rem;
  line-height: 1.5;
}

.bcsp-search-workspace__detail-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-2);
  margin-bottom: var(--bcsp-space-3);
  padding-bottom: var(--bcsp-space-3);
  border-bottom: 1px solid var(--bcsp-line);
}

.bcsp-search-workspace__route-meta {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: 0.68rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.bcsp-search-workspace__back {
  color: var(--bcsp-ink);
  font-family: var(--bcsp-font-data);
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-decoration-thickness: 1px;
  text-underline-offset: 0.22em;
  text-transform: uppercase;
}

@media (hover: hover) and (pointer: fine) {
  .bcsp-search-workspace__back:hover {
    color: var(--bcsp-accent);
  }
}

@media (max-width: 67.999rem) {
  .bcsp-search-workspace {
    grid-template-columns: minmax(24rem, 30rem) minmax(0, 1fr);
  }
}

/* RC3 workspace boundary: below this point the two work areas become one flow. */
@media (max-width: 47.999rem) {
  .bcsp-search-workspace {
    grid-template-columns: minmax(0, 1fr);
  }

  .bcsp-search-workspace__filters {
    position: static;
    top: auto;
    max-height: none;
    overflow: visible;
    border-right: 0;
    border-bottom: 1px solid var(--bcsp-line);
  }

  .bcsp-search-workspace__results {
    padding: var(--bcsp-space-3);
  }

  .bcsp-search-workspace__state {
    min-height: 18rem;
  }
}
`;

export function SearchWorkspaceStyles() {
  return <style data-bcsp-search-workspace-styles="">{SEARCH_WORKSPACE_CSS}</style>;
}
