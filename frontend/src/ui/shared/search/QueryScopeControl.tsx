import { useMemo, type ReactNode } from 'react';

import { useBcspI18n, type BcspI18nRuntime } from '../i18n/runtime';
import {
  isServiceStatusV2,
  type CatalogDiscoveryResponseV1,
  type CatalogFieldKnowledge,
  type ServiceStatus,
  type ServiceTargetStatusV2,
  type ServiceVisibleTermV2,
} from '../product';
import type { SearchScope } from './SearchSession';

const ONLINE_ALIASES = new Set(['ONLINE_NB', 'ONLINE_NK', 'ONLINE_CM']);

const QUERY_SCOPE_CSS = `
.query-scope {
  display: grid;
  border-bottom: 3px solid var(--bcsp-line);
  background: var(--bcsp-paper);
}
.query-scope__head {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--bcsp-space-3);
  align-items: end;
  padding: var(--bcsp-space-4);
  border-bottom: 1px solid var(--bcsp-line);
}
.query-scope__kicker,
.query-scope__status,
.query-scope__term-meta,
.query-scope__campus-state {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: 0.7rem;
  letter-spacing: 0.05em;
}
.query-scope__title { margin: 0.25rem 0 0; font-size: 1.45rem; line-height: 1; }
.query-scope__intro { max-width: 58ch; margin: 0.45rem 0 0; color: var(--bcsp-ink-muted); }
.query-scope__action { display: grid; min-width: 9rem; gap: 0.35rem; justify-items: stretch; }
.query-scope__action .bcsp-action { transition: transform 140ms cubic-bezier(0.23, 1, 0.32, 1); }
.query-scope__action .bcsp-action:active:not(:disabled) { transform: scale(0.97); }
.query-scope__error { margin: 0; color: var(--bcsp-danger, #8f1d14); font-size: 0.72rem; }
.query-scope__groups { display: grid; grid-template-columns: minmax(16rem, 0.85fr) minmax(0, 1.15fr); }
.query-scope__group { min-width: 0; padding: var(--bcsp-space-4); margin: 0; border: 0; }
.query-scope__group + .query-scope__group { border-left: 1px solid var(--bcsp-line); }
.query-scope__legend { width: 100%; padding: 0 0 var(--bcsp-space-3); font-weight: 800; }
.query-scope__terms,
.query-scope__campuses { display: grid; grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr)); border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.query-scope__option {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.65rem;
  align-items: start;
  min-height: 4.25rem;
  padding: 0.7rem;
  border-right: 1px solid var(--bcsp-line);
  border-bottom: 1px solid var(--bcsp-line);
  cursor: pointer;
}
.query-scope__option input { width: 1rem; height: 1rem; margin: 0.15rem 0 0; accent-color: var(--bcsp-accent); }
.query-scope__option:has(input:focus-visible) { z-index: 1; outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: -3px; }
.query-scope__option[data-unavailable='true'] { color: var(--bcsp-ink-muted); background: var(--bcsp-paper-raised); }
.query-scope__option:has(input:disabled) { cursor: not-allowed; }
.query-scope__option-copy { display: grid; gap: 0.25rem; min-width: 0; }
.query-scope__option-copy strong { overflow-wrap: anywhere; }
.query-scope__campus-state[data-state='ready'] { color: var(--bcsp-ink); }
.query-scope__campus-state[data-state='retry'] { color: var(--bcsp-danger, #8f1d14); }
.query-scope__loading-mark {
  display: inline-block;
  width: 0.7rem;
  height: 0.7rem;
  margin-right: 0.35rem;
  border: 2px solid currentColor;
  border-top-color: transparent;
  animation: query-scope-turn 900ms steps(8, end) infinite;
  vertical-align: -0.05rem;
}
@keyframes query-scope-turn { to { transform: rotate(1turn); } }
@media (hover: hover) and (pointer: fine) {
  .query-scope__option:hover:not(:has(input:disabled)) { background: var(--bcsp-paper-raised); }
}
@media (max-width: 47.999rem) {
  .query-scope__head, .query-scope__groups { grid-template-columns: minmax(0, 1fr); }
  .query-scope__action { min-width: 0; }
  .query-scope__group + .query-scope__group { border-top: 1px solid var(--bcsp-line); border-left: 0; }
}
@media (prefers-reduced-motion: reduce) {
  .query-scope__action .bcsp-action { transition: none; }
  .query-scope__action .bcsp-action:active:not(:disabled) { transform: none; }
  .query-scope__loading-mark { animation: none; border-top-color: currentColor; }
}
`;

function knownText(value: CatalogFieldKnowledge<string>): string | null {
  if (value.knowledge !== 'KNOWN' || value.presence.presence !== 'PRESENT') return null;
  return value.presence.value;
}

