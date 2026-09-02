import { useId, useMemo, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

import { isMessageKey } from '../i18n/contract';
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
/* Term & campus block (spec v2 section 6 step 2): two option columns, 44px rows,
   no cell borders; cell ids and DOM order are unchanged. */
.query-scope {
  display: grid;
  min-width: 0;
}
.query-scope > fieldset { min-width: 0; margin: 0; padding: 0; border: 0; }
.query-scope__legend {
  display: block;
  margin: 0 0 var(--bcsp-space-1);
  padding: 0;
  color: var(--bcsp-ink);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.25rem;
  text-transform: none;
}
.query-scope__matrix {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--bcsp-space-1) var(--bcsp-space-2);
  align-items: start;
}
.query-scope__cell {
  position: relative;
  min-width: 0;
  border-radius: var(--bcsp-radius-2);
}
.query-scope__option {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.625rem;
  align-items: center;
  min-height: var(--bcsp-control-h);
  padding: 0.375rem 0.625rem;
  border-radius: var(--bcsp-radius-2);
  cursor: pointer;
}
.query-scope__option input {
  width: 1.125rem;
  height: 1.125rem;
  margin: 0;
  accent-color: var(--bcsp-accent);
}
.query-scope__option:has(input:focus-visible) {
  z-index: 1;
  outline: 2px solid var(--bcsp-focus);
  outline-offset: -2px;
}
.query-scope__option:has(input:disabled) { cursor: not-allowed; }
.query-scope__option-copy { display: grid; gap: 0.125rem; min-width: 0; }
.query-scope__option-copy strong {
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  line-height: 1.25rem;
  overflow-wrap: anywhere;
}
.query-scope__term-meta,
.query-scope__campus-state,
.query-scope__campus-diagnostic,
.query-scope__status {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
}
/* Term readiness has to be readable without hovering, so the meta line wraps
   onto as many lines as it needs instead of ending in an ellipsis. */
.query-scope__term-meta {
  display: block;
  white-space: normal;
  overflow-wrap: anywhere;
}
.query-scope__term-meta samp,
.query-scope__campus-diagnostic samp,
.query-scope__campus-diagnostic time {
  font-size: inherit;
  font-variant-numeric: tabular-nums;
}
/* Selected term: tinted row with a 3px accent bar at the left edge. */
.query-scope__term::before {
  position: absolute;
  top: 0.375rem;
  bottom: 0.375rem;
  left: 0;
  width: 3px;
  border-radius: var(--bcsp-radius-pill);
  background: transparent;
  content: '';
}
.query-scope__term[data-selected='true'] { background: var(--bcsp-surface-selected); }
.query-scope__term[data-selected='true']::before { background: var(--bcsp-accent); }
.query-scope__term[data-readiness='none'] .query-scope__option-copy strong { color: var(--bcsp-ink-muted); }
/* Campus rows: mono code pill + name; details render inline beneath. */
.query-scope__campus-copy {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}
.query-scope__campus-copy strong {
  display: inline-flex;
  height: 1.375rem;
  flex: none;
  align-items: center;
  padding: 0 0.375rem;
  border-radius: var(--bcsp-radius-1);
  background: var(--bcsp-surface-2);
  font-size: var(--bcsp-text-micro);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1;
}
.query-scope__campus-copy strong samp { font-size: inherit; }
/* The campus name wraps rather than truncating: a half-shown campus is worse
   than a two-line one. */
.query-scope__campus-copy small {
  min-width: 0;
  font-size: var(--bcsp-text-body);
  line-height: 1.25rem;
  overflow-wrap: anywhere;
  white-space: normal;
}
.query-scope__campus[data-ready='false'] .query-scope__campus-copy { color: var(--bcsp-ink-muted); }
.query-scope__campus-details {
  display: block;
  padding: 0 0.625rem 0.375rem 2.375rem;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
}
.query-scope__campus-details > * { display: inline; }
/* The literal middot keeps both flanking spaces; a \00B7 escape swallows the
   one that terminates it and the line reads "service. ·Stage:". */
