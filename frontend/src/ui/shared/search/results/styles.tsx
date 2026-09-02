export const SEARCH_RESULTS_CSS = String.raw`
/* Results, course detail and section detail (design spec v2 §4.5–4.7, §4.10, §4.11, §5.4, §11.1).
   Colours and fonts come from the design-system tokens only; this string carries no literal colour. */
.search-results {
  color: var(--bcsp-ink);
  display: grid;
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  gap: 0;
  line-height: var(--bcsp-lh-body);
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
  gap: 0.5rem 1rem;
  grid-template-columns: minmax(0, 1fr) auto;
}

.search-results__header > *,
.search-results__group-header > *,
.search-results__variant-summary > *,
.search-results__section-header > *,
.search-results__detail-header > * {
  min-width: 0;
}

/* ---- Results header: sticky under the app bar (§5.4) ---- */
.search-results__header {
  align-items: center;
  background: var(--bcsp-paper);
  border-bottom: 1px solid var(--bcsp-line);
  margin-bottom: var(--bcsp-space-3);
  min-height: 3rem;
  padding: var(--bcsp-space-1) 0;
  position: sticky;
  top: calc(var(--bcsp-navigation-height, 3.5rem) + var(--bcsp-readiness-height, 0px));
  z-index: var(--bcsp-z-results-head);
}

.search-results__heading {
  font-size: var(--bcsp-text-title);
  font-weight: 600;
  line-height: var(--bcsp-lh-title);
  margin: 0;
  overflow-wrap: anywhere;
  text-wrap: balance;
}

.search-results__count {
  align-items: center;
  background: var(--bcsp-surface-2);
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink);
  display: inline-flex;
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-title);
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  line-height: var(--bcsp-lh-title);
  min-height: 1.75rem;
  padding: 0 0.625rem;
}

/* ---- Kickers: hidden in card/detail headers, one muted line on variants (§3, §5.4) ---- */
.search-results__eyebrow {
  display: none;
}

.search-results__variant-summary .search-results__eyebrow {
  color: var(--bcsp-ink-muted);
  display: block;
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
  margin-bottom: 0.125rem;
  overflow-wrap: anywhere;
  text-transform: none;
}

/* ---- Meta / labels ---- */
.search-results__meta,
.search-results__label,
.search-results__page-label {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
  overflow-wrap: anywhere;
  text-transform: none;
}

.search-results__page-label {
  font-feature-settings: "tnum" 1;
  font-variant-numeric: tabular-nums;
}

p.search-results__meta {
  margin: var(--bcsp-space-1) 0 0;
}

/* Identifiers only (term codes, campus codes) sit in mono (§3). */
data.search-results__meta {
  font-family: var(--bcsp-font-mono);
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-data);
  font-variant-numeric: tabular-nums;
  line-height: var(--bcsp-lh-data);
}

.search-results__identity {
  align-items: baseline;
  display: flex;
  flex-wrap: wrap;
  gap: 0.125rem 0;
  margin-top: 0.125rem;
}

/* The separator trails its own item, so a wrapped meta line never starts on a dot. */
.search-results__identity > *:not(:last-child)::after {
  color: var(--bcsp-ink-muted);
  content: '·';
  margin: 0 0.35em;
}

/* ---- Titles (§3) ---- */
.search-results__group-title,
.search-results__variant-title,
.search-results__section-title,
.search-results__detail-title {
  font-weight: 600;
  letter-spacing: 0;
  margin: 0;
  overflow-wrap: anywhere;
  text-transform: none;
}

/* The card headline is the course NAME, not its code (§4.5, §11 amendment). */
.search-results__group-title {
  font-size: var(--bcsp-text-subtitle);
  line-height: var(--bcsp-lh-subtitle);
  text-wrap: balance;
}

.search-results__variant-title {
  font-size: var(--bcsp-text-subtitle);
  line-height: var(--bcsp-lh-subtitle);
  text-wrap: balance;
}

.search-results__section-title {
  font-family: var(--bcsp-font-mono);
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-subtitle);
  font-variant-numeric: tabular-nums slashed-zero;
  line-height: var(--bcsp-lh-subtitle);
}

.search-results__detail-title {
  font-size: var(--bcsp-text-display);
  line-height: var(--bcsp-lh-display);
  text-wrap: balance;
}

/* ---- Lists ---- */
.search-results__list,
.search-results__variant-list,
.search-results__section-list,
.search-results__field-list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.search-results__list {
  display: grid;
  gap: var(--bcsp-space-2);
}

/* ---- Course group cards (§4.5, §5.4) ---- */
.search-results__group,
.search-results__standalone-section,
.search-results__detail {
  background: var(--bcsp-paper-raised);
  border: 1px solid var(--bcsp-line);
  border-radius: var(--bcsp-radius-3);
  container-name: bcsp-results-card;
  container-type: inline-size;
  padding: var(--bcsp-space-3) var(--bcsp-space-4);
}

.search-results__detail {
  padding: var(--bcsp-space-4);
}

.search-results__group-header {
  align-items: center;
  margin-bottom: var(--bcsp-space-2);
  min-height: var(--bcsp-control-h);
}

.search-results__detail-header {
  border-bottom: 1px solid var(--bcsp-line);
  margin-bottom: var(--bcsp-space-4);
  padding-bottom: var(--bcsp-space-3);
}

.search-results__detail-header .search-results__meta {
  display: block;
  font-family: var(--bcsp-font-mono);
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-data);
  font-variant-numeric: tabular-nums;
  line-height: var(--bcsp-lh-data);
  margin-top: 0.25rem;
}

/* ---- Variants: hairline separated (§5.4) ---- */
.search-results__variant-list {
  border-top: 1px solid var(--bcsp-line-soft);
}

.search-results__variant + .search-results__variant {
  border-top: 1px solid var(--bcsp-line-soft);
}

.search-results__variant-summary {
  align-items: center;
  min-height: var(--bcsp-control-h);
  padding: var(--bcsp-space-2) 0 var(--bcsp-space-1);
}

.search-results__detail > .search-results__variant {
  border-top: 1px solid var(--bcsp-line-soft);
  margin-top: var(--bcsp-space-4);
  padding-top: var(--bcsp-space-3);
}

.search-results__detail > .search-results__variant:first-of-type {
  border-top: 0;
  margin-top: 0;
  padding-top: 0;
}

/* ---- Fact grids: explicit column counts, never an orphan track (§11.1) ---- */
.search-results__live {
  border-top: 1px solid var(--bcsp-line-soft);
  display: grid;
  gap: var(--bcsp-space-1) var(--bcsp-space-4);
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-top: var(--bcsp-space-4);
  padding: var(--bcsp-space-4) 0 0;
}

.search-results__fact,
.search-results__field {
  background: transparent;
  display: grid;
  gap: 0.125rem;
  min-width: 0;
  padding: 0;
}

.search-results__field dt,
.search-results__field dd {
  margin: 0;
}

.search-results__value {
  color: var(--bcsp-ink);
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
  margin: 0;
  overflow-wrap: anywhere;
}

.search-results__field-list {
  display: grid;
  gap: var(--bcsp-space-3) var(--bcsp-space-5);
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

/* Three tracks: a short last row grows instead of leaving an empty cell (§11.1). */
.search-results__field:not(.search-results__field--full-row):nth-child(3n + 1):last-child,
.search-results__field:not(.search-results__field--full-row):nth-child(3n + 1):has(+ .search-results__field--full-row) {
  grid-column: 1 / -1;
}

.search-results__field:not(.search-results__field--full-row):nth-child(3n + 2):last-child,
.search-results__field:not(.search-results__field--full-row):nth-child(3n + 2):has(+ .search-results__field--full-row) {
  grid-column: span 2;
}

.search-results__field-list + .search-results__field-list,
.search-results__field-list + .search-results__section,
.search-results__field-list + .search-results__section-disclosure {
  border-top: 1px solid var(--bcsp-line-soft);
  margin-top: var(--bcsp-space-4);
  padding-top: var(--bcsp-space-4);
}

.search-results__field--full-row {
  grid-column: 1 / -1;
}

/* ---- Section rows on the shared grid: index | body | actions (§4.10, Phase 1 DOM) ---- */
.search-results__section-list {
  background: var(--bcsp-surface-2);
  border-radius: 0.5rem;
  display: grid;
  margin-top: var(--bcsp-space-1);
  padding: 0 var(--bcsp-space-2);
}

.search-results__section {
  border-top: 1px solid var(--bcsp-line);
  padding: var(--bcsp-space-2) 0;
}

.search-results__section-list > .search-results__section:first-child {
  border-top: 0;
}

.search-results__standalone-section > .search-results__section,
.search-results__detail > .search-results__section {
  border-top: 1px solid var(--bcsp-line-soft);
  margin-top: var(--bcsp-space-2);
  padding-top: var(--bcsp-space-3);
}

.search-results__section-header {
  align-items: start;
  gap: 0.5rem 1rem;
}

/* Row reading order: index + open badge | meeting summary | actions (§4.10). */
.search-results__section-summary {
  align-items: start;
  display: grid;
  gap: 0.25rem var(--bcsp-space-3);
  grid-template-columns: minmax(4.5rem, auto) minmax(0, 1fr);
  min-width: 0;
}

.search-results__section-identity {
  display: grid;
  gap: 0.25rem;
  justify-items: start;
  min-width: 0;
}

.search-results__section-body {
  display: grid;
  gap: 0.125rem;
  min-width: 0;
}

.search-results__section-actions {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: var(--bcsp-space-1);
  justify-content: flex-end;
}

.search-results__badges {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 0.375rem;
  justify-content: flex-end;
}

/* A plain match prints no badge, so the slot must not reserve a gap. */
.search-results__badges:empty {
  display: none;
}

/* ---- Status badges: tinted pills via the tone map (§4.7) ---- */
.search-results__badge {
  align-items: center;
  background: var(--bcsp-surface-2);
  border: 1px solid var(--bcsp-line);
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-2);
  display: inline-flex;
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-micro);
  font-weight: 600;
  gap: 0.375rem;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-micro);
  min-height: 1.375rem;
  padding: 0 0.5rem;
  text-transform: none;
  white-space: nowrap;
}

.search-results__badge--match,
.search-results__badge--open {
  background: var(--bcsp-ok-tint);
  border-color: var(--bcsp-ok-line);
  color: var(--bcsp-ok);
}

.search-results__badge--uncertain,
.search-results__badge--unknown {
  background: var(--bcsp-warn-tint);
  border-color: var(--bcsp-warn-line);
  color: var(--bcsp-warn);
}

.search-results__badge--no-match {
  background: var(--bcsp-danger-tint);
  border-color: var(--bcsp-danger-line);
  color: var(--bcsp-danger);
}

.search-results__badge--closed {
  background: var(--bcsp-surface-2);
  border-color: var(--bcsp-line);
  color: var(--bcsp-ink-2);
}

/* Live badges carry a leading dot in the foreground colour. */
.search-results__badge--open::before,
.search-results__badge--closed::before,
.search-results__badge--unknown::before {
  background: currentColor;
  border-radius: var(--bcsp-radius-pill);
  content: '';
  flex: 0 0 auto;
  height: 0.375rem;
  width: 0.375rem;
}

/* ---- Buttons: secondary clone and quiet link (§4.1) ---- */
.search-results__button,
.search-results__section-link {
  align-items: center;
  background: var(--bcsp-paper-raised);
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink);
  cursor: pointer;
  display: inline-flex;
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  gap: var(--bcsp-space-1);
  justify-content: center;
  letter-spacing: 0;
  line-height: 1.25rem;
  min-height: var(--bcsp-control-h);
  padding: 0 var(--bcsp-space-3);
  text-align: center;
  text-decoration: none;
  text-transform: none;
  transition:
    background-color var(--bcsp-dur-1) var(--bcsp-ease-out),
    border-color var(--bcsp-dur-1) var(--bcsp-ease-out),
    color var(--bcsp-dur-1) var(--bcsp-ease-out),
    transform var(--bcsp-dur-1) var(--bcsp-ease-out);
  white-space: nowrap;
}

.search-results__section-actions .search-results__button,
.search-results__section-actions .search-results__section-link {
  font-size: var(--bcsp-text-data);
  padding: 0 var(--bcsp-space-2);
}

.search-results__section-link {
  background: transparent;
  border-color: transparent;
  color: var(--bcsp-ink-2);
}

.search-results__button:active:not(:disabled) {
  background: var(--bcsp-surface-3);
}

.search-results__button:active:not(:disabled):not(:focus-visible),
.search-results__section-link:active:not(:focus-visible) {
  transform: translateY(1px);
}

.search-results__button:disabled {
  background: var(--bcsp-surface-3);
  border-color: var(--bcsp-line);
  color: var(--bcsp-ink-muted);
  cursor: not-allowed;
  opacity: 1;
}

[data-bcsp-locale='zh-CN'] .search-results__button,
[data-bcsp-locale='zh-CN'] .search-results__section-link {
  line-height: 1.3;
  min-width: 4.5rem;
}

@media (hover: hover) and (pointer: fine) {
  .search-results__button:hover:not(:disabled) {
    background: var(--bcsp-surface-2);
    border-color: var(--bcsp-ink-muted);
  }

  .search-results__section-link:hover {
    background: var(--bcsp-surface-2);
    color: var(--bcsp-ink);
  }

  .search-results__section-disclosure-summary:hover {
    background: var(--bcsp-surface-2);
  }
}

/* ---- Section disclosure: 44px summary with a CSS chevron (§4.11) ---- */
.search-results__section-disclosure {
  border-top: 1px solid var(--bcsp-line-soft);
  margin: 0;
  padding-bottom: var(--bcsp-space-1);
}

.search-results__section-disclosure-summary {
  align-items: center;
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink-2);
  cursor: pointer;
  display: flex;
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 400;
  gap: var(--bcsp-space-2);
  justify-content: space-between;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-body);
  list-style: none;
  min-height: var(--bcsp-control-h);
  padding: 0 var(--bcsp-space-2) 0 0;
  text-transform: none;
  transition: background-color var(--bcsp-dur-1) var(--bcsp-ease-out);
  user-select: none;
  width: 100%;
}

.search-results__section-disclosure-summary::-webkit-details-marker {
  display: none;
}

.search-results__section-disclosure-summary::marker {
  content: '';
}

.search-results__section-disclosure-summary::after {
  border-bottom: 1.5px solid currentColor;
  border-right: 1.5px solid currentColor;
  content: '';
  flex: 0 0 auto;
  height: 0.375rem;
  margin: 0 0.25rem 0.125rem 0;
  transform: rotate(-45deg);
  transition: transform var(--bcsp-dur-2) var(--bcsp-ease-out);
  width: 0.375rem;
}

.search-results__section-disclosure[open] > .search-results__section-disclosure-summary::after {
  margin-bottom: 0.25rem;
  transform: rotate(45deg);
}

.search-results__section-disclosure-action {
  display: none;
}

/* ---- Meeting summary: one quiet line per occurrence, no label column (§5.4) ---- */
.search-results__schedule {
  color: var(--bcsp-ink-2);
  display: grid;
  gap: 0.125rem;
  margin: 0;
  min-width: 0;
}

.search-results__schedule-line {
  font-family: var(--bcsp-font-sans);
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-data);
  font-variant-numeric: tabular-nums;
  line-height: var(--bcsp-lh-data);
  margin: 0;
  overflow-wrap: anywhere;
}

/* ---- Uncertain evidence: a quiet warn line, never a filled block (§4.7) ---- */
.search-results__note {
  color: var(--bcsp-ink-2);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
  margin: var(--bcsp-space-1) 0 0;
  overflow-wrap: anywhere;
  padding-left: var(--bcsp-space-2);
}

.search-results__note--warn {
  border-left: 2px solid var(--bcsp-warn-line);
}

/* ---- Empty row (§4.13 dashed empty) ---- */
.search-results__empty {
  align-items: center;
  border: 1px dashed var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink-muted);
  display: flex;
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
  margin: 0;
  min-height: var(--bcsp-control-h);
  padding: var(--bcsp-space-1) var(--bcsp-space-3);
}

/* ---- Pagination ---- */
.search-results__pagination {
  align-items: center;
  border-top: 1px solid var(--bcsp-line);
  margin-top: var(--bcsp-space-3);
  padding: var(--bcsp-space-2) 0;
}

.search-results__pagination-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--bcsp-space-1);
}

/* ---- Chinese locale floors (§3) ---- */
[data-bcsp-locale='zh-CN'] .search-results__meta,
[data-bcsp-locale='zh-CN'] .search-results__label,
[data-bcsp-locale='zh-CN'] .search-results__page-label,
[data-bcsp-locale='zh-CN'] .search-results__variant-summary .search-results__eyebrow,
[data-bcsp-locale='zh-CN'] .search-results__note,
[data-bcsp-locale='zh-CN'] .search-results__schedule-line {
  font-size: var(--bcsp-text-data);
  line-height: 1.25rem;
}

[data-bcsp-locale='zh-CN'] .search-results__badge {
  font-size: var(--bcsp-text-data);
}

[data-bcsp-locale='zh-CN'] .search-results__identity > *:not(:last-child)::after {
  margin: 0 0.2em;
}

/* ---- Card-width fact grids (§11.1: explicit counts, no empty track) ---- */
@container bcsp-results-card (max-width: 34rem) {
  .search-results__live {
    grid-template-columns: minmax(0, 1fr);
  }
}

@container bcsp-results-card (max-width: 52rem) {
  .search-results__field-list {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  /* An odd trailing field grows across the row instead of leaving an empty cell. */
  .search-results__field:nth-child(odd):last-child,
  .search-results__field:nth-child(odd):has(+ .search-results__field--full-row) {
    grid-column: 1 / -1;
  }
}

@container bcsp-results-card (max-width: 34rem) {
  .search-results__field-list {
    grid-template-columns: minmax(0, 1fr);
  }
}

/* ---- Phone: rows wrap to two lines, actions full width (§5.4) ---- */
@media (max-width: 47.99rem) {
  .search-results__header,
  .search-results__group-header,
  .search-results__variant-summary,
  .search-results__section-header,
  .search-results__detail-header,
  .search-results__pagination {
    grid-template-columns: minmax(0, 1fr);
  }

  .search-results__group,
  .search-results__standalone-section,
  .search-results__detail {
    padding: var(--bcsp-space-3);
  }

  .search-results__section-summary {
    grid-template-columns: minmax(0, 1fr);
  }

  .search-results__section-actions,
  .search-results__badges {
    justify-content: flex-start;
  }

  .search-results__section-identity {
    align-items: center;
    display: flex;
    flex-wrap: wrap;
    gap: var(--bcsp-space-2);
  }
}

@media (max-width: 31.99rem) {
  .search-results__section-actions > *,
  .search-results__pagination-actions > * {
    flex: 1 1 100%;
  }

  .search-results__section-actions .search-results__button,
  .search-results__section-actions .search-results__section-link,
  .search-results__pagination-actions .search-results__button {
    width: 100%;
  }
}

@media (prefers-reduced-motion: reduce) {
  .search-results__button,
  .search-results__section-link,
  .search-results__section-disclosure-summary,
  .search-results__section-disclosure-summary::after {
    transition: none;
  }

  .search-results__button:active:not(:disabled):not(:focus-visible),
  .search-results__section-link:active:not(:focus-visible) {
    transform: none;
  }
}
`;

export function SearchResultsStyles() {
  return <style data-bcsp-search-results-styles="">{SEARCH_RESULTS_CSS}</style>;
}
