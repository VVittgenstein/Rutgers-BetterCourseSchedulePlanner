/*
 * "Quiet Catalog" token layer (design spec v2, section 2).
 *
 * Rules for this string:
 *   - the only literal colours in the whole UI live in the two :root blocks below;
 *   - every radius is one of the --bcsp-radius-* tokens (or 0);
 *   - no gradients and no elevation literals here: modules reference --bcsp-elev-* instead;
 *   - the light palette is the default, the dark palette swaps under prefers-color-scheme.
 */
export const BCSP_DESIGN_SYSTEM_CSS = String.raw`
:root {
  color-scheme: light;

  /* ---- Surfaces (3 levels + selected) ---- */
  --bcsp-paper: #F6F6F4;            /* level 0: page ground */
  --bcsp-paper-raised: #FFFFFF;     /* level 1: cards, rail, inputs, popovers */
  --bcsp-surface-2: #F0F0EE;        /* level 2: hover rows, group headers, stat tiles, nested panels */
  --bcsp-surface-3: #E8E8E5;        /* pressed rows, disabled fills */
  --bcsp-surface-selected: #FFF0F3; /* selected term / checked option */
  --bcsp-surface-inverse: #1A1A1A;  /* sync pill only */
  --bcsp-scrim: rgba(26, 26, 26, 0.32);

  /* ---- Text (3 levels + placeholder) ---- */
  --bcsp-ink: #1A1A1A;              /* 16.1:1 on paper, 17.4:1 on white */
  --bcsp-ink-2: #3F3F3C;            /* secondary body, 10.6:1 on white */
  --bcsp-ink-muted: #5C5C58;        /* meta/helper, 6.2:1 on paper, 6.7:1 on white, 5.9:1 on surface-2 */
  --bcsp-ink-faint: #767671;        /* placeholders only, 4.57:1 on white */
  --bcsp-ink-inverse: #F6F6F4;
  --bcsp-muted-ink: var(--bcsp-ink-muted);   /* legacy alias (results/styles.tsx) */

  /* ---- Borders (2 levels + soft) ---- */
  --bcsp-line: #E4E4E0;             /* card outlines, dividers */
  --bcsp-line-strong: #8A8A85;      /* input/select/secondary-button boundaries: 3.47:1 on white */
  --bcsp-line-soft: #EFEFEC;        /* intra-list hairlines */

  /* ---- Accent: Rutgers scarlet ---- */
  --bcsp-accent: #CC0033;           /* FILL: buttons, active bars, checked controls; white on it 5.81:1 */
  --bcsp-accent-hover: #B3002D;
  --bcsp-accent-active: #99002A;
  --bcsp-accent-ink: #FFFFFF;       /* text on accent fill */
  --bcsp-accent-text: #B3002D;      /* scarlet AS TEXT / thin border: 7.11:1 on white */
  --bcsp-accent-tint: #FFF0F3;
  --bcsp-accent-tint-strong: #FFD6DE;

  /* ---- Semantic fg / bg / line ---- */
  --bcsp-ok: #1B6B3A;      --bcsp-ok-tint: #E8F5EC;      --bcsp-ok-line: #BFE3CC;      /* 5.83:1 on tint */
  --bcsp-warn: #8A5A00;    --bcsp-warn-tint: #FFF4DC;    --bcsp-warn-line: #F1D89A;    /* 5.43:1 */
  --bcsp-danger: #B42318;  --bcsp-danger-tint: #FDECEB;  --bcsp-danger-line: #F3A9A3;  /* 5.75:1 */
  --bcsp-info: #1D5FBF;    --bcsp-info-tint: #E9F0FB;    --bcsp-info-line: #BCD0F2;    /* 5.32:1 */

  /* ---- Focus (blue: legible on scarlet and on danger tints) ---- */
  --bcsp-focus: #1D5FBF;            /* 6.1:1 on white, 5.6:1 on paper */

  /* ---- Radius ---- */
  --bcsp-radius-1: 4px;             /* badges, small tags, checkbox box */
  --bcsp-radius-2: 6px;             /* buttons, inputs, option rows */
  --bcsp-radius-3: 10px;            /* cards, rail, toasts, popovers */
  --bcsp-radius-pill: 999px;

  /* ---- Elevation (consumed only by module CSS; the design-system string itself uses none) ---- */
  --bcsp-elev-1: 0 1px 2px rgba(26, 26, 26, 0.06);
  --bcsp-elev-2: 0 8px 24px rgba(26, 26, 26, 0.12);
  --bcsp-elev-up: 0 -6px 16px rgba(26, 26, 26, 0.08);

  /* ---- Spacing (4px base). Legacy 1..6 remapped. ---- */
  --bcsp-space-0: 0.25rem;  /* 4  */
  --bcsp-space-1: 0.5rem;   /* 8  */
  --bcsp-space-2: 0.75rem;  /* 12 */
  --bcsp-space-3: 1rem;     /* 16 */
  --bcsp-space-4: 1.5rem;   /* 24 */
  --bcsp-space-5: 2rem;     /* 32 */
  --bcsp-space-6: 3rem;     /* 48 */
  --bcsp-space-7: 4rem;     /* 64 */
  --bcsp-gutter: clamp(1rem, 2.5vw, 1.5rem);
  --bcsp-content-max: 84rem;        /* 1344 */
  --bcsp-detail-max: 55rem;         /* 880 */
  --bcsp-control-h: 2.75rem;        /* 44 — floor for every interactive element */
  --bcsp-nav-h: 3.5rem;             /* 56 app bar */
  --bcsp-navigation-height: 3.5rem; /* default; SharedApplication still publishes the measured value */

  /* ---- Fonts: Latin first, then CJK; system only ---- */
  --bcsp-font-sans: -apple-system, "Segoe UI Variable Text", "Segoe UI", system-ui, Roboto, "Helvetica Neue", Arial,
    "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans CJK SC", "Source Han Sans SC",
    "WenQuanYi Micro Hei", sans-serif;
  --bcsp-font-mono: ui-monospace, "Cascadia Mono", "Cascadia Code", Consolas, "SF Mono", Menlo, "Roboto Mono",
    "Noto Sans Mono CJK SC", monospace;
  --bcsp-font-display: var(--bcsp-font-sans);   /* legacy alias */
  --bcsp-font-data: var(--bcsp-font-mono);      /* legacy alias: identifiers only */

  /* ---- Type scale (size / line-height) ---- */
  --bcsp-text-display: 1.5rem;   --bcsp-lh-display: 2rem;      /* 24/32 page title */
  --bcsp-text-title: 1.125rem;   --bcsp-lh-title: 1.625rem;    /* 18/26 section title */
  --bcsp-text-subtitle: 0.9375rem; --bcsp-lh-subtitle: 1.375rem; /* 15/22 row/card title */
  --bcsp-text-body: 0.875rem;    --bcsp-lh-body: 1.3125rem;    /* 14/21 body, labels, buttons, inputs */
  --bcsp-text-body-lg: 0.9375rem; --bcsp-lh-body-lg: 1.5rem;   /* 15/24 intros, empty-state body */
  --bcsp-text-meta: 0.78125rem;  --bcsp-lh-meta: 1.125rem;     /* 12.5/18 meta, helper, kickers */
  --bcsp-text-micro: 0.75rem;    --bcsp-lh-micro: 1rem;        /* 12/16 badges (Latin ≤ 4 chars) */
  --bcsp-text-data: 0.8125rem;   --bcsp-lh-data: 1.125rem;     /* 13/18 mono identifiers */
  --bcsp-text-stat: 1.5rem;      --bcsp-lh-stat: 1.75rem;      /* 24/28 stat tile values */

  /* ---- Z-index ---- */
  --bcsp-z-sticky-sub: 1;   /* sub-layer sticky bars: rail group heads, page save bar */
  --bcsp-z-rail-sticky: 2;
  --bcsp-z-rail-footer: 3;
  --bcsp-z-results-head: 4;
  --bcsp-z-nav: 10;
  --bcsp-z-readiness: 15;
  --bcsp-z-popover: 20;
  --bcsp-z-skip: 25;
  --bcsp-z-toast: 30;
  --bcsp-z-sync: 40;

  /* ---- Motion ---- */
  --bcsp-ease-out: cubic-bezier(0.16, 1, 0.3, 1);
  --bcsp-dur-1: 120ms;
  --bcsp-dur-2: 180ms;

  font-family: var(--bcsp-font-sans);
  font-size: 100%;
  line-height: 1.5;
  color: var(--bcsp-ink);
  background: var(--bcsp-paper);
  font-synthesis: none;
  -webkit-font-smoothing: antialiased;
  text-rendering: optimizeLegibility;
}

@media (prefers-color-scheme: dark) {
  :root:not([data-bcsp-theme='light']) {
    color-scheme: dark;

    --bcsp-paper: #131416;
    --bcsp-paper-raised: #1B1C1F;
    --bcsp-surface-2: #232428;
    --bcsp-surface-3: #2B2C31;
    --bcsp-surface-selected: #2E1A20;
    --bcsp-surface-inverse: #ECECEA;
    --bcsp-scrim: rgba(0, 0, 0, 0.6);

    --bcsp-ink: #ECECEA;            /* 14.4:1 on raised */
    --bcsp-ink-2: #C8C8C4;          /* 10.2:1 */
    --bcsp-ink-muted: #A8A8A5;      /* 7.15:1 on raised, 6.5:1 on surface-2 */
    --bcsp-ink-faint: #9A9A97;      /* 6.0:1 */
    --bcsp-ink-inverse: #131416;

    --bcsp-line: #2C2D31;
    --bcsp-line-strong: #6E6F75;    /* 3.40:1 on raised */
    --bcsp-line-soft: #222327;

    --bcsp-accent: #E5153F;         /* fill; white on it 4.66:1 */
    --bcsp-accent-hover: #D6113A;   /* white on it 5.25:1 */
    --bcsp-accent-active: #CC0033;
    --bcsp-accent-ink: #FFFFFF;
    --bcsp-accent-text: #FF5C7A;    /* 5.73:1 on raised, 5.5:1 on tint */
    --bcsp-accent-tint: #2E1A20;
    --bcsp-accent-tint-strong: #4A2029;

    --bcsp-ok: #4CC38A;      --bcsp-ok-tint: #16281F;      --bcsp-ok-line: #245C3E;      /* 6.98:1 */
    --bcsp-warn: #E0B341;    --bcsp-warn-tint: #2B2416;    --bcsp-warn-line: #5C4A1E;    /* 7.82:1 */
    --bcsp-danger: #FF7B72;  --bcsp-danger-tint: #331B19;  --bcsp-danger-line: #5C2A27;  /* 6.35:1 */
    --bcsp-info: #6FA8FF;    --bcsp-info-tint: #17233A;    --bcsp-info-line: #2A4470;    /* 6.52:1 */

    --bcsp-focus: #7DB0FF;          /* 7.7:1 on raised */

    --bcsp-elev-1: 0 1px 2px rgba(0, 0, 0, 0.4);
    --bcsp-elev-2: 0 12px 32px rgba(0, 0, 0, 0.55);
    --bcsp-elev-up: 0 -6px 16px rgba(0, 0, 0, 0.45);
  }
}

/* Future manual toggle (not built in v2): duplicate the dark block under :root[data-bcsp-theme='dark'] */

/* ---- Global rules that live with the tokens ---- */
*,
*::before,
*::after {
  box-sizing: border-box;
}

html {
  min-width: 20rem;
  background: var(--bcsp-paper);
  scroll-behavior: auto;
}

body {
  min-width: 20rem;
  min-height: 100dvh;
  margin: 0;
  color: var(--bcsp-ink);
  background-color: var(--bcsp-paper);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  line-height: 1.5;
}

#root {
  min-height: 100dvh;
}

input,
select,
textarea,
progress,
button {
  font: inherit;
  accent-color: var(--bcsp-accent);
}

button,
a {
  -webkit-tap-highlight-color: transparent;
}

:focus-visible {
  outline: 2px solid var(--bcsp-focus);
  outline-offset: 2px;
}

::selection {
  color: var(--bcsp-ink);
  background: var(--bcsp-accent-tint-strong);
}

::placeholder {
  color: var(--bcsp-ink-faint);
  opacity: 1;
}

h1,
h2,
h3,
h4,
h5,
h6 {
  font-weight: 600;
  letter-spacing: 0;
  text-transform: none;
}

h2,
h3 {
  text-wrap: balance;
}

samp,
data,
code,
kbd {
  font-family: var(--bcsp-font-mono);
  font-variant-numeric: tabular-nums;
}

input[type='checkbox'],
input[type='radio'] {
  width: 1.125rem;
  height: 1.125rem;
  flex: none;
  margin: 0;
  accent-color: var(--bcsp-accent);
}

input[type='range'] {
  min-height: var(--bcsp-control-h);
  margin: 0;
  accent-color: var(--bcsp-accent);
}

/* ---- Locale: Simplified Chinese (scoped by the shell's data-bcsp-locale) ---- */
[data-bcsp-locale='zh-CN'] {
  line-height: 1.65;
  letter-spacing: 0;
  text-transform: none;
  word-break: normal;
  overflow-wrap: anywhere;
  line-break: strict;
}

[data-bcsp-locale='zh-CN'] :is(h1, h2, h3, h4, h5, h6) {
  line-height: 1.4;
}

[data-bcsp-locale='zh-CN'] :is(samp, data, code, kbd) {
  font-size: 0.93em;
}

/* ---- Scroll regions: scrollbar hidden until hover / focus-within, no reserved gutter (spec 11.2).
   Apply .bcsp-scroll to any element that sets its own overflow, or copy the six rules below
   onto a module class (FILTER_PANEL_CSS keeps its own literal copies for the subject list and
   the dictionary popover). Never hide a scrollbar with overflow: hidden. ---- */
.bcsp-scroll {
  scrollbar-width: none;
  scrollbar-gutter: auto;
  overscroll-behavior: contain;
}

.bcsp-scroll::-webkit-scrollbar {
  width: 0;
  height: 0;
}

.bcsp-scroll:hover,
.bcsp-scroll:focus-within {
  scrollbar-width: thin;
  scrollbar-color: var(--bcsp-line-strong) transparent;
}

.bcsp-scroll:hover::-webkit-scrollbar,
.bcsp-scroll:focus-within::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.bcsp-scroll::-webkit-scrollbar-thumb {
  border: 2px solid transparent;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-line-strong);
  background-clip: padding-box;
}

.bcsp-scroll::-webkit-scrollbar-track {
  background: transparent;
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

/* ---- Buttons (spec 4.1) ---- */
.bcsp-action {
  position: relative;
  display: inline-flex;
  min-height: var(--bcsp-control-h);
  align-items: center;
  justify-content: center;
  gap: var(--bcsp-space-1);
  padding: 0 var(--bcsp-space-3);
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.25rem;
  text-align: center;
  text-decoration: none;
  text-transform: none;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--bcsp-dur-1) var(--bcsp-ease-out),
    border-color var(--bcsp-dur-1) var(--bcsp-ease-out),
    color var(--bcsp-dur-1) var(--bcsp-ease-out),
    transform var(--bcsp-dur-1) var(--bcsp-ease-out);
}

.bcsp-action:active:not(:disabled) {
  background: var(--bcsp-surface-3);
}

.bcsp-action:active:not(:disabled):not(:focus-visible) {
  transform: translateY(1px);
}

.bcsp-action--accent {
  color: var(--bcsp-accent-ink);
  border-color: var(--bcsp-accent);
  background: var(--bcsp-accent);
}

.bcsp-action--accent:active:not(:disabled) {
  border-color: var(--bcsp-accent-active);
  background: var(--bcsp-accent-active);
}

.bcsp-action--quiet {
  color: var(--bcsp-ink-2);
  border-color: transparent;
  background: transparent;
}

.bcsp-action--quiet:active:not(:disabled) {
  color: var(--bcsp-ink);
  background: var(--bcsp-surface-3);
}

.bcsp-action--danger-outline {
  color: var(--bcsp-danger);
  border-color: var(--bcsp-danger-line);
  background: var(--bcsp-paper-raised);
}

.bcsp-action--danger-outline:active:not(:disabled) {
  background: var(--bcsp-danger-tint);
}

.bcsp-action--danger {
  color: var(--bcsp-accent-ink);
  border-color: var(--bcsp-danger);
  background: var(--bcsp-danger);
}

.bcsp-action--danger:active:not(:disabled) {
  border-color: color-mix(in srgb, var(--bcsp-danger) 80%, black);
  background: color-mix(in srgb, var(--bcsp-danger) 80%, black);
}

.bcsp-action:disabled {
  color: var(--bcsp-ink-muted);
  border-color: var(--bcsp-line);
  background: var(--bcsp-surface-3);
  cursor: not-allowed;
  opacity: 1;
}

.bcsp-action[aria-busy='true']::before {
  content: '';
  width: 0.875rem;
  height: 0.875rem;
  flex: none;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: var(--bcsp-radius-pill);
  animation: bcsp-action-busy 700ms linear infinite;
}

@keyframes bcsp-action-busy {
  to { transform: rotate(360deg); }
}

@media (hover: hover) and (pointer: fine) {
  .bcsp-action:hover:not(:disabled) {
    border-color: var(--bcsp-ink-muted);
    background: var(--bcsp-surface-2);
  }

  .bcsp-action--accent:hover:not(:disabled) {
    color: var(--bcsp-accent-ink);
    border-color: var(--bcsp-accent-hover);
    background: var(--bcsp-accent-hover);
  }

  .bcsp-action--quiet:hover:not(:disabled) {
    color: var(--bcsp-ink);
    border-color: transparent;
    background: var(--bcsp-surface-2);
  }

  .bcsp-action--danger-outline:hover:not(:disabled) {
    color: var(--bcsp-danger);
    border-color: var(--bcsp-danger-line);
    background: var(--bcsp-danger-tint);
  }

  .bcsp-action--danger:hover:not(:disabled) {
    color: var(--bcsp-accent-ink);
    border-color: color-mix(in srgb, var(--bcsp-danger) 88%, black);
    background: color-mix(in srgb, var(--bcsp-danger) 88%, black);
  }
}

[data-bcsp-locale='zh-CN'] .bcsp-action {
  min-width: 4.5rem;
  line-height: 1.3;
}

/* ---- State panels: empty / loading / error (spec 4.13) ---- */
.bcsp-state-panel {
  display: grid;
  max-width: 28rem;
  justify-items: center;
  align-content: center;
  gap: var(--bcsp-space-3);
  margin: 0 auto;
  padding: var(--bcsp-space-6) var(--bcsp-space-4);
  text-align: center;
}

.bcsp-state-panel__marker {
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

.bcsp-state-panel__body {
  display: grid;
  min-width: 0;
  justify-items: center;
  gap: var(--bcsp-space-1);
}

.bcsp-state-panel__title {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.5rem;
  text-transform: none;
  text-wrap: balance;
}

.bcsp-state-panel__detail {
  max-width: 58ch;
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body-lg);
  line-height: var(--bcsp-lh-body-lg);
}

.bcsp-state-panel__action {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--bcsp-space-3);
  margin-top: var(--bcsp-space-1);
}

.bcsp-state-panel--error {
  max-width: 32rem;
  border: 1px solid var(--bcsp-danger-line);
  border-radius: var(--bcsp-radius-3);
  background: var(--bcsp-paper-raised);
}

.bcsp-state-panel--error .bcsp-state-panel__marker {
  color: var(--bcsp-danger);
  background: var(--bcsp-danger-tint);
}

.bcsp-state-panel--error .bcsp-state-panel__title {
  font-size: 1.25rem;
  line-height: 1.75rem;
}

.bcsp-state-panel--loading .bcsp-state-panel__marker {
  color: var(--bcsp-info);
  background: var(--bcsp-info-tint);
}

/* ---- Status signal tile (spec 4.13 stat tiles, 4.7 tone map) ---- */
.bcsp-status-signal {
  display: grid;
  min-width: 0;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.625rem;
  align-items: start;
  padding: var(--bcsp-space-2) var(--bcsp-space-3);
  border-radius: var(--bcsp-radius-2);
  background: var(--bcsp-surface-2);
}

.bcsp-status-signal__mark {
  width: 0.625rem;
  height: 0.625rem;
  margin-top: 0.3125rem;
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-paper-raised);
}

.bcsp-status-signal[data-state='ready'] .bcsp-status-signal__mark {
  border-color: var(--bcsp-ok);
  background: var(--bcsp-ok);
}

.bcsp-status-signal[data-state='refreshing'] .bcsp-status-signal__mark {
  border-color: var(--bcsp-info);
  background: var(--bcsp-info);
}

.bcsp-status-signal[data-state='stale'] .bcsp-status-signal__mark {
  border-color: var(--bcsp-warn);
  background: var(--bcsp-warn);
}

.bcsp-status-signal[data-state='offline'] .bcsp-status-signal__mark {
  border-color: var(--bcsp-danger);
  background: var(--bcsp-danger);
}

.bcsp-status-signal__label,
.bcsp-status-signal__detail {
  display: block;
}

.bcsp-status-signal__label {
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.25rem;
  text-transform: none;
}

.bcsp-status-signal__detail {
  margin-top: 0.125rem;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
}

/* ---- Stat tile (spec 4.13) ---- */
.bcsp-metric {
  display: grid;
  min-width: 0;
  align-content: start;
  gap: var(--bcsp-space-0);
  margin: 0;
  padding: var(--bcsp-space-2) var(--bcsp-space-3);
  border-radius: var(--bcsp-radius-2);
  background: var(--bcsp-surface-2);
}

.bcsp-metric__label,
.bcsp-metric__detail {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
  text-transform: none;
}

.bcsp-metric__value {
  margin: 0;
  overflow-wrap: anywhere;
  color: var(--bcsp-ink);
  font-family: var(--bcsp-font-sans);
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-stat);
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-stat);
}

.bcsp-metric__unit {
  margin-left: 0.25em;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body);
  font-weight: 400;
}

/* ---- Field frame (spec 3 labels, 4.2 invalid state) ---- */
.bcsp-field {
  display: grid;
  gap: var(--bcsp-space-1);
}

.bcsp-field__label {
  color: var(--bcsp-ink);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.25rem;
  text-transform: none;
}

.bcsp-field__helper,
.bcsp-field__error {
  margin: 0;
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  line-height: var(--bcsp-lh-meta);
}

.bcsp-field__helper {
  color: var(--bcsp-ink-muted);
}

.bcsp-field__error {
  display: flex;
  align-items: flex-start;
  gap: 0.375rem;
  color: var(--bcsp-danger);
}

.bcsp-field__error::before {
  content: '!';
  display: inline-flex;
  width: 1rem;
  height: 1rem;
  flex: none;
  align-items: center;
  justify-content: center;
  margin-top: 0.0625rem;
  border: 1px solid var(--bcsp-danger-line);
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-danger-tint);
  font-size: 0.6875rem;
  font-weight: 600;
  line-height: 1;
}

[data-bcsp-locale='zh-CN'] :is(
  .bcsp-field__helper,
  .bcsp-field__error,
  .bcsp-metric__label,
  .bcsp-metric__detail,
  .bcsp-status-signal__detail
) {
  font-size: 0.8125rem;
  line-height: 1.25rem;
}

@media (max-width: 47.999rem) {
  .bcsp-state-panel {
    padding: var(--bcsp-space-5) var(--bcsp-space-3);
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

  button:active:not(:disabled):not(:focus-visible),
  a:active:not(:focus-visible),
  summary:active:not(:focus-visible) {
    transform: none !important;
  }

  .bcsp-action[aria-busy='true']::before {
    content: '…';
    width: auto;
    height: auto;
    border: 0;
    animation: none;
  }
}

@media (forced-colors: active) {
  .bcsp-action, .filter-panel__chip, .filter-panel__check, .query-scope__option, .watch-workspace__badge, .search-results__badge { border: 1px solid ButtonText; }
}
`;

export function DesignSystemStyles() {
  return <style data-bcsp-design-system="">{BCSP_DESIGN_SYSTEM_CSS}</style>;
}
