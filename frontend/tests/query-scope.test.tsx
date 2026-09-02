// @vitest-environment jsdom

import { useEffect, useState } from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import { LocalTermPullAction } from '../src/ui/local/LocalTermPullAction';
import type {
  CatalogDiscoveryResponseV1,
  CourseQueryRequestV1,
  CourseQueryResponseV1,
  ServiceStatusV2,
  ServiceTargetStatusV2,
  ServiceVisibleTermV2,
} from '../src/ui/shared/product';
import { createNeutralFilterState } from '../src/ui/shared/product';
import {
  QueryScopeControl,
  resolveQueryScopeAction,
} from '../src/ui/shared/search/QueryScopeControl';
import {
  SearchSessionProvider,
  type SearchScope,
  useSearchSession,
} from '../src/ui/shared/search/SearchSession';

const point = {
  contentVersion: 1,
  observationId: '20000000-0000-4000-8000-000000000001',
  observedAt: '2026-07-17T00:00:00Z',
} as const;
const provenance = {
  ...point,
  payloadDigest: 'a'.repeat(64),
  sourceId: 'synthetic-selector',
  sourceKind: 'SELECTOR',
} as const;
const known = (value: string) => ({
  knowledge: 'KNOWN',
  presence: { presence: 'PRESENT', value },
} as const);

const TERM_LABELS: Readonly<Record<string, string>> = {
  '02026': 'Winter 2026',
  '12026': 'Spring 2026',
  '72026': 'Summer 2026',
  '92026': 'Fall 2026',
  '02027': 'Winter 2027',
  '12027': 'Spring 2027',
};

function discovery(
  terms: readonly string[],
  campuses: readonly string[] = ['NB', 'NK', 'ONLINE_NB'],
): CatalogDiscoveryResponseV1 {
  return {
    contractVersion: 1,
    observedAt: point.observedAt,
    sources: [],
    status: {
      availability: 'CURRENT',
      error: null,
      isStale: false,
      lastSuccess: point,
      latestAttempt: point,
    },
    coreCodeDictionaries: [],
    subjects: [],
    targets: terms.flatMap((term) => campuses.map((campus) => ({
      campusLabel: known(campus === 'NB'
        ? 'New Brunswick'
        : campus === 'NK'
          ? 'Newark'
          : campus === 'CM'
            ? 'Camden'
            : 'Online New Brunswick'),
      key: { campus, term },
      provenance,
      termLabel: known(TERM_LABELS[term] ?? term),
    }))),
  };
}

function target(
  term: string,
  campus: string,
  overrides: Partial<ServiceTargetStatusV2> = {},
): ServiceTargetStatusV2 {
  return {
    target: { term, campus },
    primary: campus === 'NB',
    snapshotAvailability: 'READY',
    workState: 'IDLE',
    stage: null,
    usable: true,
    catalogContentVersion: 7,
    lastCompleteAt: '2026-07-17T00:00:00Z',
    nextRetryAt: null,
    error: null,
    ...overrides,
  };
}

function targetError(code: string) {
  return {
    code,
    httpStatus: 503,
    contentType: 'text/html',
    contentEncoding: null,
    decodedBytes: 2048,
    errorClass: 'UPSTREAM',
    errorChain: 'fixture',
    traceId: 'trace-fixture',
  } as const;
}

function status(
  runtime: 'LOCAL' | 'PUBLIC',
  visibleTerms: readonly ServiceVisibleTermV2[],
  targets: readonly ServiceTargetStatusV2[],
): ServiceStatusV2 {
  const currentTerm = visibleTerms.find(({ relativeOffset }) => relativeOffset === 0)?.term
    ?? visibleTerms[0]?.term
    ?? '72026';
  const nextTerm = visibleTerms.find(({ relativeOffset }) => relativeOffset === 1)?.term
    ?? visibleTerms[1]?.term
    ?? currentTerm;
  return {
    contractVersion: 2,
    observedAt: '2026-07-17T00:00:00Z',
    runtime,
    level: targets.some(({ usable }) => usable) ? 'PARTIALLY_READY' : 'INITIALIZING',
    discovery: discovery([currentTerm, nextTerm]).status,
    termWindow: { currentTerm, nextTerm, visibleTerms },
    automaticTermSummaries: [currentTerm, nextTerm].map((term) => ({
      term,
      readyTargetCount: targets.filter((entry) => entry.target.term === term && entry.usable).length,
      totalTargetCount: targets.filter((entry) => entry.target.term === term).length,
    })),
    operations: [],
    targets,
    issues: [],
  };
}

function visible(
  term: string,
  relativeOffset: -2 | -1 | 0 | 1 | 2,
  manualPullAllowed = false,
): ServiceVisibleTermV2 {
  return {
    term,
    relativeOffset,
    publication: 'PUBLISHED',
    autoManaged: relativeOffset === 0 || relativeOffset === 1,
    manualPullAllowed,
    watchable: relativeOffset === 0 || relativeOffset === 1,
  };
}