function fallbackTermLabel(term: string, i18n: BcspI18nRuntime): string {
  const match = /^([0179])(\d{4})$/u.exec(term);
  const season = match?.[1];
  const year = match?.[2];
  if (season === undefined || year === undefined) return term;
  const key = season === '0' ? 'scope.term_winter'
    : season === '1' ? 'scope.term_spring'
      : season === '7' ? 'scope.term_summer'
        : 'scope.term_fall';
  return i18n.t(key, { year });
}

function sameScope(left: SearchScope | null, right: SearchScope): boolean {
  if (left === null || left.term !== right.term) return false;
  const leftCampuses = [...left.campuses].sort();
  const rightCampuses = [...right.campuses].sort();
  return leftCampuses.length === rightCampuses.length
    && leftCampuses.every((campus, index) => campus === rightCampuses[index]);
}

function visibleTerms(
  status: ServiceStatus | null,
  _discovery: CatalogDiscoveryResponseV1,
): readonly ServiceVisibleTermV2[] {
  if (status !== null && isServiceStatusV2(status)) return status.termWindow.visibleTerms;
  return [];
}

function termPosition(offset: number): 'scope.current' | 'scope.next' | 'scope.previous' | 'scope.future' {
  if (offset === 0) return 'scope.current';
  if (offset === 1) return 'scope.next';
  return offset < 0 ? 'scope.previous' : 'scope.future';
}

function targetStatus(
  status: ServiceStatus | null,
  term: string,
  campus: string,
): ServiceTargetStatusV2 | null {
  if (status === null || !isServiceStatusV2(status)) return null;
  return status.targets.find((target) =>
    target.target.term === term && target.target.campus === campus) ?? null;
}

export interface QueryScopeControlProps {
  readonly actionPending?: boolean | undefined;
  readonly applied: SearchScope | null;
  readonly candidate: SearchScope;
  readonly discovery: CatalogDiscoveryResponseV1;
  readonly onApply: (scope: SearchScope) => void;
  readonly onCandidateChange: (scope: SearchScope) => void;
  readonly renderUnavailableAction?: QueryScopeUnavailableActionRenderer | undefined;
  readonly status: ServiceStatus | null;
}

export interface QueryScopeUnavailableActionContext {
  readonly actionPending: boolean;
  readonly status: ServiceStatus | null;
  readonly term: ServiceVisibleTermV2 | null;
}

export type QueryScopeUnavailableActionRenderer = (
  context: QueryScopeUnavailableActionContext,
) => ReactNode;