.query-scope__campus-details > * + *::before { content: ' · '; }
.query-scope__campus-details .query-scope__campus-diagnostic samp,
.query-scope__campus-details .query-scope__campus-diagnostic time { overflow-wrap: anywhere; }
/* A plainly ready campus needs no status line; diagnostics always show. */
.query-scope__campus[data-ready='true'] .query-scope__campus-details:has(.query-scope__campus-state[data-state='ready']:only-child) { display: none; }
.query-scope__campus-state[data-state='retry'] { color: var(--bcsp-danger); }
/* Apply / Search cells. */
.query-scope__cell--action,
.query-scope__cell--search {
  display: grid;
  gap: 0.375rem;
  align-content: start;
  padding: 0;
}
.query-scope__cell--action .bcsp-action,
.query-scope__cell--search .bcsp-action {
  width: 100%;
  min-width: 0;
}
.query-scope__status { overflow-wrap: anywhere; }
.query-scope__error {
  margin: 0;
  color: var(--bcsp-danger);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
  overflow-wrap: anywhere;
}
/* "Applied": a success-toned disabled pill with a CSS check (string kept). */
.bcsp-action.query-scope__applied,
.bcsp-action.query-scope__applied:disabled {
  border-color: var(--bcsp-ok-line);
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ok);
  background: var(--bcsp-ok-tint);
}
.query-scope__applied::before {
  width: 0.3125rem;
  height: 0.625rem;
  flex: none;
  border-right: 2px solid currentColor;
  border-bottom: 2px solid currentColor;
  transform: translateY(-1px) rotate(45deg);
  content: '';
}
/* Explicit placement keeps terms in column one and campuses in column two. */
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term--2'] { grid-column: 1; grid-row: 1; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term--1'] { grid-column: 1; grid-row: 2; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term-0'] { grid-column: 1; grid-row: 3; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term-1'] { grid-column: 1; grid-row: 4; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='term-2'] { grid-column: 1; grid-row: 5; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='campus-NB'] { grid-column: 2; grid-row: 1; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='campus-NK'] { grid-column: 2; grid-row: 2; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='campus-CM'] { grid-column: 2; grid-row: 3; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='scope-action'] { grid-column: 2; grid-row: 4 / span 2; }
.query-scope[data-scope-layout='local-2x5']:has([data-scope-cell='search']) [data-scope-cell='scope-action'] { grid-row: 4; }
.query-scope[data-scope-layout='local-2x5'] [data-scope-cell='search'] { grid-column: 2; grid-row: 5; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='term-0'] { grid-column: 1; grid-row: 1; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='term-1'] { grid-column: 1; grid-row: 2; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='campus-NB'] { grid-column: 2; grid-row: 1; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='campus-NK'] { grid-column: 2; grid-row: 2; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='campus-CM'] { grid-column: 2; grid-row: 3; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='scope-action'] { grid-column: 1 / -1; grid-row: 4; }
.query-scope[data-scope-layout='public-2x3-search'] [data-scope-cell='search'] { grid-column: 1 / -1; grid-row: 5; }
.query-scope__empty {
  grid-column: 1 / -1;
  margin: 0;
  padding: 0.75rem;
  border: 1px dashed var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
}
@media (hover: hover) and (pointer: fine) {
  .query-scope__option:hover:not(:has(input:disabled)) { background: var(--bcsp-surface-2); }
  .query-scope__term[data-selected='true'] .query-scope__option:hover { background: transparent; }
}
[data-bcsp-locale='zh-CN'] .query-scope__term-meta,
[data-bcsp-locale='zh-CN'] .query-scope__campus-details,
[data-bcsp-locale='zh-CN'] .query-scope__status,
[data-bcsp-locale='zh-CN'] .query-scope__error,
[data-bcsp-locale='zh-CN'] .query-scope__campus-copy strong {
  font-size: 0.8125rem;
  line-height: 1.25rem;
}
/* Match the workspace collapse: below it the two option columns stack. */
@media (max-width: 47.999rem) {
  .query-scope__matrix { grid-template-columns: minmax(0, 1fr); }
  .query-scope[data-scope-layout] [data-scope-cell] { grid-column: auto; grid-row: auto; }
}
@container (max-width: 20rem) {
  .query-scope__matrix { grid-template-columns: minmax(0, 1fr); }
  .query-scope[data-scope-layout] [data-scope-cell] { grid-column: auto; grid-row: auto; }
}
`;

export function deterministicTermLabel(term: string, i18n: BcspI18nRuntime): string {
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

/** A refresh stage is an internal enum; the user only ever sees the translated
 * phrase. An unrecognised value keeps its raw code so a new server stage is
 * still legible rather than silently dropped. */
function stageLabel(stage: string, i18n: BcspI18nRuntime): string | null {
  const key = `scope.stage.${stage}`;
  return isMessageKey(key) ? i18n.t(key) : null;
}

/** Instants are shown in the reader's locale; the machine-readable ISO value
 * stays on the <time> element's dateTime attribute. */
function instantLabel(iso: string, i18n: BcspI18nRuntime): string {
  const parsed = new Date(iso);
  if (Number.isNaN(parsed.getTime())) return iso;
  return i18n.formatDate(parsed, { dateStyle: 'short', timeStyle: 'short' });
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
  /** When set, the search cell is portaled into this element (the rail's
   * sticky submit footer); otherwise it renders inline after the Apply cell. */
  readonly searchSlot?: HTMLElement | null | undefined;
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
  searchSlot = null,
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
      <div className="query-scope__cell query-scope__term" data-readiness={readiness} data-selected={candidate.term === term.term} data-scope-cell={`term-${term.relativeOffset}`} key={term.term}>
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
      <div className="query-scope__cell query-scope__campus" data-ready={usable} data-scope-cell={`campus-${campus}`} key={campus}>
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
          <span className="query-scope__option-copy query-scope__campus-copy">
            <strong><samp>{campus}</samp></strong>
            <small>{i18n.t(`scope.campus_name.${campus}`)}</small>
          </span>
        </label>
        <span className="query-scope__campus-details" id={detailId}>
          <span className="query-scope__campus-state" data-state={retrying || terminalFailure ? 'retry' : usable ? 'ready' : 'waiting'}>
            {i18n.t(stateKey)}
          </span>
          {target?.stage === null || target?.stage === undefined ? null : (
            <span className="query-scope__campus-diagnostic" data-stage={target.stage}>
              {i18n.t('scope.target_stage')}{' '}
              {stageLabel(target.stage, i18n) ?? <samp>{target.stage}</samp>}
            </span>
          )}
          {target?.nextRetryAt === null || target?.nextRetryAt === undefined ? null : (
            <span className="query-scope__campus-diagnostic">
              {i18n.t('scope.target_retry_at')}{' '}
              <time dateTime={target.nextRetryAt}>{instantLabel(target.nextRetryAt, i18n)}</time>
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
    <div className="query-scope__cell query-scope__cell--action query-scope__action" data-scope-cell="scope-action" key="action">
      {unavailableAction ?? (
        <button
          className={scopeAction.kind === 'APPLIED'
            ? 'bcsp-action query-scope__applied'
            : scopeAction.enabled
              ? 'bcsp-action bcsp-action--accent'
              : 'bcsp-action'}
          aria-busy={scopeAction.reason === 'VALIDATING' || undefined}
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
      className="query-scope__cell query-scope__cell--search query-scope__search"
      data-scope-cell="search"
      data-span={runtime === 'PUBLIC' || undefined}
      key="search"
    >
      <button
        className="bcsp-action bcsp-action--accent"
        aria-busy={searchPending || undefined}
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
    ...(searchSlot === null ? [searchCell] : []),
  ];

  return (
    <section
      aria-label={i18n.t('scope.accessible_label')}
      className="query-scope"
      data-scope-layout={runtime === 'LOCAL' ? 'local-2x5' : 'public-2x3-search'}
    >
      <style data-bcsp-query-scope="">{QUERY_SCOPE_CSS}</style>
      <fieldset className="bcsp-plain-fieldset">
        <legend className="query-scope__legend">{i18n.t('scope.title')}</legend>
        <div className="query-scope__matrix">
          {terms.length === 0
            ? <p className="query-scope__status query-scope__empty" role="status">{i18n.t('scope.term_window_unavailable')}</p>
            : cells}
        </div>
      </fieldset>
      {searchSlot === null ? null : createPortal(searchCell, searchSlot)}
    </section>
  );
}