function ScopeHarness({
  serviceStatus,
  catalogDiscovery,
  local = false,
  initialCandidate,
  onPull = vi.fn(async () => undefined),
}: {
  readonly serviceStatus: ServiceStatusV2;
  readonly catalogDiscovery: CatalogDiscoveryResponseV1;
  readonly local?: boolean;
  readonly initialCandidate?: SearchScope;
  readonly onPull?: (term: string) => Promise<void>;
}) {
  const [candidate, setCandidate] = useState<SearchScope>(initialCandidate ?? {
    term: serviceStatus.termWindow.currentTerm,
    campuses: [],
  });
  const [applied, setApplied] = useState<SearchScope | null>(null);
  return (
    <BcspI18nProvider initialLocale="en-US">
      <QueryScopeControl
        applied={applied}
        candidate={candidate}
        discovery={catalogDiscovery}
        onApply={setApplied}
        onCandidateChange={setCandidate}
        renderUnavailableAction={local ? (context) => (
          <LocalTermPullAction {...context} onPull={onPull} />
        ) : undefined}
        searchAvailable={applied !== null}
        searchFormId="test-filter-form"
        status={serviceStatus}
      />
      <output data-testid="candidate">{JSON.stringify(candidate)}</output>
      <output data-testid="applied">{JSON.stringify(applied)}</output>
    </BcspI18nProvider>
  );
}

function SessionProbe() {
  const session = useSearchSession();
  useEffect(() => {
    session.initializeScope(
      { term: '72026', campuses: [] },
      null,
      createNeutralFilterState('72026'),
    );
  }, [session.initializeScope]);
  return (
    <>
      <output data-testid="session-candidate">{JSON.stringify(session.state.candidateScope)}</output>
      <output data-testid="session-applied">{JSON.stringify(session.state.appliedScope)}</output>
      <button
        onClick={() => session.setCandidateScope({ term: '92026', campuses: ['NB'] })}
        type="button"
      >Change candidate</button>
      <button
        onClick={() => session.applyScope(
          { term: '92026', campuses: ['NB'] },
          { ...createNeutralFilterState('92026'), campuses: ['NB'] },
        )}
        type="button"
      >Apply candidate</button>
    </>
  );
}

const STORED_SCOPE: SearchScope = { term: '72026', campuses: ['NB'] };
const STORED_FILTERS = { ...createNeutralFilterState('72026'), campuses: ['NB'], keywords: ['data'] };
const OTHER_SCOPE: SearchScope = { term: '72026', campuses: ['NK'] };

/** Drives INITIALIZE_SCOPE the way SearchWorkspace does on every status poll:
 * first with no applied scope, later with the stored scope once it is ready. */
function AdoptionProbe({ initialTerm = '72026' }: { readonly initialTerm?: string | null }) {
  const session = useSearchSession();
  useEffect(() => {
    session.initializeScope(
      { term: initialTerm, campuses: [] },
      null,
      { ...createNeutralFilterState(initialTerm), keywords: ['data'] },
    );
  }, [initialTerm, session.initializeScope]);
  return (
    <>
      <output data-testid="session-candidate">{JSON.stringify(session.state.candidateScope)}</output>
      <output data-testid="session-applied">{JSON.stringify(session.state.appliedScope)}</output>
      <output data-testid="session-draft-campuses">
        {JSON.stringify(session.state.draftFilters?.campuses ?? null)}
      </output>
      <button
        onClick={() => session.initializeScope(STORED_SCOPE, STORED_SCOPE, STORED_FILTERS)}
        type="button"
      >Adopt stored scope</button>
      <button
        onClick={() => session.initializeScope(OTHER_SCOPE, OTHER_SCOPE, { ...STORED_FILTERS, campuses: ['NK'] })}
        type="button"
      >Adopt other scope</button>
      <button
        onClick={() => session.initializeScope({ term: '72026', campuses: [] }, null, STORED_FILTERS)}
        type="button"
      >Poll without applied scope</button>
      <button
        onClick={() => session.setCandidateScope({ term: '72026', campuses: ['NK'] })}
        type="button"
      >Change candidate</button>
      <button
        onClick={() => session.setDraftFilters({ ...createNeutralFilterState('72026'), levels: ['U'] }, true)}
        type="button"
      >Edit draft</button>
      <button
        onClick={() => session.applyScope(OTHER_SCOPE, { ...createNeutralFilterState('72026'), campuses: ['NK'] })}
        type="button"
      >Apply other scope</button>
    </>
  );
}

const staleRequest: CourseQueryRequestV1 = {
  filters: {
    contractVersion: 3,
    values: {
      ...createNeutralFilterState('12026'),
      campuses: ['NB'],
    },
  },
  page: { page: 1, pageSize: 25 },
  sort: { direction: 'DESCENDING', field: 'RELEVANCE' },
};

const staleResponse: CourseQueryResponseV1 = {
  contractVersion: 3,
  items: [],
  page: { page: 1, pageSize: 25, total: 4559, totalPages: 183 },
};

function StaleResponseProbe() {
  const session = useSearchSession();
  useEffect(() => {
    const filters = { ...createNeutralFilterState('12026'), campuses: ['NB'] };
    session.initializeScope(
      { term: '12026', campuses: ['NB'] },
      { term: '12026', campuses: ['NB'] },
      filters,
    );
  }, [session.initializeScope]);
  return (
    <>
      <output data-testid="successful-total">
        {session.state.lastSuccessfulResponse?.page.total ?? 'none'}
      </output>
      <button onClick={() => session.recordSubmission(staleRequest)} type="button">
        Start old search
      </button>
      <button
        onClick={() => session.applyScope(
          { term: '72026', campuses: ['NB'] },
          { ...createNeutralFilterState('72026'), campuses: ['NB'] },
        )}
        type="button"
      >Apply new scope</button>
      <button onClick={() => session.recordSuccess(staleRequest, staleResponse)} type="button">
        Finish old search
      </button>
    </>
  );
}

