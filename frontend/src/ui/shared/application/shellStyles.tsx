export const BCSP_SHELL_CSS = String.raw`
.bcsp-shell {
  width: min(100%, 100rem);
  min-height: 100dvh;
  margin: 0 auto;
  border-right: 1px solid var(--bcsp-line);
  border-left: 1px solid var(--bcsp-line);
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
  transition: transform 160ms cubic-bezier(0.16, 1, 0.3, 1);
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
  transition: color 160ms cubic-bezier(0.16, 1, 0.3, 1),
    background-color 160ms cubic-bezier(0.16, 1, 0.3, 1),
    transform 160ms cubic-bezier(0.16, 1, 0.3, 1);
}

.bcsp-language__button + .bcsp-language__button {
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-language__button[aria-pressed='true'] {
  color: var(--bcsp-accent-ink);
  background: var(--bcsp-ink);
}

.bcsp-language__button:hover {
  color: var(--bcsp-accent-ink);
  background: var(--bcsp-accent);
}

.bcsp-language__button:active {
  transform: translateY(1px);
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

.bcsp-navigation__label,
.bcsp-navigation__link {
  display: flex;
  align-items: center;
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
  transition: color 160ms cubic-bezier(0.16, 1, 0.3, 1),
    background-color 160ms cubic-bezier(0.16, 1, 0.3, 1);
}

.bcsp-navigation__link span {
  color: var(--bcsp-accent);
}

.bcsp-navigation__link:hover {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}

.bcsp-navigation__link[data-active='true'] {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}

.bcsp-navigation__link:hover span {
  color: var(--bcsp-paper-raised);
}

.bcsp-navigation__link[data-active='true'] span {
  color: var(--bcsp-accent);
}

.bcsp-main {
  display: grid;
  grid-template-columns: minmax(13rem, 0.28fr) minmax(0, 1fr);
  min-height: 32rem;
}

.bcsp-rail {
  display: grid;
  align-content: space-between;
  min-width: 0;
  border-right: 1px solid var(--bcsp-line);
}

.bcsp-rail__section {
  display: grid;
  gap: var(--bcsp-space-4);
  padding: var(--bcsp-space-4);
}

.bcsp-rail__sequence {
  margin: 0;
  font-size: clamp(3rem, 7vw, 6.5rem);
  font-weight: 900;
  letter-spacing: -0.08em;
  line-height: 0.76;
}

.bcsp-rail__note {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: 0.82rem;
  line-height: 1.55;
}

.bcsp-rail__footer {
  padding: var(--bcsp-space-3) var(--bcsp-space-4);
  border-top: 1px solid var(--bcsp-line);
  font-family: var(--bcsp-font-data);
  font-size: 0.65rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.bcsp-workspace {
  min-width: 0;
  padding: clamp(1rem, 3vw, 2.5rem);
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

.bcsp-workspace__title {
  max-width: 18ch;
  margin: 0.3rem 0 0;
  font-size: clamp(1.8rem, 4.4vw, 4.75rem);
  font-weight: 850;
  letter-spacing: -0.055em;
  line-height: 0.9;
  text-transform: uppercase;
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

.bcsp-status-grid {
  display: grid;
  grid-template-columns: 1.15fr repeat(3, minmax(0, 1fr));
  margin: var(--bcsp-space-5) 0 0;
  border-top: 1px solid var(--bcsp-line);
  border-left: 1px solid var(--bcsp-line);
}

.bcsp-status-grid > * {
  min-height: 7.5rem;
  padding: var(--bcsp-space-3);
  border-right: 1px solid var(--bcsp-line);
  border-bottom: 1px solid var(--bcsp-line);
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
  transition: color 160ms cubic-bezier(0.16, 1, 0.3, 1),
    background-color 160ms cubic-bezier(0.16, 1, 0.3, 1),
    transform 160ms cubic-bezier(0.16, 1, 0.3, 1);
}

.bcsp-target-button:hover,
.bcsp-target-button[aria-pressed='true'] {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}

.bcsp-target-button:active {
  transform: translateY(1px);
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

.bcsp-target-button:hover .bcsp-target-button__code,
.bcsp-target-button:hover .bcsp-target-button__arrow,
.bcsp-target-button[aria-pressed='true'] .bcsp-target-button__code,
.bcsp-target-button[aria-pressed='true'] .bcsp-target-button__arrow {
  color: var(--bcsp-paper-raised);
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
    grid-template-columns: 10rem minmax(0, 1fr);
  }

  .bcsp-status-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .bcsp-catalog-grid {
    grid-template-columns: minmax(12rem, 0.42fr) minmax(0, 1fr);
  }
}

@media (max-width: 47.999rem) {
  .bcsp-shell {
    border-right: 0;
    border-left: 0;
  }

  .bcsp-masthead,
  .bcsp-main,
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

  .bcsp-navigation__label {
    grid-column: 1 / -1;
  }

  .bcsp-navigation__link:first-of-type {
    border-left: 0;
  }

  .bcsp-rail {
    grid-template-columns: minmax(0, 1fr);
    border-right: 0;
    border-bottom: 1px solid var(--bcsp-line);
  }

  .bcsp-rail__section {
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    padding: var(--bcsp-space-3);
  }

  .bcsp-rail__sequence {
    font-size: 3rem;
  }

  .bcsp-rail__footer {
    border-top: 1px solid var(--bcsp-line);
    border-left: 0;
  }

  .bcsp-status-grid {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  }

  .bcsp-targets__list {
    max-height: 16rem;
  }

  .bcsp-ready-plane {
    min-height: 22rem;
  }
}

@media (max-width: 31.999rem) {
  .bcsp-navigation {
    grid-template-columns: 1fr;
  }

  .bcsp-navigation__label {
    grid-column: auto;
  }

  .bcsp-navigation__link {
    border-top: 1px solid var(--bcsp-line);
    border-left: 0;
  }

  .bcsp-masthead__mark {
    font-size: clamp(2.8rem, 18vw, 4.5rem);
  }

  .bcsp-utility-copy {
    grid-template-columns: 1fr;
  }

  .bcsp-workspace {
    padding: var(--bcsp-space-3);
  }

  .bcsp-status-grid {
    grid-template-columns: minmax(0, 1fr);
  }

  .bcsp-ready-plane__target {
    grid-template-columns: minmax(0, 1fr);
  }
}
`;

export function ShellStyles() {
  return <style data-bcsp-shell-styles="">{BCSP_SHELL_CSS}</style>;
}
