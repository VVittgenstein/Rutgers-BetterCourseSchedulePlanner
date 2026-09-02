/**
 * Shell chrome (spec 5.1 / 5.2): one 56px sticky app bar, the workspace heading and the
 * service-status card. Consumes only --bcsp-* tokens; the only runtime token it reads is
 * --bcsp-navigation-height, which SharedApplication publishes from the measured nav.
 *
 * Layout note: masthead and navigation share the same 56px band. The shell is a flex column
 * (sticky children of a flex container may travel the whole column; sticky grid items are
 * clamped to their own grid area, so the spec's fixed-row grid cannot stick). The nav sits on
 * top of the masthead through a negative top margin and lets pointer events fall through
 * everywhere except on the links; below 48rem it becomes its own 44px row under the masthead.
 */
export const BCSP_SHELL_CSS = String.raw`
.bcsp-shell {
  --bcsp-brand-w: 10rem;
  --bcsp-lang-w: 11rem;
  display: flex;
  flex-direction: column;
  width: 100%;
  min-height: 100dvh;
  color: var(--bcsp-ink);
  background: var(--bcsp-paper);
}

.bcsp-skip-link {
  position: fixed;
  top: 0.75rem;
  left: 0.75rem;
  z-index: var(--bcsp-z-skip);
  padding: 0.75rem 1rem;
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-accent-ink);
  background: var(--bcsp-accent);
  box-shadow: var(--bcsp-elev-2);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  line-height: 1.25rem;
  letter-spacing: 0;
  text-decoration: none;
  text-transform: none;
  transform: translateY(-200%);
}

.bcsp-skip-link:focus {
  transform: translateY(0);
}

/* ---- App bar: masthead (brand + language) ---- */
.bcsp-masthead {
  position: sticky;
  top: 0;
  z-index: var(--bcsp-z-nav);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-3);
  height: var(--bcsp-nav-h);
  padding: 0 var(--bcsp-gutter);
  border-bottom: 1px solid var(--bcsp-line);
  background: color-mix(in srgb, var(--bcsp-paper-raised) 94%, transparent);
  -webkit-backdrop-filter: saturate(1.2) blur(8px);
  backdrop-filter: saturate(1.2) blur(8px);
}

.bcsp-masthead__identity {
  display: flex;
  align-items: center;
  width: var(--bcsp-brand-w);
  min-width: 0;
  flex: 0 0 var(--bcsp-brand-w);
  height: 100%;
  padding: 0;
}

.bcsp-masthead__title {
  display: flex;
  align-items: baseline;
  gap: var(--bcsp-space-1);
  min-width: 0;
  margin: 0;
  overflow: hidden;
  white-space: nowrap;
  font-weight: 400;
  line-height: 1.25rem;
}

.bcsp-masthead__mark {
  flex: none;
  color: var(--bcsp-ink);
  font-size: 1rem;
  font-weight: 700;
  letter-spacing: 0;
  line-height: 1.25rem;
  text-transform: none;
}

.bcsp-masthead__name {
  display: none;
  min-width: 0;
  overflow: hidden;
  color: var(--bcsp-ink-muted);
  font-size: 0.8125rem;
  font-weight: 400;
  line-height: 1.125rem;
  letter-spacing: 0;
  text-overflow: ellipsis;
  text-transform: none;
}

.bcsp-masthead__utility {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  min-width: 0;
  flex: 0 0 auto;
  height: 100%;
}

.bcsp-language {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  min-inline-size: 0;
  margin: 0;
  padding: 2px;
  border: 0;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-surface-2);
}

.bcsp-language__button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 2.5rem;
  min-height: var(--bcsp-control-h);
  padding: 0 0.625rem;
  border: 0;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-muted);
  background: transparent;
  box-shadow: none;
  font-family: var(--bcsp-font-sans);
  font-size: 0.8125rem;
  font-weight: 600;
  line-height: 1.125rem;
  letter-spacing: 0;
  text-transform: none;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--bcsp-dur-1) var(--bcsp-ease-out),
    color var(--bcsp-dur-1) var(--bcsp-ease-out),
    box-shadow var(--bcsp-dur-1) var(--bcsp-ease-out);
}

.bcsp-language__button[aria-pressed='true'] {
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  box-shadow: var(--bcsp-elev-1);
}

.bcsp-language__button:active:not(:focus-visible) {
  transform: translateY(1px);
}

/* ---- App bar: navigation overlaid on the free middle of the masthead ---- */
.bcsp-navigation {
  position: sticky;
  top: 0;
  z-index: calc(var(--bcsp-z-nav) + 1);
  /* The router scrolls this element into view after a route change. On phones the
     nav is a second row that starts one masthead down, so without this margin the
     browser lands the document 56px in and buries the page title under the bar. */
  scroll-margin-top: var(--bcsp-nav-h);
  display: flex;
  align-items: center;
  height: var(--bcsp-nav-h);
  margin-top: calc(-1 * var(--bcsp-nav-h));
  padding: 0 calc(var(--bcsp-gutter) + var(--bcsp-lang-w)) 0 calc(var(--bcsp-gutter) + var(--bcsp-brand-w));
  background: transparent;
  pointer-events: none;
  font-family: var(--bcsp-font-sans);
}

.bcsp-navigation__link {
  display: inline-flex;
  align-items: center;
  min-width: 0;
  height: var(--bcsp-nav-h);
  margin-right: var(--bcsp-space-4);
  padding: 0 0.25rem;
  border-bottom: 2px solid transparent;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  line-height: 1.25rem;
  letter-spacing: 0;
  text-decoration: none;
  text-transform: none;
  white-space: nowrap;
  pointer-events: auto;
  transition:
    color var(--bcsp-dur-1) var(--bcsp-ease-out),
    border-color var(--bcsp-dur-2) var(--bcsp-ease-out);
}

.bcsp-navigation__link:last-child {
  margin-right: 0;
}

.bcsp-navigation__link span {
  display: none;
}

.bcsp-navigation__link[data-active='true'] {
  color: var(--bcsp-ink);
  border-bottom-color: var(--bcsp-accent-text);
}

.bcsp-navigation__link:active:not(:focus-visible) {
  transform: translateY(1px);
}

/* ---- Workspace ---- */
/* #bcsp-workspace is the element the skip link and the router's post-navigation
   focus actually scroll to, so it -- not only the section inside it -- has to
   clear the sticky app bar, which is two rows tall on phones. Without this the
   page title lands behind the navigation after every route change. */
.bcsp-main {
  display: block;
  flex: 1 0 auto;
  min-height: 32rem;
  scroll-margin-top: var(--bcsp-navigation-height, 3.5rem);
}

.bcsp-workspace {
  max-width: var(--bcsp-content-max);
  min-width: 0;
  margin: 0 auto;
  padding: var(--bcsp-space-4) var(--bcsp-gutter) var(--bcsp-space-6);
  scroll-margin-top: var(--bcsp-navigation-height, 3.5rem);
}

.bcsp-workspace__heading {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(18rem, 24rem);
  align-items: start;
  gap: var(--bcsp-space-4);
  padding-bottom: var(--bcsp-space-2);
}

.bcsp-workspace__identity {
  min-width: 0;
}

.bcsp-workspace__title {
  margin: 0;
  color: var(--bcsp-ink);
  font-size: var(--bcsp-text-display);
  font-weight: 600;
  letter-spacing: -0.01em;
  line-height: var(--bcsp-lh-display);
  text-transform: none;
  text-wrap: balance;
}

.bcsp-workspace__intro {
  max-width: 64ch;
  margin: var(--bcsp-space-1) 0 0;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body-lg);
  line-height: var(--bcsp-lh-body-lg);
}

.bcsp-workspace__status-slot {
  display: grid;
  min-width: 0;
  gap: var(--bcsp-space-2);
}

.bcsp-state-wrap {
  margin-top: var(--bcsp-space-5);
}

/* ---- Service status card (spec 5.2; DOM unchanged) ---- */
.bcsp-service-status {
  display: grid;
  gap: var(--bcsp-space-0);
  min-width: 0;
  margin: 0;
  padding: 0.625rem 0.875rem;
  border: 1px solid var(--bcsp-line);
  border-radius: var(--bcsp-radius-3);
  background: var(--bcsp-paper-raised);
  box-shadow: var(--bcsp-elev-1);
}

/*
 * Healthy and complete is the state this card is in almost always, and a five-row
 * card beside a two-line page title leaves a band of nothing above the content.
 * So the settled card folds into two content rows under the headline: activity
 * beside the progress bar, counts beside the diagnostics disclosure. Nothing is
 * hidden -- the card only stacks back out (data-expanded) while something is
 * loading, retrying, degraded or interrupted, which is when the height is earned.
 */
.bcsp-service-status:not([data-expanded]) {
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  column-gap: var(--bcsp-space-2);
}

.bcsp-service-status:not([data-expanded]) .bcsp-service-status__lead {
  grid-area: 1 / 1 / 2 / 3;
}

.bcsp-service-status:not([data-expanded]) .bcsp-service-status__operation {
  grid-area: 2 / 1 / 3 / 2;
}

.bcsp-service-status:not([data-expanded]) .bcsp-service-status__progress {
  grid-area: 2 / 2 / 3 / 3;
  width: 7rem;
  padding-left: 0;
}

.bcsp-service-status:not([data-expanded]) .bcsp-service-status__counts {
  grid-area: 3 / 1 / 4 / 2;
}

.bcsp-service-status:not([data-expanded]) .bcsp-service-status__detail {
  grid-area: 3 / 2 / 4 / 3;
  margin: 0 -0.375rem;
}

.bcsp-service-status:not([data-expanded]) .bcsp-service-status__detail summary {
  padding-left: 0.375rem;
}

/* Opened, the diagnostics need the whole card width, so they drop to their own row. */
.bcsp-service-status:not([data-expanded]) .bcsp-service-status__detail[open] {
  grid-area: 4 / 1 / 5 / 3;
}

.bcsp-service-status[data-level='degraded'],
.bcsp-service-status[data-level='error'],
.bcsp-service-status[data-connection='interrupted'] {
  border-color: var(--bcsp-danger-line);
  background: var(--bcsp-danger-tint);
}

.bcsp-service-status__lead {
  display: flex;
  align-items: flex-start;
  gap: 0.625rem;
  min-width: 0;
}

.bcsp-service-status__lead > div {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0 var(--bcsp-space-1);
  min-width: 0;
}

.bcsp-service-status__signal {
  width: 0.625rem;
  height: 0.625rem;
  flex: 0 0 auto;
  margin-top: 0.375rem;
  border: 1px solid var(--bcsp-ok);
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-ok);
}

[data-level='initializing'] .bcsp-service-status__signal,
[data-level='partially_ready'] .bcsp-service-status__signal {
  border-color: var(--bcsp-warn);
  background: var(--bcsp-warn);
  animation: bcsp-pulse 2s ease-in-out infinite;
}

[data-level='degraded'] .bcsp-service-status__signal,
[data-level='error'] .bcsp-service-status__signal,
[data-connection='interrupted'] .bcsp-service-status__signal {
  border-color: var(--bcsp-danger);
  background: var(--bcsp-danger);
  animation: none;
}

.bcsp-service-status__signal[data-loading='true'] {
  border: 2px solid var(--bcsp-ink-muted);
  border-top-color: transparent;
  background: transparent;
  animation: bcsp-status-turn 900ms steps(8, end) infinite;
}

@keyframes bcsp-status-turn {
  to { transform: rotate(1turn); }
}

@keyframes bcsp-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.35; }
}

.bcsp-service-status__kicker,
.bcsp-service-status__headline,
.bcsp-service-status__detail p {
  margin: 0;
}

.bcsp-service-status__kicker,
.bcsp-service-status__label {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  line-height: var(--bcsp-lh-meta);
  letter-spacing: 0;
  text-transform: none;
}

.bcsp-service-status__headline {
  min-width: 0;
  color: var(--bcsp-ink);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  line-height: 1.25rem;
  letter-spacing: 0;
  text-transform: none;
  overflow-wrap: anywhere;
}

.bcsp-service-status__operation {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0 var(--bcsp-space-1);
  min-width: 0;
  padding-left: 1.25rem;
}

.bcsp-service-status__operation strong,
.bcsp-service-status__operation samp {
  overflow-wrap: anywhere;
}

.bcsp-service-status__operation strong {
  color: var(--bcsp-ink-2);
  font-size: 0.8125rem;
  font-weight: 400;
  line-height: 1.125rem;
}

.bcsp-service-status__operation samp {
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-data);
}

.bcsp-service-status__progress {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--bcsp-space-2);
  min-width: 0;
  padding-left: 1.25rem;
}

.bcsp-service-status__progress progress {
  display: block;
  width: 100%;
  height: 0.25rem;
  border: 0;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-2);
  background: var(--bcsp-line-soft);
  accent-color: var(--bcsp-accent);
  appearance: none;
  overflow: hidden;
}

.bcsp-service-status__progress progress::-webkit-progress-bar {
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-line-soft);
}

.bcsp-service-status__progress progress::-webkit-progress-value {
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-ink-2);
}

.bcsp-service-status__progress progress::-moz-progress-bar {
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-ink-2);
}

[data-level='ready'] .bcsp-service-status__progress progress { color: var(--bcsp-ok); }
[data-level='ready'] .bcsp-service-status__progress progress::-webkit-progress-value { background: var(--bcsp-ok); }
[data-level='ready'] .bcsp-service-status__progress progress::-moz-progress-bar { background: var(--bcsp-ok); }

[data-level='degraded'] .bcsp-service-status__progress progress,
[data-level='error'] .bcsp-service-status__progress progress,
[data-connection='interrupted'] .bcsp-service-status__progress progress { color: var(--bcsp-danger); }
[data-level='degraded'] .bcsp-service-status__progress progress::-webkit-progress-value,
[data-level='error'] .bcsp-service-status__progress progress::-webkit-progress-value,
[data-connection='interrupted'] .bcsp-service-status__progress progress::-webkit-progress-value { background: var(--bcsp-danger); }
[data-level='degraded'] .bcsp-service-status__progress progress::-moz-progress-bar,
[data-level='error'] .bcsp-service-status__progress progress::-moz-progress-bar,
[data-connection='interrupted'] .bcsp-service-status__progress progress::-moz-progress-bar { background: var(--bcsp-danger); }

.bcsp-service-status__progress span {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-mono);
  font-size: var(--bcsp-text-micro);
  font-weight: 400;
  font-variant-numeric: tabular-nums;
  line-height: var(--bcsp-lh-micro);
  white-space: nowrap;
}

/*
 * Counts read as one quiet line ("92026 3/3 - 02027 3/3", spec 5.2 row 4) rather than
 * as tiles: a tile grid here both cost a row of height and risked the empty trailing
 * cell of spec 11.1. With no cell chrome there is no track left to look empty.
 */
.bcsp-service-status__counts {
  display: flex;
  flex-wrap: wrap;
  gap: 0 var(--bcsp-space-2);
  min-width: 0;
  margin: 0;
  padding-left: 1.25rem;
}

.bcsp-service-status__counts div {
  display: flex;
  flex: 0 1 auto;
  align-items: baseline;
  gap: 0.375rem;
  min-width: 0;
}

.bcsp-service-status__counts dt {
  min-width: 0;
  overflow: hidden;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  line-height: var(--bcsp-lh-meta);
  letter-spacing: 0;
  text-overflow: ellipsis;
  text-transform: none;
  white-space: nowrap;
}

.bcsp-service-status__counts dt samp {
  color: inherit;
  font-size: var(--bcsp-text-data);
}

.bcsp-service-status__counts dd {
  margin: 0;
  color: var(--bcsp-ink);
  font-family: var(--bcsp-font-mono);
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-data);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  line-height: var(--bcsp-lh-data);
  white-space: nowrap;
}

.bcsp-service-status__detail {
  min-width: 0;
  margin: 0 -0.375rem -0.25rem;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
}

.bcsp-service-status__detail summary {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-1);
  min-height: 2rem;
  padding: 0 0.375rem 0 calc(1.25rem + 0.375rem);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink-2);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  line-height: var(--bcsp-lh-meta);
  letter-spacing: 0;
  text-decoration: underline;
  text-decoration-color: var(--bcsp-line-strong);
  text-underline-offset: 0.15em;
  text-transform: none;
  list-style: none;
  cursor: pointer;
}

/* The row is 32px so the card stays short; the pointer target is still 44px (spec 4.1). */
.bcsp-service-status__detail summary::before {
  content: '';
  position: absolute;
  inset: -0.375rem 0;
}

.bcsp-service-status__detail summary::-webkit-details-marker {
  display: none;
}

.bcsp-service-status__detail summary::after {
  content: '';
  flex: none;
  width: 0.375rem;
  height: 0.375rem;
  margin-right: 0.25rem;
  border-right: 1.5px solid currentColor;
  border-bottom: 1.5px solid currentColor;
  transform: rotate(-45deg);
  transition: transform var(--bcsp-dur-2) var(--bcsp-ease-out);
}

.bcsp-service-status__detail[open] summary::after {
  transform: rotate(45deg);
}

.bcsp-service-status__diagnostics {
  display: grid;
  gap: 0.375rem;
  margin: 0.25rem 0.375rem 0.375rem;
  padding: var(--bcsp-space-2);
  border-radius: var(--bcsp-radius-2);
  background: var(--bcsp-surface-2);
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
}

.bcsp-service-status[data-level='degraded'] .bcsp-service-status__diagnostics,
.bcsp-service-status[data-level='error'] .bcsp-service-status__diagnostics,
.bcsp-service-status[data-connection='interrupted'] .bcsp-service-status__diagnostics {
  background: var(--bcsp-paper-raised);
}

.bcsp-service-status__diagnostics p {
  color: var(--bcsp-ink-2);
}

.bcsp-service-status__diagnostics samp {
  font-size: var(--bcsp-text-data);
  line-height: var(--bcsp-lh-data);
  overflow-wrap: anywhere;
}

.bcsp-service-status__retry {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  justify-self: start;
  gap: var(--bcsp-space-1);
  min-height: var(--bcsp-control-h);
  margin-top: var(--bcsp-space-1);
  padding: 0 var(--bcsp-space-3);
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  line-height: 1.25rem;
  letter-spacing: 0;
  text-transform: none;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--bcsp-dur-1) var(--bcsp-ease-out),
    border-color var(--bcsp-dur-1) var(--bcsp-ease-out),
    color var(--bcsp-dur-1) var(--bcsp-ease-out);
}

.bcsp-service-status__retry:active:not(:focus-visible) {
  background: var(--bcsp-surface-3);
  transform: translateY(1px);
}

/* ---- Footer ---- */
.bcsp-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-3);
  min-height: 2.75rem;
  padding: var(--bcsp-space-4) var(--bcsp-gutter);
  border-top: 1px solid var(--bcsp-line);
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
  letter-spacing: 0;
  text-transform: none;
}

.bcsp-footer__copyright {
  min-width: 0;
  overflow-wrap: anywhere;
}

/* ---- Hover (fine pointers only) ---- */
@media (hover: hover) and (pointer: fine) {
  .bcsp-language__button:hover {
    color: var(--bcsp-ink);
  }

  .bcsp-navigation__link:hover {
    color: var(--bcsp-ink);
  }

  .bcsp-service-status__detail summary:hover {
    background: var(--bcsp-surface-2);
    color: var(--bcsp-ink);
  }

  .bcsp-service-status__retry:hover {
    border-color: var(--bcsp-ink-muted);
    background: var(--bcsp-surface-2);
  }
}

/* ---- Locale: zh-CN (spec 3) ---- */
.bcsp-shell[data-bcsp-locale='zh-CN'] .bcsp-workspace__title {
  letter-spacing: 0;
  line-height: 2.125rem;
}

.bcsp-shell[data-bcsp-locale='zh-CN'] .bcsp-workspace__intro {
  line-height: 1.625rem;
}

.bcsp-shell[data-bcsp-locale='zh-CN'] :is(
  .bcsp-masthead__name,
  .bcsp-service-status__kicker,
  .bcsp-service-status__label,
  .bcsp-service-status__counts dt,
  .bcsp-service-status__detail,
  .bcsp-service-status__detail summary,
  .bcsp-service-status__diagnostics,
  .bcsp-footer
) {
  font-size: 0.8125rem;
  line-height: 1.25rem;
}

.bcsp-shell[data-bcsp-locale='zh-CN'] .bcsp-service-status__operation strong {
  font-size: var(--bcsp-text-body);
  line-height: 1.375rem;
}

/* ---- Responsive ---- */
@media (min-width: 75rem) {
  .bcsp-shell {
    --bcsp-brand-w: 22rem;
  }

  .bcsp-masthead__name {
    display: block;
  }
}

@media (max-width: 63.999rem) {
  .bcsp-shell {
    --bcsp-brand-w: 6rem;
  }

  .bcsp-navigation__link {
    margin-right: var(--bcsp-space-3);
  }
}

@media (max-width: 61.999rem) {
  .bcsp-workspace__heading {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (max-width: 47.999rem) {
  /* The overlay is dropped: the nav becomes a 44px row under the 56px masthead and both stick. */
  /* Spec 11.1: flex-wrap with growing links, so the last row fills and never
     leaves an empty track (5 local links would orphan a column in a 3-up grid). */
  .bcsp-navigation {
    top: var(--bcsp-nav-h);
    display: flex;
    flex-wrap: wrap;
    height: auto;
    min-height: var(--bcsp-control-h);
    margin-top: 0;
    padding: 0 var(--bcsp-gutter);
    border-bottom: 1px solid var(--bcsp-line);
    background: var(--bcsp-paper-raised);
    pointer-events: auto;
  }

  .bcsp-navigation__link,
  .bcsp-navigation__link:last-child {
    flex: 1 1 7rem;
    justify-content: center;
    height: var(--bcsp-control-h);
    margin-right: 0;
    padding: 0 0.5rem;
  }

  .bcsp-workspace {
    padding: var(--bcsp-space-3) var(--bcsp-gutter) var(--bcsp-space-5);
  }

  .bcsp-workspace__heading {
    gap: var(--bcsp-space-3);
  }

  .bcsp-footer {
    flex-wrap: wrap;
    padding: var(--bcsp-space-3) var(--bcsp-gutter);
    padding-bottom: max(var(--bcsp-space-3), env(safe-area-inset-bottom));
  }
}

@media (max-width: 20.999rem) {
  .bcsp-navigation__link,
  .bcsp-navigation__link:last-child {
    flex-basis: 6rem;
  }

  .bcsp-language__button {
    padding: 0 0.5rem;
  }
}

/* ---- Forced colours ---- */
@media (forced-colors: active) {
  .bcsp-navigation__link[data-active='true'] {
    border-bottom-color: Highlight;
  }

  .bcsp-language__button[aria-pressed='true'],
  .bcsp-service-status__retry {
    border: 1px solid ButtonText;
  }

  .bcsp-service-status__signal {
    border-color: CanvasText;
  }
}

/* ---- Reduced motion (spec 9): no transitions, no transforms, no spin / pulse ---- */
@media (prefers-reduced-motion: reduce) {
  .bcsp-language__button,
  .bcsp-navigation__link,
  .bcsp-service-status__retry,
  .bcsp-service-status__detail summary::after {
    transition: none;
  }

  .bcsp-language__button:active:not(:focus-visible),
  .bcsp-navigation__link:active:not(:focus-visible),
  .bcsp-service-status__retry:active:not(:focus-visible) {
    transform: none;
  }

  .bcsp-service-status .bcsp-service-status__signal,
  .bcsp-service-status__signal[data-loading='true'] {
    animation: none;
  }
}
`;

export function ShellStyles() {
  return <style data-bcsp-shell-styles="">{BCSP_SHELL_CSS}</style>;
}