afterEach(cleanup);

describe('RC3 query scope contract', () => {
  it('uses one pure resolver for Apply, Applied, Pull, publication, activity, and 0/3–3/3 readiness', () => {
    const past = visible('12026', -1, true);
    const current = visible('72026', 0);
    const next = visible('92026', 1);
    const terms = [past, current, next];
    const baseInput = {
      actionPending: false,
      applied: null,
      candidate: { term: past.term, campuses: [] },
      pullAllowed: true,
      status: status('LOCAL', terms, []),
      term: past,
    } as const;

    expect(resolveQueryScopeAction(baseInput)).toMatchObject({
      enabled: true, kind: 'PULL', observation: { readyCount: 0 }, reason: null,
    });
    expect(resolveQueryScopeAction({ ...baseInput, pullRequestPending: true })).toMatchObject({
      enabled: false, kind: 'PULL', reason: 'PULL_ACTIVE',
    });
    expect(resolveQueryScopeAction({
      ...baseInput, term: { ...past, publication: 'UNPUBLISHED' },
    })).toMatchObject({ enabled: false, kind: 'PULL', reason: 'PULL_UNPUBLISHED' });
    expect(resolveQueryScopeAction({
      ...baseInput, term: { ...past, publication: 'UNKNOWN' },
    })).toMatchObject({ enabled: false, kind: 'PULL', reason: 'PULL_PUBLICATION_UNKNOWN' });

    const activeTargets = ['NB', 'NK', 'CM'].map((campus) => target(past.term, campus, {
      snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', workState: 'RUNNING', stage: 'CATALOG_FETCH',
      usable: false, catalogContentVersion: null, lastCompleteAt: null,
    }));
    expect(resolveQueryScopeAction({
      ...baseInput, status: status('LOCAL', terms, activeTargets),
    })).toMatchObject({
      enabled: false, kind: 'PULL', observation: { active: true }, reason: 'PULL_ACTIVE',
    });
    const terminal = target(past.term, 'NB', {
      snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', workState: 'RETRY_WAIT', usable: false,
      catalogContentVersion: null, lastCompleteAt: null, nextRetryAt: null, error: targetError('TERMINAL'),
    });
    expect(resolveQueryScopeAction({
      ...baseInput, status: status('LOCAL', terms, [terminal]),
    })).toMatchObject({ enabled: true, kind: 'PULL', reason: null });

    const oneReady = status('LOCAL', terms, [target(past.term, 'NB')]);
    expect(resolveQueryScopeAction({
      ...baseInput, candidate: { term: past.term, campuses: ['NB'] }, status: oneReady,
    })).toMatchObject({ enabled: true, kind: 'APPLY', observation: { readyCount: 1 } });
    const twoReady = status('LOCAL', terms, [target(past.term, 'NB'), target(past.term, 'NK')]);
    expect(resolveQueryScopeAction({
      ...baseInput, candidate: { term: past.term, campuses: ['NK'] }, status: twoReady,
    })).toMatchObject({ enabled: true, kind: 'APPLY', observation: { readyCount: 2 } });
    const threeReady = status('LOCAL', terms, [
      target(past.term, 'NB'), target(past.term, 'NK'), target(past.term, 'CM'),
    ]);
    expect(resolveQueryScopeAction({ ...baseInput, status: threeReady })).toMatchObject({
      enabled: false, kind: 'APPLY', observation: { allReady: true, readyCount: 3 }, reason: 'SELECT_READY_CAMPUS',
    });
    expect(resolveQueryScopeAction({
      ...baseInput,
      applied: { term: past.term, campuses: ['NK'] },
      candidate: { term: past.term, campuses: ['NB'] },
      status: threeReady,
    })).toMatchObject({ enabled: true, kind: 'APPLY', reason: null });
    expect(resolveQueryScopeAction({
      ...baseInput,
      applied: { term: past.term, campuses: ['NB'] },
      candidate: { term: past.term, campuses: ['NB'] },
      status: threeReady,
    })).toMatchObject({ enabled: false, kind: 'APPLIED', reason: 'ALREADY_APPLIED' });
    expect(resolveQueryScopeAction({
      ...baseInput,
      actionPending: true,
      candidate: { term: past.term, campuses: ['NB'] },
      status: threeReady,
    })).toMatchObject({ enabled: false, kind: 'APPLY', reason: 'VALIDATING' });

    for (const autoManagedTerm of [current, next]) {
      expect(resolveQueryScopeAction({
        ...baseInput,
        candidate: { term: autoManagedTerm.term, campuses: [] },
        pullAllowed: false,
        term: autoManagedTerm,
      }).kind).toBe('APPLY');
    }
    expect(resolveQueryScopeAction({
      ...baseInput, status: status('PUBLIC', terms, []),
    }).kind).toBe('APPLY');
  });
  it('exposes pending Apply and Search operations through aria-busy without changing button labels', () => {
    const terms = [visible('72026', 0), visible('92026', 1)];
    const serviceStatus = status('PUBLIC', terms, [
      target('72026', 'NB'), target('72026', 'NK'), target('72026', 'CM'),
    ]);
    const view = render(
      <BcspI18nProvider initialLocale="en-US">
        <QueryScopeControl
          actionPending
          applied={null}
          candidate={{ term: '72026', campuses: ['NB'] }}
          discovery={discovery(terms.map(({ term }) => term))}
          onApply={vi.fn()}
          onCandidateChange={vi.fn()}
          searchAvailable
          searchFormId="test-filter-form"
          status={serviceStatus}
        />
      </BcspI18nProvider>,
    );
    const apply = screen.getByRole('button', { name: 'Apply' });
    expect(apply.getAttribute('aria-busy')).toBe('true');

    view.rerender(
      <BcspI18nProvider initialLocale="en-US">
        <QueryScopeControl
          applied={{ term: '72026', campuses: ['NB'] }}
          candidate={{ term: '72026', campuses: ['NB'] }}
          discovery={discovery(terms.map(({ term }) => term))}
          onApply={vi.fn()}
          onCandidateChange={vi.fn()}
          searchAvailable
          searchFormId="test-filter-form"
          searchPending
          status={serviceStatus}
        />
      </BcspI18nProvider>,
    );
    const search = screen.getByRole('button', { name: 'Search' });
    expect(search.getAttribute('aria-busy')).toBe('true');
  });

  it('never falls back to the unbounded Discovery term list before Status V2 arrives', () => {
    render(
      <BcspI18nProvider initialLocale="en-US">
        <QueryScopeControl
          applied={null}
          candidate={{ term: null, campuses: [] }}
          discovery={discovery(['12025', '72025', '92025', '12026', '72026', '92026'])}
          onApply={vi.fn()}
          onCandidateChange={vi.fn()}
          searchAvailable={false}
          searchFormId="test-filter-form"
          status={null}
        />
      </BcspI18nProvider>,
    );
    expect(screen.queryAllByRole('radio')).toHaveLength(0);
    expect(screen.getByText('Waiting for the authoritative Rutgers term window.')).toBeTruthy();
  });

  it('renders exactly two Public terms, five Local terms, and never auto-selects a Campus', () => {
    const publicTerms = [visible('72026', 0), visible('92026', 1)];
    const publicStatus = status('PUBLIC', publicTerms, [
      target('72026', 'NB'), target('72026', 'NK'), target('72026', 'CM'), target('92026', 'NB'),
    ]);
    const first = render(
      <ScopeHarness
        catalogDiscovery={discovery(publicTerms.map(({ term }) => term))}
        serviceStatus={publicStatus}
      />,
    );
    expect(screen.getAllByRole('radio')).toHaveLength(2);
    expect(screen.getAllByRole('checkbox')).toHaveLength(3);
    expect(document.querySelector('[data-scope-layout="public-2x3-search"]')).not.toBeNull();
    expect([...document.querySelectorAll('[data-scope-cell]')].map((cell) => cell.getAttribute('data-scope-cell'))).toEqual([
      'term-0', 'term-1', 'campus-NB', 'campus-NK', 'campus-CM', 'scope-action', 'search',
    ]);
    expect(document.querySelector('[data-scope-cell="search"]')?.getAttribute('data-span')).toBe('true');
    expect([...document.querySelectorAll('.query-scope input, .query-scope button')]
      .map((control) => control.closest('[data-scope-cell]')?.getAttribute('data-scope-cell'))).toEqual([
      'term-0', 'term-1', 'campus-NB', 'campus-NK', 'campus-CM', 'scope-action', 'search',
    ]);
    expect(screen.getAllByRole('checkbox').every((input) => !(input as HTMLInputElement).checked)).toBe(true);
    const disabledApply = screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement;
    expect(disabledApply.disabled).toBe(true);
    expect(document.querySelector('[data-scope-cell="term-0"]')?.textContent).toContain('Ready 3/3');
    const disabledApplyDescription = disabledApply.getAttribute('aria-describedby');
    expect(disabledApplyDescription).not.toBeNull();
    expect(document.getElementById(disabledApplyDescription ?? '')?.textContent).toContain(
      'Select at least one ready Campus',
    );
    first.unmount();

    const localTerms = [
      visible('02026', -2, true),
      visible('12026', -1, true),
      visible('72026', 0),
      visible('92026', 1),
      visible('02027', 2, true),
    ];
    render(
      <ScopeHarness
        catalogDiscovery={discovery(localTerms.map(({ term }) => term))}
        local
        serviceStatus={status('LOCAL', localTerms, [target('72026', 'NB')])}
      />,
    );
    expect(screen.getAllByRole('radio')).toHaveLength(5);
    expect(document.querySelector('[data-scope-layout="local-2x5"]')).not.toBeNull();
    expect([...document.querySelectorAll('[data-scope-cell]')].map((cell) => cell.getAttribute('data-scope-cell'))).toEqual([
      'term--2', 'term--1', 'term-0', 'term-1', 'term-2',
      'campus-NB', 'campus-NK', 'campus-CM', 'scope-action', 'search',
    ]);
    const semanticControlOrder = [...document.querySelectorAll('.query-scope input, .query-scope button')]
      .map((control) => control.closest('[data-scope-cell]')?.getAttribute('data-scope-cell'));
    expect(semanticControlOrder).toEqual([
      'term--2', 'term--1', 'term-0', 'term-1', 'term-2',
      'campus-NB', 'campus-NK', 'campus-CM', 'scope-action', 'search',
    ]);
    expect(screen.getAllByRole('checkbox').every((input) => !(input as HTMLInputElement).checked)).toBe(true);
    expect(document.querySelector('.query-scope')?.textContent).toContain('Not loaded 0/3');
    expect(document.querySelector('[data-scope-cell="term--2"]')?.textContent).toContain('Not loaded 0/3');
  });

  it('uses a localized term label when an unpublished visible term has no Discovery target', () => {
    const terms = [
      visible('72026', 0),
      { ...visible('92026', 1), publication: 'UNPUBLISHED' },
    ];
    render(
      <ScopeHarness
        catalogDiscovery={discovery(['72026'], ['NB'])}
        serviceStatus={status('PUBLIC', terms, [target('72026', 'NB')])}
      />,
    );

    expect(screen.getByRole('radio', { name: /Fall 2026/u })).toBeTruthy();
    expect(document.querySelector('[data-scope-cell="term-1"]')?.textContent).toContain('Not loaded 0/3');
    expect(document.querySelector('.query-scope')?.textContent).not.toMatch(
      /Not published by Rutgers|Publication status unknown/u,
    );
  });

  it('distinguishes publication unknown, active, retry, and terminal term states', () => {
    const terms = [visible('72026', 0), visible('92026', 1)];
    const activeAndTerminal = status('PUBLIC', terms, [
      target('72026', 'NB', {
        snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', workState: 'RUNNING', stage: 'OPEN_FETCH', usable: false,
      }),
      target('92026', 'NB', {
        snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', workState: 'RETRY_WAIT', usable: false,
        nextRetryAt: null, error: targetError('TERMINAL'),
      }),
    ]);
    const view = render(<ScopeHarness catalogDiscovery={discovery(['72026', '92026'])} serviceStatus={activeAndTerminal} />);
    expect(document.querySelector('[data-scope-cell="term-0"]')?.textContent).toContain('Queued or loading');
    expect(document.querySelector('[data-scope-cell="term-1"]')?.textContent).toContain('Load failed; retry available');
    view.unmount();

    const retryAndUnknownTerms = [
      visible('72026', 0),
      { ...visible('92026', 1), publication: 'UNKNOWN' as const },
    ];
    const retryAndUnknown = status('PUBLIC', retryAndUnknownTerms, [target('72026', 'NB', {
      snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', workState: 'RETRY_WAIT', usable: false,
      nextRetryAt: '2026-07-17T01:00:00Z', error: targetError('RETRY'),
    })]);
    render(<ScopeHarness catalogDiscovery={discovery(['72026', '92026'])} serviceStatus={retryAndUnknown} />);
    expect(document.querySelector('[data-scope-cell="term-0"]')?.textContent).toContain('Retry scheduled');
    expect(document.querySelector('[data-scope-cell="term-1"]')?.textContent).toContain('Not loaded 0/3');
    expect(document.querySelector('.query-scope')?.textContent).not.toMatch(
      /Not published by Rutgers|Publication status unknown/u,
    );
  });

  it('keeps candidate and applied scopes separate until the explicit Apply action', () => {
    render(
      <SearchSessionProvider>
        <SessionProbe />
      </SearchSessionProvider>,
    );
    expect(screen.getByTestId('session-applied').textContent).toBe('null');
    fireEvent.click(screen.getByRole('button', { name: 'Change candidate' }));
    expect(screen.getByTestId('session-candidate').textContent).toBe(
      JSON.stringify({ term: '92026', campuses: ['NB'] }),
    );
    expect(screen.getByTestId('session-applied').textContent).toBe('null');
    fireEvent.click(screen.getByRole('button', { name: 'Apply candidate' }));
    expect(screen.getByTestId('session-applied').textContent).toBe(
      JSON.stringify({ term: '92026', campuses: ['NB'] }),
    );
  });

  it('adopts a stored scope once when it becomes ready and ignores every later initialisation', () => {
    render(
      <SearchSessionProvider>
        <AdoptionProbe />
      </SearchSessionProvider>,
    );
    expect(screen.getByTestId('session-applied').textContent).toBe('null');
    fireEvent.click(screen.getByRole('button', { name: 'Poll without applied scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe('null');
    expect(screen.getByTestId('session-draft-campuses').textContent).toBe('[]');

    fireEvent.click(screen.getByRole('button', { name: 'Adopt stored scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe(JSON.stringify(STORED_SCOPE));
    expect(screen.getByTestId('session-candidate').textContent).toBe(JSON.stringify(STORED_SCOPE));
    expect(screen.getByTestId('session-draft-campuses').textContent).toBe('["NB"]');

    fireEvent.click(screen.getByRole('button', { name: 'Adopt other scope' }));
    fireEvent.click(screen.getByRole('button', { name: 'Poll without applied scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe(JSON.stringify(STORED_SCOPE));
    expect(screen.getByTestId('session-candidate').textContent).toBe(JSON.stringify(STORED_SCOPE));
  });

  it('never adopts a stored scope after the user changed the candidate', () => {
    render(
      <SearchSessionProvider>
        <AdoptionProbe />
      </SearchSessionProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Change candidate' }));
    fireEvent.click(screen.getByRole('button', { name: 'Adopt stored scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe('null');
    expect(screen.getByTestId('session-candidate').textContent).toBe(
      JSON.stringify({ term: '72026', campuses: ['NK'] }),
    );
  });

  it('never adopts a stored scope after the user edited the draft', () => {
    render(
      <SearchSessionProvider>
        <AdoptionProbe />
      </SearchSessionProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Edit draft' }));
    fireEvent.click(screen.getByRole('button', { name: 'Adopt stored scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe('null');
    expect(screen.getByTestId('session-draft-campuses').textContent).toBe('[]');
  });

  it('never resurrects a stored scope after an explicit Apply', () => {
    render(
      <SearchSessionProvider>
        <AdoptionProbe />
      </SearchSessionProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Apply other scope' }));
    fireEvent.click(screen.getByRole('button', { name: 'Adopt stored scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe(JSON.stringify(OTHER_SCOPE));
    expect(screen.getByTestId('session-candidate').textContent).toBe(JSON.stringify(OTHER_SCOPE));
  });

  it('carries a stored applied scope through the null-term upgrade unless the candidate was touched', () => {
    const first = render(
      <SearchSessionProvider>
        <AdoptionProbe initialTerm={null} />
      </SearchSessionProvider>,
    );
    expect(screen.getByTestId('session-candidate').textContent).toBe(
      JSON.stringify({ term: null, campuses: [] }),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Adopt stored scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe(JSON.stringify(STORED_SCOPE));
    expect(screen.getByTestId('session-candidate').textContent).toBe(JSON.stringify(STORED_SCOPE));
    first.unmount();

    render(
      <SearchSessionProvider>
        <AdoptionProbe initialTerm={null} />
      </SearchSessionProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Change candidate' }));
    fireEvent.click(screen.getByRole('button', { name: 'Poll without applied scope' }));
    expect(screen.getByTestId('session-applied').textContent).toBe('null');
    expect(screen.getByTestId('session-candidate').textContent).toBe(
      JSON.stringify({ term: '72026', campuses: ['NK'] }),
    );
  });

  it('rejects a late response from the previously applied scope', () => {
    render(
      <SearchSessionProvider>
        <StaleResponseProbe />
      </SearchSessionProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Start old search' }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply new scope' }));
    fireEvent.click(screen.getByRole('button', { name: 'Finish old search' }));
    expect(screen.getByTestId('successful-total').textContent).toBe('none');
  });

  it('keeps READY and LKG targets usable when publication is UNKNOWN', () => {
    const terms = [
      { ...visible('72026', 0), publication: 'UNKNOWN' as const },
      visible('92026', 1),
    ];
    const serviceStatus = status('PUBLIC', terms, [
      target('72026', 'NB'),
      target('72026', 'NK', {
        snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
        workState: 'RETRY_WAIT',
        usable: false,
        catalogContentVersion: null,
        lastCompleteAt: null,
        nextRetryAt: '2026-07-17T00:01:00Z',
        stage: 'CATALOG_PROCESS',
        error: targetError('RUTGERS_503'),
      }),
      target('72026', 'CM', {
        workState: 'RETRY_WAIT',
        nextRetryAt: '2026-07-17T00:01:00Z',
        stage: 'OPEN_FETCH',
        error: targetError('OPEN_RETRY'),
      }),
    ]);
    render(
      <ScopeHarness
        catalogDiscovery={discovery(['72026', '92026'], ['NB', 'NK', 'CM', 'ONLINE_NB'])}
        serviceStatus={serviceStatus}
      />,
    );

    const nb = screen.getByRole('checkbox', { name: /NB/u });
    const nk = screen.getByRole('checkbox', { name: /NK/u });
    const cm = screen.getByRole('checkbox', { name: /CM/u });
    expect((nk as HTMLInputElement).disabled).toBe(true);
    expect((cm as HTMLInputElement).disabled).toBe(false);
    expect(screen.getByText(/Refresh failed; retry scheduled/u)).toBeTruthy();
    // The stage enum reaches the reader as a translated phrase; the raw code
    // stays available to tests and styling on data-stage.
    expect(document.querySelector('[data-scope-cell="campus-NK"] [data-stage="CATALOG_PROCESS"]')?.textContent)
      .toContain('processing the course catalogue');
    expect(document.querySelector('[data-scope-cell="campus-NK"]')?.textContent).not.toContain('CATALOG_PROCESS');
    expect(document.querySelector('[data-scope-cell="campus-NK"] time')?.getAttribute('datetime'))
      .toBe('2026-07-17T00:01:00Z');
    expect(document.querySelector('[data-scope-cell="campus-NK"]')?.textContent).not.toContain('2026-07-17T00:01:00Z');
    expect(document.querySelector('[data-scope-cell="campus-NK"]')?.textContent).toContain('RUTGERS_503');
    expect(document.querySelector('[data-scope-cell="campus-CM"]')?.textContent).toContain(
      'The last complete data remains available',
    );
    expect(document.querySelector('[data-scope-cell="term-0"]')?.textContent).toContain(
      'Partially available 2/3',
    );
    fireEvent.click(nb);
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    const applied = screen.getByRole('button', { name: 'Applied' }) as HTMLButtonElement;
    expect(applied.disabled).toBe(true);
    const appliedDescription = applied.getAttribute('aria-describedby');
    expect(appliedDescription).not.toBeNull();
    expect(document.getElementById(appliedDescription ?? '')?.textContent).toBe(
      'This term and Campus range is already applied.',
    );
  });

  it('blocks Search with every unavailable applied target and ignores unselected targets', () => {
    const terms = [visible('72026', 0), visible('92026', 1)];
    const ready = status('PUBLIC', terms, [
      target('72026', 'NB'), target('72026', 'NK'), target('72026', 'CM'),
    ]);
    const view = render(<ScopeHarness catalogDiscovery={discovery(['72026', '92026'])} serviceStatus={ready} />);
    fireEvent.click(screen.getByRole('checkbox', { name: /^NB/u }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(false);

    view.rerender(<ScopeHarness catalogDiscovery={discovery(['72026', '92026'])} serviceStatus={status('PUBLIC', terms, [
      target('72026', 'NB', { snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', usable: false }),
      target('72026', 'NK', { snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', usable: false }),
      target('72026', 'CM'),
    ])} />);
    const search = screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement;
    expect(search.disabled).toBe(true);
    const reason = document.getElementById(search.getAttribute('aria-describedby') ?? '')?.textContent ?? '';
    expect(reason).toContain('72026/NB');
    expect(reason).not.toContain('72026/NK');
  });

  it('uses one button for Apply or Local Pull and disables it while the term is already requested', async () => {
    const terms = [
      visible('02026', -2, true), visible('12026', -1, true), visible('72026', 0),
      visible('92026', 1), visible('02027', 2, true),
    ];
    const onPull = vi.fn(async () => undefined);
    const ready = status('LOCAL', terms, [
      target('72026', 'NB'),
      target('12026', 'NB', {
        snapshotAvailability: 'UNREQUESTED', usable: false, catalogContentVersion: null, lastCompleteAt: null,
      }),
    ]);
    const view = render(
      <ScopeHarness
        catalogDiscovery={discovery(terms.map(({ term }) => term), ['NB'])}
        local
        onPull={onPull}
        serviceStatus={ready}
      />,
    );
    fireEvent.click(screen.getByRole('radio', { name: /Spring 2026/u }));
    const pull = screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement;
    expect(pull.disabled).toBe(false);
    fireEvent.click(pull);
    expect(onPull).toHaveBeenCalledWith('12026');
    await waitFor(() => expect(pull.disabled).toBe(true));
    expect(pull.textContent).toBe('Pull');
    expect(pull.getAttribute('aria-busy')).toBe('true');
    expect(screen.queryByRole('button', { name: /Continue|Loading|Applied/u })).toBeNull();

    view.rerender(
      <ScopeHarness
        catalogDiscovery={discovery(terms.map(({ term }) => term), ['NB'])}
        initialCandidate={{ term: '12026', campuses: [] }}
        local
        onPull={onPull}
        serviceStatus={status('LOCAL', terms, [
          target('12026', 'NB', {
            snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
            workState: 'QUEUED',
            stage: 'CATALOG_FETCH',
            usable: false,
            catalogContentVersion: null,
            lastCompleteAt: null,
          }),
          target('12026', 'NK', {
            snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
            workState: 'RUNNING',
            stage: 'OPEN_FETCH',
            usable: false,
            catalogContentVersion: null,
            lastCompleteAt: null,
          }),
          target('12026', 'CM', {
            snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
            workState: 'RETRY_WAIT',
            usable: false,
            catalogContentVersion: null,
            lastCompleteAt: null,
            error: targetError('TERMINAL_PARSE'),
          }),
        ])}
      />,
    );
    expect((screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement).disabled).toBe(false);
    expect(document.querySelector('[data-scope-cell="campus-NB"] [data-stage="CATALOG_FETCH"]')?.textContent)
      .toContain('downloading the course catalogue');
    expect(document.querySelector('[data-scope-cell="campus-NK"] [data-stage="OPEN_FETCH"]')?.textContent)
      .toContain('reading open-seat status');
    expect(document.querySelector('[data-scope-cell="campus-CM"]')?.textContent).toContain('TERMINAL_PARSE');
  });

  it('keeps term Pull enabled when one Campus is active but other targets still need work', () => {
    const terms = [
      visible('02026', -2, true), visible('12026', -1, true), visible('72026', 0),
      visible('92026', 1), visible('02027', 2, true),
    ];
    render(<ScopeHarness
      catalogDiscovery={discovery(terms.map(({ term }) => term), ['NB', 'NK', 'CM'])}
      initialCandidate={{ term: '12026', campuses: [] }}
      local
      serviceStatus={status('LOCAL', terms, [target('12026', 'NB', {
        snapshotAvailability: 'NO_COMPLETE_SNAPSHOT', workState: 'QUEUED', stage: 'CATALOG_FETCH',
        usable: false, catalogContentVersion: null, lastCompleteAt: null,
      })])}
    />);
    expect((screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('deduplicates a terminal retry until active work or genuinely new terminal evidence arrives', async () => {
    const terms = [
      visible('02026', -2, true), visible('12026', -1, true), visible('72026', 0),
      visible('92026', 1), visible('02027', 2, true),
    ];
    const terminalTargets = ['NB', 'NK', 'CM'].map((campus) => target('12026', campus, {
      snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
      workState: 'RETRY_WAIT',
      usable: false,
      catalogContentVersion: null,
      lastCompleteAt: null,
      nextRetryAt: null,
      error: targetError('TERMINAL_INITIAL'),
    }));
    const onPull = vi.fn(async () => undefined);
    const view = render(<ScopeHarness
      catalogDiscovery={discovery(terms.map(({ term }) => term), ['NB', 'NK', 'CM'])}
      initialCandidate={{ term: '12026', campuses: [] }}
      local
      onPull={onPull}
      serviceStatus={status('LOCAL', terms, terminalTargets)}
    />);

    const pull = screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement;
    expect(pull.disabled).toBe(false);
    fireEvent.click(pull);
    await waitFor(() => expect(pull.disabled).toBe(true));
    fireEvent.click(pull);
    expect(onPull).toHaveBeenCalledTimes(1);

    const activeTargets = ['NB', 'NK', 'CM'].map((campus) => target('12026', campus, {
      snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
      workState: campus === 'NB' ? 'QUEUED' : 'RUNNING',
      stage: 'CATALOG_FETCH',
      usable: false,
      catalogContentVersion: null,
      lastCompleteAt: null,
      nextRetryAt: null,
      error: null,
    }));
    view.rerender(<ScopeHarness
      catalogDiscovery={discovery(terms.map(({ term }) => term), ['NB', 'NK', 'CM'])}
      initialCandidate={{ term: '12026', campuses: [] }}
      local
      onPull={onPull}
      serviceStatus={status('LOCAL', terms, activeTargets)}
    />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement).disabled).toBe(true));

    const nextTerminalTargets = terminalTargets.map((entry) => ({
      ...entry,
      error: { ...targetError('TERMINAL_NEXT'), traceId: 'trace-next-generation' },
    }));
    view.rerender(<ScopeHarness
      catalogDiscovery={discovery(terms.map(({ term }) => term), ['NB', 'NK', 'CM'])}
      initialCandidate={{ term: '12026', campuses: [] }}
      local
      onPull={onPull}
      serviceStatus={status('LOCAL', terms, nextTerminalTargets)}
    />);
    await waitFor(() => expect((screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement).disabled).toBe(false));
  });

  it('never projects ONLINE aliases into the Campus UI', () => {
    const terms = [visible('72026', 0), visible('92026', 1)];
    render(
      <ScopeHarness
        catalogDiscovery={discovery(['72026', '92026'], ['NB', 'ONLINE_NB', 'ONLINE_NK', 'ONLINE_CM'])}
        serviceStatus={status('PUBLIC', terms, [
          target('72026', 'NB'),
          target('72026', 'ONLINE_NB'),
          target('72026', 'ONLINE_NK'),
          target('72026', 'ONLINE_CM'),
        ])}
      />,
    );
    expect(screen.getAllByRole('checkbox')).toHaveLength(3);
    expect(document.querySelector('.query-scope')?.textContent).not.toMatch(/ONLINE_(NB|NK|CM)/u);
  });

  it('keeps unpublished and unknown Local Pull visible and disabled without a nearby reason', () => {
    const terms = [
      visible('02026', -2, true), visible('12026', -1, true), visible('72026', 0),
      visible('92026', 1), { ...visible('02027', 2, true), publication: 'UNPUBLISHED' },
    ];
    const view = render(
      <ScopeHarness
        catalogDiscovery={discovery(['72026', '92026'], ['NB', 'NK', 'CM'])}
        initialCandidate={{ term: '02027', campuses: [] }}
        local
        serviceStatus={status('LOCAL', terms, [target('72026', 'NB')])}
      />,
    );
    const pull = screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement;
    expect(pull.disabled).toBe(true);
    const unpublishedReason = document.getElementById(pull.getAttribute('aria-describedby') ?? '');
    expect(unpublishedReason?.textContent).toBe('Not published by Rutgers');
    expect((unpublishedReason as HTMLElement | null)?.hidden).toBe(true);
    expect(pull.getAttribute('title')).toBeNull();
    expect(document.querySelector('[data-scope-cell="scope-action"] .query-scope__status')).toBeNull();

    const unknownTerms = terms.map((term) => term.term === '02027'
      ? { ...term, publication: 'UNKNOWN' as const }
      : term);
    view.rerender(<ScopeHarness
      catalogDiscovery={discovery(['72026', '92026'], ['NB', 'NK', 'CM'])}
      initialCandidate={{ term: '02027', campuses: [] }}
      local
      serviceStatus={status('LOCAL', unknownTerms, [target('72026', 'NB')])}
    />);
    const unknownPull = screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement;
    expect(unknownPull.disabled).toBe(true);
    const unknownReason = document.getElementById(unknownPull.getAttribute('aria-describedby') ?? '');
    expect(unknownReason?.textContent).toBe('Publication status unknown');
    expect((unknownReason as HTMLElement | null)?.hidden).toBe(true);
    expect(document.querySelector('[data-scope-cell="scope-action"] .query-scope__status')).toBeNull();
  });
});
