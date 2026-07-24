export const SEARCH_CONTROL_MOTION_CSS = String.raw`
@media (hover: hover) and (pointer: fine) {
  .bcsp-search-workspace__filters .bcsp-action:not(:disabled),
  .bcsp-search-workspace__filters button:not(:disabled),
  .query-scope__term,
  .query-scope__campus,
  .filter-panel__check,
  .filter-panel__dictionary-option,
  .filter-panel__chip,
  .filter-panel__token {
    transition-duration: 120ms;
    transition-property: background-color, border-color, color, box-shadow, opacity;
    transition-timing-function: ease-out;
  }
}

.bcsp-search-workspace__filters .bcsp-action,
.bcsp-search-workspace__filters button,
.query-scope__term,
.query-scope__campus,
.filter-panel__check,
.filter-panel__dictionary-option,
.filter-panel__chip,
.filter-panel__token {
  -webkit-tap-highlight-color: transparent;
}

.bcsp-search-workspace__filters .bcsp-action:active,
.bcsp-search-workspace__filters button:active {
  transform: none !important;
}

@media (prefers-reduced-motion: no-preference) {
  .bcsp-search-workspace__filters .bcsp-action[data-pointer-pressed='true'],
  .bcsp-search-workspace__filters button[data-pointer-pressed='true'] {
    transform: translateY(1px) !important;
  }
}

.query-scope__term:has(input:checked),
.query-scope__campus:has(input:checked) {
  box-shadow: inset 0 0 0 1px var(--bcsp-accent);
}

.query-scope__term:has(input:focus-visible),
.query-scope__campus:has(input:focus-visible),
.filter-panel__check:has(input:focus-visible),
.filter-panel__dictionary:has(.filter-panel__input:focus-visible) .filter-panel__dictionary-option[data-active='true'] {
  position: relative;
  z-index: 1;
  box-shadow: inset 0 0 0 2px var(--bcsp-focus);
}

.query-scope__action:has(.bcsp-action[aria-busy='true']),
.query-scope__search:has(.bcsp-action[aria-busy='true']) {
  background: color-mix(in srgb, var(--bcsp-accent) 9%, var(--bcsp-paper-raised));
  animation: bcsp-search-busy-confirm 160ms ease-out both;
}

.query-scope__action .bcsp-action[aria-busy='true'],
.query-scope__search .bcsp-action[aria-busy='true'] {
  cursor: progress;
}

.filter-panel__dictionary-option[data-active='true'],
.filter-panel__check:has(input:checked) {
  background: color-mix(in srgb, var(--bcsp-accent) 8%, var(--bcsp-paper-raised));
}

.filter-panel__validation-error,
.bcsp-search-workspace__scope [role='alert'] {
  animation: bcsp-search-feedback-emphasis 160ms ease-out both;
}

@keyframes bcsp-search-feedback-emphasis {
  from { opacity: .65; transform: translateX(-3px); }
  to { opacity: 1; transform: translateX(0); }
}

@keyframes bcsp-search-busy-confirm {
  from { opacity: .72; }
  to { opacity: 1; }
}

@media (prefers-reduced-motion: reduce) {
  .bcsp-search-workspace__filters *,
  .bcsp-search-workspace__filters *::before,
  .bcsp-search-workspace__filters *::after {
    scroll-behavior: auto !important;
    animation: none !important;
    transition-duration: 0ms !important;
  }

  .bcsp-search-workspace__filters .bcsp-action:active,
  .bcsp-search-workspace__filters button:active {
    transform: none !important;
  }
}
`;

export function SearchControlMotionStyles() {
  return <style data-bcsp-search-control-motion="">{SEARCH_CONTROL_MOTION_CSS}</style>;
}
