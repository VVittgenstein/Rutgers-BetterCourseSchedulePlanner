export const BCSP_SHELL_CSS = String.raw`
.bcsp-shell {
  width: 100%;
  min-height: 100dvh;
  background: var(--bcsp-paper);
}

.bcsp-skip-link {
  position: fixed;
  top: 0.75rem;
  left: 0.75rem;
  z-index: 2;
  padding: 0.75rem 1rem;
  color: var(--bcsp-accent-ink);
  background: var(--bcsp-accent);
  font-family: var(--bcsp-font-data);
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  transform: translateY(-200%);
}

.bcsp-skip-link:focus {
  transform: translateY(0);
}

.bcsp-masthead {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(17rem, 0.72fr);
  border-bottom: 1px solid var(--bcsp-line);
}

.bcsp-masthead__identity {
  min-width: 0;
  padding: clamp(1rem, 3vw, 2.5rem);
}

.bcsp-masthead__eyebrow,
.bcsp-section-label,
.bcsp-utility-copy,
.bcsp-ready-plane__eyebrow {
  margin: 0;
  font-family: var(--bcsp-font-data);
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.11em;
  line-height: 1.3;
  text-transform: uppercase;
}

.bcsp-masthead__title {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 0.7rem 1rem;
  margin: 0.55rem 0 0;
}

.bcsp-masthead__mark {
  font-size: clamp(3.25rem, 8vw, 8.5rem);
  font-weight: 900;
  letter-spacing: -0.075em;
  line-height: 0.78;
  text-transform: uppercase;
}

.bcsp-masthead__name {
  width: min(24rem, 100%);
  padding-bottom: 0.25rem;
  font-size: clamp(0.82rem, 1.5vw, 1.15rem);
  font-weight: 700;
  letter-spacing: -0.02em;
  line-height: 1.05;
  text-transform: uppercase;
}

.bcsp-masthead__utility {
  display: grid;
  grid-template-rows: 1fr auto;
  min-width: 0;
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-utility-copy {
  display: grid;
  align-content: space-between;
  gap: var(--bcsp-space-4);
  min-height: 8rem;
  padding: var(--bcsp-space-4);
  color: var(--bcsp-ink-muted);
}

.bcsp-utility-copy strong {
  color: var(--bcsp-ink);
  font-size: 1.1rem;
}

.bcsp-language {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  margin: 0;
  padding: 0;
  border: 0;
  border-top: 1px solid var(--bcsp-line);
}

.bcsp-language__button {
  min-height: 2.75rem;
  padding: 0.65rem 0.75rem;
  border: 0;
  border-radius: 0;
  color: var(--bcsp-ink);
  background: transparent;
  font-family: var(--bcsp-font-data);
  font-size: 0.67rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  cursor: pointer;
  transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1));
}

.bcsp-language__button + .bcsp-language__button {
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-language__button[aria-pressed='true'] {
  color: var(--bcsp-accent-ink);
  background: var(--bcsp-ink);
}

.bcsp-language__button:active:not(:focus-visible) {
  transform: scale(0.97);
}

.bcsp-navigation {
  display: grid;
  grid-template-columns: minmax(12rem, 0.45fr) repeat(3, minmax(0, 1fr));
  min-height: 3.5rem;
  border-bottom: 1px solid var(--bcsp-line);
  font-family: var(--bcsp-font-data);
  font-size: 0.7rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.bcsp-navigation[data-extended='true'] {
  grid-template-columns: minmax(12rem, 0.45fr) repeat(6, minmax(0, 1fr));
}

.bcsp-navigation__label,
.bcsp-navigation__link {
  display: flex;
  align-items: center;
  min-height: 2.75rem;
  min-width: 0;
  padding: 0.85rem 1rem;
}

.bcsp-navigation__label {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}

.bcsp-navigation__link {
  gap: 0.7rem;
  color: var(--bcsp-ink);
  text-decoration: none;
  border-left: 1px solid var(--bcsp-line);
  transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1));
}

.bcsp-navigation__link span {
  color: var(--bcsp-ink-muted);
}

.bcsp-navigation__link[data-active='true'] {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}

.bcsp-navigation__link[data-active='true'] span {
  color: var(--bcsp-paper-raised);
}

.bcsp-navigation__link:active:not(:focus-visible) {
  transform: scale(0.97);
}

.bcsp-main {
  display: block;
  min-height: 32rem;
}

.bcsp-workspace {
  min-width: 0;
  padding: clamp(1rem, 2.25vw, 2.5rem);
}

.bcsp-workspace__heading {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  justify-content: space-between;
  gap: var(--bcsp-space-2) var(--bcsp-space-4);
  padding-bottom: var(--bcsp-space-3);
  border-bottom: 1px solid var(--bcsp-line);
}

.bcsp-workspace__identity {
  min-width: min(100%, 48rem);
}

.bcsp-workspace__title-line {
  display: flex;
  align-items: baseline;
  gap: clamp(0.75rem, 1.5vw, 1.5rem);
}

.bcsp-workspace__sequence {
  flex: 0 0 auto;
  color: var(--bcsp-accent);
  font-family: var(--bcsp-font-data);
  font-size: clamp(1.4rem, 2.6vw, 2.5rem);
  font-weight: 900;
  letter-spacing: -0.06em;
}

.bcsp-workspace__title {
  max-width: 24ch;
  margin: 0.3rem 0 0;
  font-size: clamp(1.8rem, 3.4vw, 4rem);
  font-weight: 850;
  letter-spacing: -0.055em;
  line-height: 0.9;
  text-transform: uppercase;
}

.bcsp-workspace__intro {
  max-width: 72ch;
  margin: 0.55rem 0 0;
  color: var(--bcsp-ink-muted);
  font-size: 0.82rem;
  line-height: 1.5;
}

.bcsp-workspace__protocol {
  margin: 0;
  font-family: var(--bcsp-font-data);
  font-size: 0.68rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.bcsp-state-wrap {
  margin-top: var(--bcsp-space-5);
}

.bcsp-service-status {
  display: grid;
  grid-template-columns: minmax(13rem, 1.1fr) minmax(17rem, 2fr) auto;
  align-items: stretch;
  margin: var(--bcsp-space-3) 0 0;
  border: 1px solid var(--bcsp-line);
  border-left: 0.4rem solid var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
}

.bcsp-service-status[data-level='degraded'],
.bcsp-service-status[data-level='error'],
.bcsp-service-status[data-connection='interrupted'] {
  border-left-color: var(--bcsp-accent);
}

.bcsp-service-status__lead,
.bcsp-service-status__operation,
.bcsp-service-status__counts,
.bcsp-service-status__detail {
  min-width: 0;
  padding: 0.7rem 0.85rem;
}

.bcsp-service-status__lead {
  display: flex;
  align-items: center;
  gap: 0.65rem;
}

.bcsp-service-status__signal {
  width: 0.72rem;
  height: 0.72rem;
  flex: 0 0 auto;
  border: 2px solid currentColor;
  background: var(--bcsp-ink);
}

[data-level='initializing'] .bcsp-service-status__signal,
[data-level='partially_ready'] .bcsp-service-status__signal {
  background: transparent;
}

[data-level='degraded'] .bcsp-service-status__signal,
[data-level='error'] .bcsp-service-status__signal,
[data-connection='interrupted'] .bcsp-service-status__signal {
  color: var(--bcsp-accent);
  background: var(--bcsp-accent);
}

.bcsp-service-status__kicker,
.bcsp-service-status__headline,
.bcsp-service-status__detail p {
  margin: 0;
}

.bcsp-service-status__kicker,
.bcsp-service-status__label,
.bcsp-service-status__counts dt {
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: 0.64rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.bcsp-service-status__headline {
  margin-top: 0.15rem;
  font-size: 0.86rem;
  font-weight: 850;
  line-height: 1.15;
  text-transform: uppercase;
}

.bcsp-service-status__operation {
  display: grid;
  align-content: center;
  gap: 0.15rem;
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-service-status__operation strong,
.bcsp-service-status__operation samp {
  overflow-wrap: anywhere;
}

.bcsp-service-status__operation strong {
  font-size: 0.82rem;
}

.bcsp-service-status__operation samp {
  color: var(--bcsp-ink-muted);
  font-size: 0.68rem;
}

.bcsp-service-status__counts {
  display: grid;
  grid-template-columns: repeat(2, minmax(5.5rem, 1fr));
  margin: 0;
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-service-status__counts div {
  display: grid;
  align-content: center;
}

.bcsp-service-status__counts div + div {
  padding-left: 0.85rem;
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-service-status__counts dd {
  margin: 0.12rem 0 0;
  font-family: var(--bcsp-font-data);
  font-size: 1rem;
  font-weight: 850;
  font-variant-numeric: tabular-nums;
}

.bcsp-service-status__detail {
  display: flex;
  grid-column: 1 / -1;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-3);
  border-top: 1px solid var(--bcsp-line);
  color: var(--bcsp-ink-muted);
  font-size: 0.75rem;
  line-height: 1.45;
}

.bcsp-service-status__retry {
  min-height: 2.5rem;
  padding: 0.45rem 0.8rem;
  border: 1px solid var(--bcsp-line);
  border-radius: 0;
  color: var(--bcsp-paper);
  background: var(--bcsp-ink);
  font-family: var(--bcsp-font-data);
  font-size: 0.67rem;
  font-weight: 800;
  text-transform: uppercase;
  cursor: pointer;
  transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1));
}

.bcsp-service-status__retry:active:not(:focus-visible) {
  transform: scale(0.97);
}

.bcsp-catalog-grid {
  display: grid;
  grid-template-columns: minmax(14rem, 0.38fr) minmax(0, 1fr);
  margin-top: var(--bcsp-space-5);
  border-top: 1px solid var(--bcsp-line);
  border-bottom: 1px solid var(--bcsp-line);
}

.bcsp-targets {
  border-right: 1px solid var(--bcsp-line);
}

.bcsp-targets__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-2);
  min-height: 3.25rem;
  padding: var(--bcsp-space-2) var(--bcsp-space-3);
  border-bottom: 1px solid var(--bcsp-line);
}

.bcsp-targets__count {
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
}

.bcsp-targets__list {
  display: grid;
  max-height: 24rem;
  margin: 0;
  padding: 0;
  overflow: auto;
  list-style: none;
}

.bcsp-target-button {
  display: grid;
  width: 100%;
  min-height: 4.25rem;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--bcsp-space-2);
  padding: 0.85rem var(--bcsp-space-3);
  border: 0;
  border-bottom: 1px solid var(--bcsp-line-soft);
  border-radius: 0;
  color: var(--bcsp-ink);
  background: transparent;
  text-align: left;
  cursor: pointer;
  transition: transform 140ms var(--bcsp-ease-out, cubic-bezier(0.16, 1, 0.3, 1));
}

.bcsp-target-button[aria-pressed='true'] {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}

.bcsp-target-button:active:not(:focus-visible) {
  transform: scale(0.98);
}

.bcsp-target-button__label,
.bcsp-target-button__code {
  display: block;
}

.bcsp-target-button__label {
  font-size: 0.88rem;
  font-weight: 750;
  line-height: 1.15;
  text-transform: uppercase;
}

.bcsp-target-button__code,
.bcsp-target-button__arrow {
  margin-top: 0.35rem;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: 0.65rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.bcsp-target-button[aria-pressed='true'] .bcsp-target-button__code,
.bcsp-target-button[aria-pressed='true'] .bcsp-target-button__arrow {
  color: var(--bcsp-paper-raised);
}

.bcsp-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-3);
  min-height: 2.75rem;
  padding: var(--bcsp-space-3) clamp(1rem, 2.25vw, 2.5rem);
  border-top: 1px solid var(--bcsp-line);
  font-family: var(--bcsp-font-data);
  font-size: 0.65rem;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.bcsp-footer__protocol {
  color: var(--bcsp-ink-muted);
}

@media (hover: hover) and (pointer: fine) {
  .bcsp-language__button:hover {
    color: var(--bcsp-accent-ink);
    background: var(--bcsp-accent);
  }

  .bcsp-navigation__link:hover {
    color: var(--bcsp-paper-raised);
    background: var(--bcsp-ink);
  }

  .bcsp-navigation__link:hover span {
    color: var(--bcsp-paper-raised);
  }

  .bcsp-target-button:hover {
    color: var(--bcsp-paper-raised);
    background: var(--bcsp-ink);
  }

  .bcsp-target-button:hover .bcsp-target-button__code,
  .bcsp-target-button:hover .bcsp-target-button__arrow {
    color: var(--bcsp-paper-raised);
  }
}

.bcsp-ready-plane {
  display: grid;
  min-height: 27rem;
  align-content: space-between;
  gap: var(--bcsp-space-6);
  padding: clamp(1.5rem, 5vw, 4.5rem);
  background: var(--bcsp-paper-raised);
}

.bcsp-ready-plane__title {
  max-width: 13ch;
  margin: var(--bcsp-space-2) 0 0;
  font-size: clamp(2.2rem, 6vw, 6.8rem);
  font-weight: 900;
  letter-spacing: -0.07em;
  line-height: 0.82;
  text-transform: uppercase;
}

.bcsp-ready-plane__body {
  max-width: 48ch;
  margin: var(--bcsp-space-3) 0 0;
  color: var(--bcsp-ink-muted);
  font-size: 1rem;
  line-height: 1.6;
}

.bcsp-ready-plane__target {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--bcsp-space-3);
  align-items: end;
  padding-top: var(--bcsp-space-3);
  border-top: 1px solid var(--bcsp-line);
}

.bcsp-ready-plane__target strong,
.bcsp-ready-plane__target code {
  display: block;
}

.bcsp-ready-plane__target strong {
  font-size: clamp(1.25rem, 3vw, 2.5rem);
  letter-spacing: -0.04em;
  line-height: 1;
  text-transform: uppercase;
}

.bcsp-ready-plane__target code {
  margin-top: 0.4rem;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  letter-spacing: 0.06em;
}

.bcsp-ready-plane__registration {
  color: var(--bcsp-accent);
  font-family: var(--bcsp-font-data);
  font-size: 2rem;
  font-weight: 800;
}

@media (max-width: 61.999rem) {
  .bcsp-masthead {
    grid-template-columns: minmax(0, 1fr) 16rem;
  }

  .bcsp-main {
    display: block;
  }

  .bcsp-service-status {
    grid-template-columns: minmax(12rem, 1fr) minmax(15rem, 1.5fr);
  }

  .bcsp-service-status__counts {
    grid-column: 1 / -1;
    border-top: 1px solid var(--bcsp-line);
    border-left: 0;
  }

  .bcsp-catalog-grid {
    grid-template-columns: minmax(12rem, 0.42fr) minmax(0, 1fr);
  }
}

@media (max-width: 47.999rem) {
  .bcsp-masthead,
  .bcsp-catalog-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .bcsp-masthead__utility,
  .bcsp-targets {
    border-left: 0;
    border-right: 0;
    border-top: 1px solid var(--bcsp-line);
  }

  .bcsp-utility-copy {
    min-height: 0;
    grid-template-columns: 1fr auto;
    align-items: center;
  }

  .bcsp-navigation {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .bcsp-navigation[data-extended='true'] {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .bcsp-navigation__label {
    grid-column: 1 / -1;
  }

  .bcsp-navigation__link:first-of-type {
    border-left: 0;
  }

  .bcsp-navigation__link {
    border-top: 1px solid var(--bcsp-line);
  }

  .bcsp-navigation__link:nth-of-type(3n + 1) {
    border-left: 0;
  }

  .bcsp-service-status {
    grid-template-columns: minmax(0, 1fr);
  }

  .bcsp-service-status__operation,
  .bcsp-service-status__counts {
    grid-column: 1;
    border-top: 1px solid var(--bcsp-line);
    border-left: 0;
  }

  .bcsp-targets__list {
    max-height: 16rem;
  }

  .bcsp-ready-plane {
    min-height: 22rem;
  }

  .bcsp-footer {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--bcsp-space-2) var(--bcsp-space-4);
    align-items: center;
    min-height: 2.75rem;
    padding: var(--bcsp-space-3);
    padding-bottom: max(var(--bcsp-space-3), env(safe-area-inset-bottom));
    border-top: 1px solid var(--bcsp-line);
    font-family: var(--bcsp-font-data);
    font-size: 0.65rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .bcsp-footer__protocol {
    color: var(--bcsp-ink-muted);
    text-align: right;
  }

  .bcsp-footer__copyright {
    min-width: 0;
    overflow-wrap: anywhere;
  }
}

@media (max-width: 31.999rem) {
  .bcsp-masthead__mark {
    font-size: clamp(2.8rem, 18vw, 4.5rem);
  }

  .bcsp-utility-copy {
    grid-template-columns: 1fr;
  }

  .bcsp-workspace {
    padding: var(--bcsp-space-3);
  }

  .bcsp-ready-plane__target {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (max-width: 20.999rem) {
  .bcsp-utility-copy {
    display: none;
  }

  .bcsp-navigation,
  .bcsp-navigation[data-extended='true'] {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .bcsp-navigation__label {
    grid-column: 1 / -1;
  }

  .bcsp-navigation__link:nth-of-type(3n + 1) {
    border-left: 1px solid var(--bcsp-line);
  }

  .bcsp-navigation__link:nth-of-type(odd) {
    border-left: 0;
  }

  .bcsp-footer {
    grid-template-columns: minmax(0, 1fr);
  }

  .bcsp-footer__protocol {
    text-align: left;
  }
}

@media (prefers-reduced-motion: reduce) {
  .bcsp-language__button,
  .bcsp-navigation__link,
  .bcsp-target-button,
  .bcsp-service-status__retry {
    transition: none;
  }

  .bcsp-language__button:active:not(:focus-visible),
  .bcsp-navigation__link:active:not(:focus-visible),
  .bcsp-target-button:active:not(:focus-visible),
  .bcsp-service-status__retry:active:not(:focus-visible) {
    transform: none;
  }
}
`;

export function ShellStyles() {
  return <style data-bcsp-shell-styles="">{BCSP_SHELL_CSS}</style>;
}
