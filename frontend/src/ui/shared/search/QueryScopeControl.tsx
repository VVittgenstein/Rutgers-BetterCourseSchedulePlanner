import { useId, useMemo, type ReactNode } from 'react';

import { useBcspI18n, type BcspI18nRuntime } from '../i18n/runtime';
import {
  isServiceStatusV2,
  type CatalogDiscoveryResponseV1,
  type ServiceRuntimeV1,
  type ServiceStatus,
  type ServiceTargetStatusV2,
  type ServiceTermPublicationV2,
  type ServiceVisibleTermV2,
} from '../product';
import type { SearchScope } from './SearchSession';

const MAIN_CAMPUSES = ['NB', 'NK', 'CM'] as const;

export type TermPublicationState = ServiceTermPublicationV2;

const QUERY_SCOPE_CSS = String.raw`
.query-scope {
  container-name: query-scope;
  container-type: inline-size;
  border-bottom: 3px solid var(--bcsp-line);
  background: var(--bcsp-line);
}
.query-scope > fieldset { min-width: 0; margin: 0; padding: 0; border: 0; }
.query-scope__matrix {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1px;
}
.query-scope__cell {
  min-width: 0;
  min-height: 5rem;
  background: var(--bcsp-paper);
}
.query-scope__option {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.75rem;
  align-items: center;
  min-height: 5rem;
  padding: 0.75rem 0.9rem;
  cursor: pointer;
}
.query-scope__option input {
  width: 1.15rem;
  height: 1.15rem;
  margin: 0;
  accent-color: var(--bcsp-accent);
}
.query-scope__option:has(input:focus-visible) {
  position: relative;
  z-index: 1;
  outline: 3px solid var(--bcsp-focus, var(--bcsp-accent));
  outline-offset: -3px;
}
.query-scope__option:has(input:disabled) { cursor: not-allowed; }
.query-scope__option-copy { display: grid; gap: 0.25rem; min-width: 0; }
.query-scope__option-copy strong { overflow-wrap: anywhere; }
.query-scope__term-meta,
.query-scope__campus-state,
.query-scope__campus-diagnostic,
.query-scope__status,
.query-scope__error {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-data);
  font-size: 0.7rem;
  letter-spacing: 0.045em;
  line-height: 1.4;
}
.query-scope__term[data-readiness='none'] { color: var(--bcsp-ink-muted); background: var(--bcsp-paper-raised); }
.query-scope__term[data-readiness='partial'] { border-left: 4px solid var(--bcsp-accent); }
.query-scope__term[data-readiness='ready'] .query-scope__term-meta,
.query-scope__campus-state[data-state='ready'] { color: var(--bcsp-ink); }
.query-scope__campus-state[data-state='retry'] { color: var(--bcsp-danger, #8f1d14); }
.query-scope__campus-details {
  display: grid;
  gap: 0.2rem;
  padding: 0 0.9rem 0.75rem 2.8rem;
}
.query-scope__campus-details .query-scope__campus-state,
.query-scope__campus-details .query-scope__campus-diagnostic { color: var(--bcsp-ink); }
.query-scope__campus-diagnostic samp,
.query-scope__campus-diagnostic time { color: var(--bcsp-ink); overflow-wrap: anywhere; }
.query-scope__cell--action,
.query-scope__cell--search {
  display: grid;
  align-content: stretch;
  padding: 0.75rem;
}
.query-scope__cell--action { gap: 0.35rem; }
.query-scope__cell--action .query-scope__status,
.query-scope__cell--search .query-scope__status {
  color: var(--bcsp-ink);
  background: var(--bcsp-paper);
}
.query-scope__cell--action .bcsp-action,
.query-scope__cell--search .bcsp-action { min-height: 3.5rem; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term--2'] { grid-column: 1; grid-row: 1; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term--1'] { grid-column: 1; grid-row: 2; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term-0'] { grid-column: 1; grid-row: 3; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term-1'] { grid-column: 1; grid-row: 4; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term-2'] { grid-column: 1; grid-row: 5; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='campus-NB'] { grid-column: 2; grid-row: 1; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='campus-NK'] { grid-column: 2; grid-row: 2; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='campus-CM'] { grid-column: 2; grid-row: 3; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='scope-action'] { grid-column: 2; grid-row: 4; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='search'] { grid-column: 2; grid-row: 5; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='term-0'] { grid-column: 1; grid-row: 1; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='term-1'] { grid-column: 1; grid-row: 2; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='campus-NB'] { grid-column: 2; grid-row: 1; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='campus-NK'] { grid-column: 2; grid-row: 2; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='campus-CM'] { grid-column: 1; grid-row: 3; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='scope-action'] { grid-column: 2; grid-row: 3; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='search'] { grid-column: 1 / -1; grid-row: 4; }
.query-scope__error { color: var(--bcsp-danger, #8f1d14); }
.query-scope__empty { grid-column: 1 / -1; padding: 1rem; background: var(--bcsp-paper); }
@media (hover: hover) and (pointer: fine) {
  .query-scope__option:hover:not(:has(input:disabled)) { background: var(--bcsp-paper-raised); }
}
/* Two matrix cells need 21.75rem each for long zh-CN labels and 44px controls. */
@container query-scope (max-width: 43.5rem) {
  .query-scope__matrix { grid-template-columns: minmax(0, 1fr); }
  .query-scope[data-scope-layout] [data-scope-cell] { grid-column: auto; grid-row: auto; }
}
`;

