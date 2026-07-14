export const BCSP_DESIGN_SYSTEM_CSS = String.raw`
:root {
  color-scheme: light;
  --bcsp-paper: #efeee8;
  --bcsp-paper-raised: #f8f7f1;
  --bcsp-ink: #11110e;
  --bcsp-ink-muted: #5b5a53;
  --bcsp-line: #22221e;
  --bcsp-line-soft: #a6a49a;
  --bcsp-accent: #d42b1e;
  --bcsp-accent-ink: #ffffff;
  --bcsp-grid-unit: 0.5rem;
  --bcsp-space-1: 0.5rem;
  --bcsp-space-2: 0.75rem;
  --bcsp-space-3: 1rem;
  --bcsp-space-4: 1.5rem;
  --bcsp-space-5: 2rem;
  --bcsp-space-6: 3rem;
  --bcsp-font-display: Arial, "Helvetica Neue", "Nimbus Sans L", sans-serif;
  --bcsp-font-data: "Cascadia Mono", "IBM Plex Mono", "SFMono-Regular", Consolas, monospace;
  font-family: var(--bcsp-font-display);
  color: var(--bcsp-ink);
  background: var(--bcsp-paper);
  font-synthesis: none;
  line-height: 1.4;
}

*,
*::before,
*::after {
  box-sizing: border-box;
}

html {
  min-width: 20rem;
  background: var(--bcsp-paper);
  scroll-behavior: smooth;
}

body {
  min-width: 20rem;
  min-height: 100dvh;
  margin: 0;
  color: var(--bcsp-ink);
  background-color: var(--bcsp-paper);
}

button,
input,
select,
textarea {
  font: inherit;
}

button,
a {
  -webkit-tap-highlight-color: transparent;
}

:focus-visible {
  outline: 3px solid var(--bcsp-accent);
  outline-offset: 3px;
}

::selection {
  color: var(--bcsp-accent-ink);
  background: var(--bcsp-accent);
}

#root {
  min-height: 100dvh;
}

.bcsp-visually-hidden {
  position: absolute !important;
  width: 1px !important;
  height: 1px !important;
  padding: 0 !important;
  margin: -1px !important;
  overflow: hidden !important;
  clip: rect(0 0 0 0) !important;
  white-space: nowrap !important;
  border: 0 !important;
}

.bcsp-action {
  display: inline-grid;
  min-height: 2.75rem;
  grid-auto-flow: column;
  align-items: center;
  justify-content: center;
  gap: var(--bcsp-space-1);
  padding: 0.65rem 1rem;
  border: 1px solid var(--bcsp-line);
  border-radius: 0;
  color: var(--bcsp-ink);
  background: transparent;
  font-family: var(--bcsp-font-data);
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  line-height: 1;
  text-transform: uppercase;
  cursor: pointer;
  transition: transform 160ms cubic-bezier(0.16, 1, 0.3, 1),
    color 160ms cubic-bezier(0.16, 1, 0.3, 1),
    background-color 160ms cubic-bezier(0.16, 1, 0.3, 1);
}

.bcsp-action:hover:not(:disabled) {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}

.bcsp-action:active:not(:disabled) {
  transform: translateY(1px);
}

.bcsp-action--accent {
  color: var(--bcsp-accent-ink);
  border-color: var(--bcsp-accent);
  background: var(--bcsp-accent);
}

.bcsp-action--accent:hover:not(:disabled) {
  color: var(--bcsp-accent-ink);
  border-color: var(--bcsp-ink);
  background: var(--bcsp-ink);
}

.bcsp-action--quiet {
  border-color: transparent;
  border-bottom-color: var(--bcsp-line);
}

.bcsp-action:disabled {
  color: var(--bcsp-ink-muted);
  border-color: var(--bcsp-line-soft);
  cursor: not-allowed;
  opacity: 0.64;
}

.bcsp-state-panel {
  display: grid;
  min-height: 13rem;
  grid-template-columns: minmax(4rem, 0.24fr) minmax(0, 1fr);
  border-top: 1px solid var(--bcsp-line);
  border-bottom: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}

.bcsp-state-panel__marker {
  display: grid;
  place-items: start center;
  padding: var(--bcsp-space-4) var(--bcsp-space-2);
  border-right: 1px solid var(--bcsp-line);
  color: var(--bcsp-accent);
  font-family: var(--bcsp-font-data);
  font-size: clamp(2rem, 5vw, 4.5rem);
  font-weight: 700;
  letter-spacing: -0.08em;
  line-height: 0.8;
}

.bcsp-state-panel__body {
  display: grid;
  align-content: center;
  gap: var(--bcsp-space-2);
  padding: clamp(1.5rem, 5vw, 4rem);
}

.bcsp-state-panel__title {
  max-width: 18ch;
  margin: 0;
  font-size: clamp(1.75rem, 4.5vw, 4rem);
  font-weight: 800;
  letter-spacing: -0.045em;
  line-height: 0.95;
  text-transform: uppercase;
}

.bcsp-state-panel__detail {
  max-width: 58ch;
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: 0.98rem;
  line-height: 1.55;
}

.bcsp-state-panel__action {
  margin-top: var(--bcsp-space-2);
  justify-self: start;
}

.bcsp-state-panel--error {
  border-color: var(--bcsp-accent);
}

.bcsp-state-panel--error .bcsp-state-panel__marker {
  color: var(--bcsp-accent-ink);
  border-color: var(--bcsp-accent);
  background: var(--bcsp-accent);
}

.bcsp-state-panel--loading .bcsp-state-panel__marker {
  animation: bcsp-marker-pulse 1.4s ease-in-out infinite alternate;
}

.bcsp-status-signal {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.625rem;
  align-items: start;
  min-width: 0;
}

.bcsp-status-signal__mark {
  width: 0.75rem;
  height: 0.75rem;
  margin-top: 0.15rem;
  border: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper);
}

.bcsp-status-signal[data-state='ready'] .bcsp-status-signal__mark {
  background: var(--bcsp-ink);
}

.bcsp-status-signal[data-state='stale'] .bcsp-status-signal__mark,
.bcsp-status-signal[data-state='offline'] .bcsp-status-signal__mark {
  border-color: var(--bcsp-accent);
  background: var(--bcsp-accent);
}

.bcsp-status-signal[data-state='refreshing'] .bcsp-status-signal__mark {
  animation: bcsp-marker-pulse 1.1s ease-in-out infinite alternate;
}

.bcsp-status-signal__label,
.bcsp-status-signal__detail {
  display: block;
}

.bcsp-status-signal__label {
  font-family: var(--bcsp-font-data);
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  line-height: 1.2;
  text-transform: uppercase;
}

.bcsp-status-signal__detail {
  margin-top: 0.25rem;
  color: var(--bcsp-ink-muted);
  font-size: 0.76rem;
  line-height: 1.35;
}

.bcsp-metric {
  display: grid;
  min-width: 0;
  align-content: start;
  gap: 0.4rem;
  margin: 0;
}

.bcsp-metric__label,
.bcsp-metric__detail {
  font-family: var(--bcsp-font-data);
  font-size: 0.67rem;
  letter-spacing: 0.075em;
  line-height: 1.35;
  text-transform: uppercase;
}

.bcsp-metric__label {
  color: var(--bcsp-ink-muted);
}

.bcsp-metric__value {
  margin: 0;
  overflow-wrap: anywhere;
  font-family: var(--bcsp-font-data);
  font-size: clamp(1.35rem, 3vw, 2.35rem);
  font-variant-numeric: tabular-nums;
  font-weight: 700;
  letter-spacing: -0.055em;
  line-height: 0.95;
}

.bcsp-metric__unit {
  margin-left: 0.35em;
  font-size: 0.65em;
  letter-spacing: 0;
}

.bcsp-metric__detail {
  color: var(--bcsp-ink-muted);
  text-transform: none;
}

.bcsp-field {
  display: grid;
  gap: var(--bcsp-space-1);
}

.bcsp-field__label {
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.bcsp-field__helper,
.bcsp-field__error {
  margin: 0;
  font-size: 0.78rem;
  line-height: 1.45;
}

.bcsp-field__helper {
  color: var(--bcsp-ink-muted);
}

.bcsp-field__error {
  color: var(--bcsp-accent);
  font-weight: 700;
}

@keyframes bcsp-marker-pulse {
  from { opacity: 0.35; }
  to { opacity: 1; }
}

@media (max-width: 47.999rem) {
  .bcsp-state-panel {
    min-height: 0;
    grid-template-columns: 3.25rem minmax(0, 1fr);
  }

  .bcsp-state-panel__marker {
    padding: var(--bcsp-space-3) var(--bcsp-space-1);
    font-size: 1.75rem;
  }

  .bcsp-state-panel__body {
    padding: var(--bcsp-space-4);
  }

  .bcsp-state-panel__title {
    font-size: clamp(1.5rem, 9vw, 2.5rem);
  }
}

@media (prefers-reduced-motion: reduce) {
  html { scroll-behavior: auto; }
  *, *::before, *::after {
    animation-duration: 0.001ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: 0.001ms !important;
  }
}
`;

export function DesignSystemStyles() {
  return <style data-bcsp-design-system="">{BCSP_DESIGN_SYSTEM_CSS}</style>;
}