export function QueryScopeControl({
  actionPending = false,
  applied,
  candidate,
  discovery,
  onApply,
  onCandidateChange,
  renderUnavailableAction,
  status,
}: QueryScopeControlProps) {
  const i18n = useBcspI18n();
  const terms = useMemo(() => visibleTerms(status, discovery), [discovery, status]);
  const labels = useMemo(() => {
    const values = new Map<string, string>();
    for (const target of discovery.targets) {
      if (!values.has(target.key.term)) {
        values.set(
          target.key.term,
          knownText(target.termLabel) ?? fallbackTermLabel(target.key.term, i18n),
        );
      }
    }
    return values;
  }, [discovery.targets, i18n]);
  const selectedTerm = terms.find(({ term }) => term === candidate.term) ?? null;
  const campusLabels = useMemo(() => {
    const values = new Map<string, string>();
    for (const target of discovery.targets) {
      if (target.key.term !== candidate.term || ONLINE_ALIASES.has(target.key.campus)) continue;
      values.set(target.key.campus, knownText(target.campusLabel) ?? target.key.campus);
    }
    if (status !== null && isServiceStatusV2(status)) {
      for (const target of status.targets) {
        if (target.target.term !== candidate.term || ONLINE_ALIASES.has(target.target.campus)) continue;
        if (!values.has(target.target.campus)) values.set(target.target.campus, target.target.campus);
      }
    }
    return [...values.entries()].sort(([left], [right]) => left.localeCompare(right, 'en-US'));
  }, [candidate.term, discovery.targets, status]);
  const selectedStatuses = candidate.term === null
    ? []
    : candidate.campuses.map((campus) => targetStatus(status, candidate.term as string, campus));
  const candidateReady = candidate.term !== null
    && candidate.campuses.length > 0
    && selectedStatuses.every((target) => target?.usable === true);
  const alreadyApplied = candidateReady && sameScope(applied, candidate);
  const unavailableAction = renderUnavailableAction?.({
    actionPending,
    status,
    term: selectedTerm,
  });

  return (
    <section className="query-scope" aria-labelledby="query-scope-title">
      <style data-bcsp-query-scope="">{QUERY_SCOPE_CSS}</style>
      <header className="query-scope__head">
        <div>
          <p className="query-scope__kicker">[ 01–02 / TARGET ]</p>
          <h2 className="query-scope__title" id="query-scope-title">{i18n.t('scope.title')}</h2>
          <p className="query-scope__intro">{i18n.t('scope.intro')}</p>
        </div>
        <div className="query-scope__action">
          {candidateReady ? (
            <button
              className="bcsp-action bcsp-action--accent"
              disabled={actionPending || alreadyApplied}
              onClick={() => onApply(candidate)}
              type="button"
            >
              {i18n.t('action.apply')}
            </button>
          ) : unavailableAction ?? (
            <button className="bcsp-action bcsp-action--accent" disabled type="button">
              {i18n.t('action.apply')}
            </button>
          )}
          {alreadyApplied ? <p className="query-scope__status">{i18n.t('scope.current_use')}</p> : null}
          {actionPending ? <p className="query-scope__status" role="status">{i18n.t('scope.validating')}</p> : null}
        </div>
      </header>
      <div className="query-scope__groups">
        <fieldset className="query-scope__group">
          <legend className="query-scope__legend">{i18n.t('scope.term_group')}</legend>
          {terms.length === 0 ? (
            <p className="query-scope__status" role="status">{i18n.t('scope.term_window_unavailable')}</p>
          ) : <div className="query-scope__terms">
            {terms.map((term) => (
              <label className="query-scope__option" data-unavailable={!term.discovered || undefined} key={term.term}>
                <input
                  checked={candidate.term === term.term}
                  name="query-scope-term"
                  onChange={() => onCandidateChange({ campuses: [], term: term.term })}
                  type="radio"
                />
                <span className="query-scope__option-copy">
                  <strong>{labels.get(term.term) ?? fallbackTermLabel(term.term, i18n)}</strong>
                  <span className="query-scope__term-meta">
                    {i18n.t(termPosition(term.relativeOffset))} / <samp>{term.term}</samp>
                  </span>
                </span>
              </label>
            ))}
          </div>}
        </fieldset>
        <fieldset className="query-scope__group">
          <legend className="query-scope__legend">{i18n.t('scope.campus_group')}</legend>
          {campusLabels.length === 0 ? (
            <p className="query-scope__status" role="status">
              {selectedTerm?.discovered === false ? i18n.t('scope.not_published') : i18n.t('scope.no_ready_campus')}
            </p>
          ) : (
            <div className="query-scope__campuses">
              {campusLabels.map(([campus, label]) => {
                const target = candidate.term === null ? null : targetStatus(status, candidate.term, campus);
                const usable = target?.usable === true;
                const running = target?.workState === 'QUEUED' || target?.workState === 'RUNNING';
                const retrying = target?.workState === 'RETRY_WAIT' && target.nextRetryAt !== null;
                const failedWithoutRetry = target?.error !== null
                  && target?.error !== undefined
                  && target?.workState === 'RETRY_WAIT'
                  && target.nextRetryAt === null;
                const stateKey = usable && retrying
                  ? 'scope.target_refresh_retrying'
                  : retrying
                    ? 'scope.target_retrying'
                    : running
                      ? 'scope.target_loading'
                      : usable
                        ? 'scope.target_ready'
                        : failedWithoutRetry
                          ? 'scope.target_failed'
                          : 'scope.target_unrequested';
                const retryTime = retrying && target?.nextRetryAt !== null
                  ? i18n.formatDate(new Date(target.nextRetryAt), {
                    hour: '2-digit',
                    minute: '2-digit',
                  })
                  : null;
                return (
                  <label className="query-scope__option" data-unavailable={!usable || undefined} key={campus}>
                    <input
                      checked={candidate.campuses.includes(campus)}
                      disabled={!usable}
                      onChange={(event) => onCandidateChange({
                        campuses: event.target.checked
                          ? [...candidate.campuses, campus]
                          : candidate.campuses.filter((value) => value !== campus),
                        term: candidate.term,
                      })}
                      type="checkbox"
                    />
                    <span className="query-scope__option-copy">
                      <strong>{label} / <samp>{campus}</samp></strong>
                      <span className="query-scope__campus-state" data-state={retrying ? 'retry' : usable ? 'ready' : 'waiting'}>
                        {running ? <span aria-hidden="true" className="query-scope__loading-mark" /> : null}
                        {i18n.t(stateKey)}
                        {retryTime === null ? null : ` · ${i18n.t('scope.next_retry', { time: retryTime })}`}
                      </span>
                    </span>
                  </label>
                );
              })}
            </div>
          )}
        </fieldset>
      </div>
    </section>
  );
}
