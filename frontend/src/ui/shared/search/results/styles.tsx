export const SEARCH_RESULTS_CSS = String.raw`
.search-results {
  --result-paper: var(--bcsp-paper, #efeee8);
  --result-ink: var(--bcsp-ink, #11110e);
  --result-muted: var(--bcsp-muted-ink, #5b5a53);
  --result-accent: var(--bcsp-accent, #d42b1e);
  color: var(--result-ink);
  display: grid;
  font-family: var(--bcsp-font-sans, Arial, Helvetica, sans-serif);
  gap: 0;
}

.search-results,
.search-results * {
  box-sizing: border-box;
}

.search-results__header,
.search-results__group-header,
.search-results__variant-summary,
.search-results__section-header,
.search-results__detail-header,
.search-results__pagination {
  align-items: start;
  display: grid;
  gap: 0.75rem;
  grid-template-columns: minmax(0, 1fr) auto;
}

.search-results__header {
  border-block: 2px solid var(--result-ink);
  padding: 0.8rem 0;
}

.search-results__eyebrow,
.search-results__meta,
.search-results__badge,
.search-results__label,
.search-results__witness-title,
.search-results__occurrence-title,
.search-results__button,
.search-results__section-link,
.search-results__page-label {
  font-family: var(--bcsp-font-mono, "Courier New", monospace);
  font-size: 0.72rem;
  letter-spacing: 0.075em;
  line-height: 1.35;
  text-transform: uppercase;
}

.search-results__eyebrow,
.search-results__meta,
.search-results__label,
.search-results__page-label {
  color: var(--result-muted);
  overflow-wrap: anywhere;
}

.search-results__heading,
.search-results__group-title,
.search-results__variant-title,
.search-results__section-title,
.search-results__detail-title {
  font-weight: 800;
  letter-spacing: -0.035em;
  line-height: 0.98;
  margin: 0;
  overflow-wrap: anywhere;
}

.search-results__heading {
  font-size: clamp(1.5rem, 3vw, 2.8rem);
}

.search-results__group-title,
.search-results__detail-title {
  font-size: clamp(1.2rem, 2.2vw, 2rem);
}

.search-results__variant-title,
.search-results__section-title {
  font-size: 1rem;
  letter-spacing: -0.018em;
  line-height: 1.15;
}

.search-results__count {
  color: var(--result-accent);
  font-family: var(--bcsp-font-mono, "Courier New", monospace);
  font-size: clamp(1.6rem, 4vw, 3.2rem);
  font-weight: 800;
  line-height: 0.85;
}

.search-results__list,
.search-results__variant-list,
.search-results__section-list,
.search-results__witness-list,
.search-results__occurrence-list,
.search-results__field-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.search-results__group,
.search-results__standalone-section,
.search-results__detail {
  border-bottom: 2px solid var(--result-ink);
  padding: 1rem 0;
}

.search-results__group-header,
.search-results__detail-header {
  margin-bottom: 1rem;
}

.search-results__identity {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem 0.75rem;
  margin-top: 0.35rem;
}

.search-results__variant {
  border-top: 1px solid var(--result-ink);
}

.search-results__variant-summary {
  min-height: 2.75rem;
  padding: 0.8rem 0;
}

.search-results__variant-summary > * {
  min-width: 0;
}

.search-results__variant-facts,
.search-results__live,
.search-results__field-list {
  display: grid;
  gap: 1px;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  background: var(--result-ink);
  border: 1px solid var(--result-ink);
}

.search-results__fact,
.search-results__field {
  background: var(--result-paper);
  display: grid;
  gap: 0.2rem;
  min-width: 0;
  padding: 0.55rem;
}

.search-results__field--full-row {
  grid-column: 1 / -1;
}

.search-results__value {
  font-size: 0.86rem;
  line-height: 1.35;
  margin: 0;
  overflow-wrap: anywhere;
}

.search-results__section {
  border-top: 1px solid var(--result-ink);
  padding: 0.8rem 0;
}

.search-results__section-header {
  margin-bottom: 0.65rem;
}

.search-results__section-actions {
  align-items: end;
  display: grid;
  gap: 0.35rem;
  justify-items: end;
}

.search-results__badges {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
  justify-content: flex-end;
}

.search-results__badge {
  border: 1px solid currentColor;
  display: inline-block;
  padding: 0.18rem 0.35rem;
}

.search-results__badge--match,
.search-results__badge--open {
  color: var(--result-ink);
}

.search-results__badge--uncertain,
.search-results__badge--no-match,
.search-results__badge--closed,
.search-results__badge--unknown {
  color: var(--result-accent);
}

.search-results__button,
.search-results__section-link,
.search-results__section-disclosure-summary {
  background: transparent;
  border: 1px solid var(--result-ink);
  border-radius: 0;
  color: var(--result-ink);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  min-height: 2.75rem;
  padding: 0.45rem 0.55rem;
  text-align: center;
  text-decoration: none;
  transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1));
}

.search-results__button:active:not(:focus-visible),
.search-results__section-link:active:not(:focus-visible),
.search-results__section-disclosure-summary:active:not(:focus-visible) {
  transform: scale(0.97);
}

.search-results__button:focus-visible,
.search-results__section-link:focus-visible,
.search-results__section-disclosure-summary:focus-visible {
  outline: 3px solid var(--bcsp-focus, var(--result-accent));
  outline-offset: 2px;
}

@media (hover: hover) and (pointer: fine) {
  .search-results__button:hover,
  .search-results__section-link:hover,
  .search-results__section-disclosure-summary:hover {
    background: var(--result-ink);
    color: var(--result-paper);
  }
}

.search-results__section-disclosure {
  margin: 0;
  border-top: 1px solid var(--result-ink);
}

.search-results__section-disclosure-summary {
  display: flex;
  width: 100%;
  justify-content: space-between;
  gap: 0.75rem;
  border: 0;
  list-style: none;
}

.search-results__section-disclosure-summary::-webkit-details-marker {
  display: none;
}

.search-results__section-disclosure-summary::marker {
  content: '';
}

.search-results__section-disclosure-action {
  color: var(--result-accent);
  font-size: 1rem;
  font-weight: 900;
}

.search-results__witness,
.search-results__occurrences {
  border-left: 0.35rem solid var(--result-ink);
  margin-top: 0.65rem;
  padding-left: 0.65rem;
}

.search-results__witness--uncertain {
  border-left-color: var(--result-accent);
}

.search-results__witness-list,
.search-results__occurrence-list {
  display: grid;
  gap: 0.35rem;
  margin-top: 0.35rem;
}

.search-results__witness-item,
.search-results__occurrence {
  border-top: 1px solid color-mix(in srgb, var(--result-ink) 35%, transparent);
  display: grid;
  font-size: 0.8rem;
  gap: 0.2rem 0.6rem;
  grid-template-columns: minmax(8rem, auto) 1fr;
  line-height: 1.35;
  padding-top: 0.35rem;
}

.search-results__empty {
  border-bottom: 2px solid var(--result-ink);
  font-size: 0.95rem;
  margin: 0;
  padding: 1.4rem 0;
}

.search-results__pagination {
  border-bottom: 2px solid var(--result-ink);
  padding: 0.8rem 0;
}

.search-results__pagination-actions {
  display: flex;
  gap: 0.4rem;
}

.search-results__button:disabled {
  color: var(--result-muted);
  cursor: not-allowed;
  opacity: 0.55;
}

@media (max-width: 47.99rem) {
  .search-results__header,
  .search-results__group-header,
  .search-results__variant-summary,
  .search-results__section-header,
  .search-results__detail-header,
  .search-results__pagination {
    grid-template-columns: minmax(0, 1fr);
  }

  .search-results__variant-facts,
  .search-results__live,
  .search-results__field-list {
    grid-template-columns: minmax(0, 1fr);
  }

  .search-results__section-actions {
    justify-items: start;
  }

  .search-results__badges {
    justify-content: flex-start;
  }

  .search-results__witness-item,
  .search-results__occurrence {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (prefers-reduced-motion: reduce) {
  .search-results__button,
  .search-results__section-link,
  .search-results__section-disclosure-summary {
    transition: none;
  }

  .search-results__button:active:not(:focus-visible),
  .search-results__section-link:active:not(:focus-visible),
  .search-results__section-disclosure-summary:active:not(:focus-visible) {
    transform: none;
  }
}
`;

export function SearchResultsStyles() {
  return <style data-bcsp-search-results-styles="">{SEARCH_RESULTS_CSS}</style>;
}