function deterministicTermLabel(term: string, i18n: BcspI18nRuntime): string {
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

function runtimeFor(status: ServiceStatus | null): ServiceRuntimeV1 {
  return status !== null && isServiceStatusV2(status) ? status.runtime : 'PUBLIC';
}

function visibleTerms(status: ServiceStatus | null): readonly ServiceVisibleTermV2[] {
  if (status === null || !isServiceStatusV2(status)) return [];
  const requiredOffsets = status.runtime === 'LOCAL'
    ? new Set([-2, -1, 0, 1, 2])
    : new Set([0, 1]);
  return [...status.termWindow.visibleTerms]
    .filter(({ relativeOffset }) => requiredOffsets.has(relativeOffset))
    .sort((left, right) => left.relativeOffset - right.relativeOffset);
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

function termReadiness(status: ServiceStatus | null, term: string): 'none' | 'partial' | 'ready' {
  const ready = MAIN_CAMPUSES.filter((campus) => targetStatus(status, term, campus)?.usable === true).length;
  if (ready === 0) return 'none';
  return ready === MAIN_CAMPUSES.length ? 'ready' : 'partial';
}

export function termPublicationState(
  term: ServiceVisibleTermV2 | null,
): TermPublicationState {
  return term?.publication ?? 'UNKNOWN';
}

export type QueryScopeActionKind = 'APPLY' | 'APPLIED' | 'PULL';
export type QueryScopeActionReason =
  | 'VALIDATING'
  | 'TERM_WINDOW_UNAVAILABLE'
  | 'SELECT_READY_CAMPUS'
  | 'SELECTED_TARGET_NOT_READY'
  | 'ALREADY_APPLIED'
  | 'PULL_UNPUBLISHED'
  | 'PULL_PUBLICATION_UNKNOWN'
  | 'PULL_ACTIVE';

export interface ResolveQueryScopeActionInput {
  readonly actionPending: boolean;
  readonly applied: SearchScope | null;
  readonly candidate: SearchScope;
  readonly pullAllowed?: boolean | undefined;
  readonly pullRequestPending?: boolean | undefined;
  readonly status: ServiceStatus | null;
  readonly term: ServiceVisibleTermV2 | null;
}

export interface QueryScopeActionResolution {
  readonly enabled: boolean;
  readonly kind: QueryScopeActionKind;
  readonly observation: {
    readonly active: boolean;
    readonly allReady: boolean;
    readonly readyCount: number;
    readonly terminalEvidence: string;
  };
  readonly reason: QueryScopeActionReason | null;
}

function terminalEvidence(targets: readonly (ServiceTargetStatusV2 | null)[]): string {
  return JSON.stringify(targets
    .filter((target): target is ServiceTargetStatusV2 => target !== null
      && target.workState === 'RETRY_WAIT'
      && target.nextRetryAt === null
      && target.error !== null)
    .map((target) => ({
      campus: target.target.campus,
      error: target.error,
      stage: target.stage,
    }))
    .sort((left, right) => left.campus.localeCompare(right.campus)));
}

export function resolveQueryScopeAction({
  actionPending,
  applied,
  candidate,
  pullAllowed = false,
  pullRequestPending = false,
  status,
  term,
}: ResolveQueryScopeActionInput): QueryScopeActionResolution {
  const termTargets = term === null
    ? []
    : MAIN_CAMPUSES.map((campus) => targetStatus(status, term.term, campus));
  const readyCount = termTargets.filter((target) => target?.usable === true).length;
  const allReady = readyCount === MAIN_CAMPUSES.length;
  const active = termTargets.some((target) => target?.workState === 'QUEUED'
    || target?.workState === 'RUNNING'
    || (target?.workState === 'RETRY_WAIT' && target.nextRetryAt !== null));
  const observation = {
    active,
    allReady,
    readyCount,
    terminalEvidence: terminalEvidence(termTargets),
  };
  const candidateReady = candidate.term !== null
    && candidate.campuses.length > 0
    && candidate.campuses.every((campus) => MAIN_CAMPUSES.includes(campus as typeof MAIN_CAMPUSES[number]))
    && candidate.campuses.every((campus) => targetStatus(status, candidate.term as string, campus)?.usable === true);
  if (candidateReady) {
    if (sameScope(applied, candidate)) {
      return { enabled: false, kind: 'APPLIED', observation, reason: 'ALREADY_APPLIED' };
    }
    return {
      enabled: !actionPending,
      kind: 'APPLY',
      observation,
      reason: actionPending ? 'VALIDATING' : null,
    };
  }

  const hasMissingTarget = term !== null
    && MAIN_CAMPUSES.some((campus) => targetStatus(status, term.term, campus)?.usable !== true);
  const canOfferPull = runtimeFor(status) === 'LOCAL'
    && pullAllowed
    && hasMissingTarget;
  if (canOfferPull) {
    const publication = termPublicationState(term);
    const hasPullableTarget = MAIN_CAMPUSES.some((campus) => {
      const target = targetStatus(status, term.term, campus);
      if (target === null) return true;
      const targetActive = target.workState === 'QUEUED'
        || target.workState === 'RUNNING'
        || (target.workState === 'RETRY_WAIT' && target.nextRetryAt !== null);
      return !target.usable && !targetActive;
    });
    if (actionPending) return { enabled: false, kind: 'PULL', observation, reason: 'VALIDATING' };
    if (publication === 'UNPUBLISHED') {
      return { enabled: false, kind: 'PULL', observation, reason: 'PULL_UNPUBLISHED' };
    }
    if (publication === 'UNKNOWN') {
      return { enabled: false, kind: 'PULL', observation, reason: 'PULL_PUBLICATION_UNKNOWN' };
    }
    if (pullRequestPending || !hasPullableTarget) {
      return { enabled: false, kind: 'PULL', observation, reason: 'PULL_ACTIVE' };
    }
    return { enabled: true, kind: 'PULL', observation, reason: null };
  }

  const reason = actionPending
    ? 'VALIDATING'
    : candidate.term === null
      ? 'TERM_WINDOW_UNAVAILABLE'
      : candidate.campuses.length === 0
        ? 'SELECT_READY_CAMPUS'
        : 'SELECTED_TARGET_NOT_READY';
  return { enabled: false, kind: 'APPLY', observation, reason };
}

export interface QueryScopeControlProps {
  readonly actionPending?: boolean | undefined;
  readonly applied: SearchScope | null;
  readonly candidate: SearchScope;
  readonly discovery: CatalogDiscoveryResponseV1;
  readonly onApply: (scope: SearchScope) => void;
  readonly onCandidateChange: (scope: SearchScope) => void;
  readonly renderUnavailableAction?: QueryScopeUnavailableActionRenderer | undefined;
  readonly searchAvailable: boolean;
  readonly searchFormId: string;
  readonly searchPending?: boolean | undefined;
  readonly status: ServiceStatus | null;
}

export interface QueryScopeUnavailableActionContext {
  readonly action: QueryScopeActionResolution;
  readonly input: ResolveQueryScopeActionInput;
}

export type QueryScopeUnavailableActionRenderer = (
  context: QueryScopeUnavailableActionContext,
) => ReactNode;

export function QueryScopeControl({
  actionPending = false,
  applied,
  candidate,
  discovery: _discovery,
  onApply,
  onCandidateChange,
  renderUnavailableAction,
  searchAvailable,
  searchFormId,
  searchPending = false,
  status,
}: QueryScopeControlProps) {
  const i18n = useBcspI18n();
  const disabledReasonId = useId();
  const appliedReasonId = useId();
  const validationStatusId = useId();
  const searchReasonId = useId();
  const baseId = useId();
  const runtime = runtimeFor(status);
  const terms = useMemo(() => visibleTerms(status), [status]);
  const selectedTerm = terms.find(({ term }) => term === candidate.term) ?? null;
  const actionInput: ResolveQueryScopeActionInput = {
    actionPending,
    applied,
    candidate,
    status,
    term: selectedTerm,
  };
  const scopeAction = resolveQueryScopeAction(actionInput);
  const appliedTerm = applied?.term ?? null;
  const unavailableAppliedTargets = applied === null || appliedTerm === null
    ? []
    : applied.campuses.filter((campus) => targetStatus(status, appliedTerm, campus)?.usable !== true)
      .map((campus) => `${appliedTerm}/${campus}`);
  const effectiveSearchAvailable = searchAvailable && unavailableAppliedTargets.length === 0;
  const unavailableAction = runtime === 'LOCAL'
    && selectedTerm !== null
    && scopeAction.kind === 'APPLY'
    && !scopeAction.enabled
    && scopeAction.reason !== 'VALIDATING'
    ? renderUnavailableAction?.({ action: scopeAction, input: actionInput })
    : null;
  const disabledApplyReason = scopeAction.reason === 'TERM_WINDOW_UNAVAILABLE'
    ? i18n.t('scope.term_window_unavailable')
    : scopeAction.reason === 'SELECT_READY_CAMPUS'
      ? i18n.t('scope.select_ready_campus')
      : i18n.t('scope.selected_not_ready');

  const termCell = (term: ServiceVisibleTermV2) => {
    const readiness = termReadiness(status, term.term);
    const readyCount = MAIN_CAMPUSES.filter((campus) => targetStatus(status, term.term, campus)?.usable === true).length;
    const readinessLabel = readiness === 'ready'
      ? i18n.t('scope.term_readiness_ready')
      : readiness === 'partial'
        ? i18n.t('scope.term_readiness_partial', { count: readyCount })
        : i18n.t('scope.term_readiness_none');
    const termTargets = MAIN_CAMPUSES.map((campus) => targetStatus(status, term.term, campus));
    const activityKey = termTargets.some((target) => target?.workState === 'QUEUED' || target?.workState === 'RUNNING')
      ? 'scope.term_state_active'
      : termTargets.some((target) => target?.workState === 'RETRY_WAIT' && target.nextRetryAt !== null)
        ? 'scope.term_state_retry'
        : termTargets.some((target) => target?.workState === 'RETRY_WAIT' && target.nextRetryAt === null && target.error !== null)
          ? 'scope.term_state_terminal'
          : null;
    return (
      <div className="query-scope__cell query-scope__term" data-readiness={readiness} data-scope-cell={`term-${term.relativeOffset}`} key={term.term}>
        <label className="query-scope__option">
          <input
            checked={candidate.term === term.term}
            name="query-scope-term"
            onChange={() => onCandidateChange({ campuses: [], term: term.term })}
            type="radio"
          />
          <span className="query-scope__option-copy">
            <strong>{deterministicTermLabel(term.term, i18n)}</strong>
            <span className="query-scope__term-meta">
              {i18n.t(termPosition(term.relativeOffset))} / <samp>{term.term}</samp> / {readinessLabel}
              {activityKey === null ? null : <> / {i18n.t(activityKey)}</>}
            </span>
          </span>
        </label>
      </div>
    );
  };

  const campusCell = (campus: typeof MAIN_CAMPUSES[number]) => {
    const target = candidate.term === null ? null : targetStatus(status, candidate.term, campus);
    const usable = target?.usable === true;
    const running = target?.workState === 'QUEUED' || target?.workState === 'RUNNING';
    const retrying = target?.workState === 'RETRY_WAIT' && target.nextRetryAt !== null;
    const terminalFailure = target?.workState === 'RETRY_WAIT'
      && target.nextRetryAt === null
      && target.error !== null;
    const stateKey = usable && running
      ? 'scope.target_refreshing'
      : usable && retrying
        ? 'scope.target_refresh_retrying'
        : usable && terminalFailure
          ? 'scope.target_refresh_failed'
          : retrying
        ? 'scope.target_retrying'
        : running
          ? 'scope.target_loading'
          : usable
            ? 'scope.target_ready'
            : terminalFailure
              ? 'scope.target_failed'
              : 'scope.target_unrequested';
    const detailId = `${baseId}-campus-${campus}-status`;
    return (
      <div className="query-scope__cell" data-scope-cell={`campus-${campus}`} key={campus}>
        <label className="query-scope__option">
          <input
            aria-describedby={detailId}
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
            <strong><samp>{campus}</samp></strong>
          </span>
        </label>
        <span className="query-scope__campus-details" id={detailId}>
          <span className="query-scope__campus-state" data-state={retrying || terminalFailure ? 'retry' : usable ? 'ready' : 'waiting'}>
            {i18n.t(stateKey)}
          </span>
          {target?.stage === null || target?.stage === undefined ? null : (
            <span className="query-scope__campus-diagnostic">
              {i18n.t('scope.target_stage')} <samp>{target.stage}</samp>
            </span>
          )}
          {target?.nextRetryAt === null || target?.nextRetryAt === undefined ? null : (
            <span className="query-scope__campus-diagnostic">
              {i18n.t('scope.target_retry_at')} <time dateTime={target.nextRetryAt}>{target.nextRetryAt}</time>
            </span>
          )}
          {target?.error?.code === undefined ? null : (
            <span className="query-scope__campus-diagnostic">
              {i18n.t('scope.target_diagnostic')} <samp>{target.error.code}</samp>
            </span>
          )}
        </span>
      </div>
    );
  };

  const actionCell = (
    <div className="query-scope__cell query-scope__cell--action" data-scope-cell="scope-action" key="action">
      {unavailableAction ?? (
        <button
          className="bcsp-action bcsp-action--accent"
          aria-describedby={scopeAction.reason === 'VALIDATING'
            ? validationStatusId
            : scopeAction.kind === 'APPLIED'
              ? appliedReasonId
              : scopeAction.enabled ? undefined : disabledReasonId}
          disabled={!scopeAction.enabled}
          onClick={scopeAction.kind === 'APPLY' ? () => onApply(candidate) : undefined}
          type="button"
        >
          {i18n.t(scopeAction.kind === 'APPLIED' ? 'action.applied' : 'action.apply')}
        </button>
      )}
      {scopeAction.kind === 'APPLIED'
        ? <p className="query-scope__status" id={appliedReasonId}>{i18n.t('scope.already_applied_reason')}</p>
        : null}
      {unavailableAction === null
        && scopeAction.kind === 'APPLY'
        && !scopeAction.enabled
        && scopeAction.reason !== 'VALIDATING'
        ? <p className="query-scope__status" id={disabledReasonId}>{disabledApplyReason}</p>
        : null}
      {scopeAction.reason === 'VALIDATING'
        ? <p className="query-scope__status" id={validationStatusId} role="status">{i18n.t('scope.validating')}</p>
        : null}
    </div>
  );

  const searchCell = (
    <div
      className="query-scope__cell query-scope__cell--search"
      data-scope-cell="search"
      data-span={runtime === 'PUBLIC' || undefined}
      key="search"
    >
      <button
        className="bcsp-action bcsp-action--accent"
        aria-describedby={!effectiveSearchAvailable && !searchPending ? searchReasonId : undefined}
        disabled={!effectiveSearchAvailable || searchPending}
        form={searchFormId}
        type="submit"
      >
        {i18n.t('action.search')}
      </button>
      {!effectiveSearchAvailable && !searchPending
        ? <p className="query-scope__status" id={searchReasonId}>
          {applied !== null && unavailableAppliedTargets.length > 0
            ? i18n.t('scope.search_applied_targets_unavailable', { targets: unavailableAppliedTargets.join(', ') })
            : i18n.t('scope.search_requires_applied')}
        </p>
        : null}
    </div>
  );

  const cells: ReactNode[] = [
    ...terms.map(termCell),
    ...MAIN_CAMPUSES.map(campusCell),
    actionCell,
    searchCell,
  ];

  return (
    <section
      aria-label={i18n.t('scope.accessible_label')}
      className="query-scope"
      data-scope-layout={runtime === 'LOCAL' ? 'local-2x5' : 'public-2x3-search'}
    >
      <style data-bcsp-query-scope="">{QUERY_SCOPE_CSS}</style>
      <fieldset className="bcsp-plain-fieldset">
        <legend className="bcsp-visually-hidden">{i18n.t('scope.accessible_label')}</legend>
        <div className="query-scope__matrix">
          {terms.length === 0
            ? <p className="query-scope__status query-scope__empty" role="status">{i18n.t('scope.term_window_unavailable')}</p>
            : cells}
        </div>
      </fieldset>
    </section>
  );
}
