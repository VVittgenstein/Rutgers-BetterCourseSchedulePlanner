export const SEARCH_WORKSPACE_CSS = String.raw`
.bcsp-search-workspace {
  display: grid;
  grid-template-columns: clamp(20rem, 22vw, 24rem) minmax(0, 1fr);
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

.bcsp-search-workspace[data-detail-route='true'] {
  grid-template-columns: minmax(0, 1fr);
}

.bcsp-search-workspace__filters {
  container-type: inline-size;
  position: sticky;
  top: 0;
  align-self: start;
  max-height: 100dvh;
  overflow: auto;
  border-right: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}

.bcsp-search-workspace__results {
  padding: var(--bcsp-space-4);
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
  min-height: 26rem;
  align-content: center;
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
    grid-template-columns: minmax(18rem, 21rem) minmax(0, 1fr);
  }
}

@media (max-width: 47.999rem) {
  .bcsp-search-workspace {
    grid-template-columns: minmax(0, 1fr);
  }

  .bcsp-search-workspace__filters {
    position: static;
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
