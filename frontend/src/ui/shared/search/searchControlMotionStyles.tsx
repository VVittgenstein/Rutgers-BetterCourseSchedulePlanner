/* The only home for filter-rail transitions (FILTER_PANEL_CSS stays
 * transition-free). Colour, border, shadow and opacity only, 120ms ease-out;
 * the press transform comes from the design-system .bcsp-action rule. */
export const SEARCH_CONTROL_MOTION_CSS = String.raw`
.bcsp-search-workspace__filters .bcsp-action,
.bcsp-search-workspace__filters .filter-panel__minor-action,
.bcsp-search-workspace__filters .filter-panel__input,
.bcsp-search-workspace__filters .filter-panel__select,
.bcsp-search-workspace__filters .filter-panel__check,
.bcsp-search-workspace__filters .filter-panel__incomplete,
.bcsp-search-workspace__filters .filter-panel__dictionary-option,
.bcsp-search-workspace__filters .filter-panel__chip button,
.bcsp-search-workspace__filters .filter-panel__token button,
.bcsp-search-workspace__filters .filter-panel__window-list button,
.bcsp-search-workspace__filters .query-scope__option,
.bcsp-search-workspace__filters .query-scope__term,
.bcsp-search-workspace__filters .bcsp-search-workspace__back,
.bcsp-search-diagnosis__action {
  transition-property: background-color, border-color, color, box-shadow, opacity;
  transition-duration: var(--bcsp-dur-1, 120ms);
  transition-timing-function: var(--bcsp-ease-out);
}

.bcsp-search-workspace__filters .filter-panel__dictionary-input + .filter-panel__dictionary-options[data-open='true'] {
  transition-property: opacity;
  transition-duration: var(--bcsp-dur-1, 120ms);
  transition-timing-function: var(--bcsp-ease-out);
}

@media (prefers-reduced-motion: reduce) {
  .bcsp-search-workspace__filters *,
  .bcsp-search-workspace__filters *::before,
  .bcsp-search-workspace__filters *::after,
  .bcsp-search-diagnosis__action {
    scroll-behavior: auto !important;
    animation: none !important;
    transition-duration: 0.001ms !important;
  }
}
`;

export function SearchControlMotionStyles() {
  return <style data-bcsp-search-control-motion="">{SEARCH_CONTROL_MOTION_CSS}</style>;
}
