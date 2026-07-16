// @vitest-environment jsdom

import { useEffect, useState } from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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
import { QueryScopeControl } from '../src/ui/shared/search/QueryScopeControl';
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
    discovered: true,
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

const staleRequest: CourseQueryRequestV1 = {
  filters: {
    contractVersion: 2,
    values: {
      ...createNeutralFilterState('12026'),
      campuses: ['NB'],
    },
  },
  page: { page: 1, pageSize: 25 },
  sort: { direction: 'DESCENDING', field: 'RELEVANCE' },
};

const staleResponse: CourseQueryResponseV1 = {
  contractVersion: 2,
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
  it('never falls back to the unbounded Discovery term list before Status V2 arrives', () => {
    render(
      <BcspI18nProvider initialLocale="en-US">
        <QueryScopeControl
          applied={null}
          candidate={{ term: null, campuses: [] }}
          discovery={discovery(['12025', '72025', '92025', '12026', '72026', '92026'])}
          onApply={vi.fn()}
          onCandidateChange={vi.fn()}
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
      target('72026', 'NB'), target('72026', 'NK'), target('92026', 'NB'),
    ]);
    const first = render(
      <ScopeHarness
        catalogDiscovery={discovery(publicTerms.map(({ term }) => term))}
        serviceStatus={publicStatus}
      />,
    );
    expect(screen.getAllByRole('radio')).toHaveLength(2);
    expect(screen.getAllByRole('checkbox')).toHaveLength(2);
    expect(screen.getAllByRole('checkbox').every((input) => !(input as HTMLInputElement).checked)).toBe(true);
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
    first.unmount();

    const localTerms = [
      visible('12026', -2, true),
      visible('72026', -1, true),
      visible('92026', 0),
      visible('02027', 1),
      visible('12027', 2, true),
    ];
    render(
      <ScopeHarness
        catalogDiscovery={discovery(localTerms.map(({ term }) => term))}
        local
        serviceStatus={status('LOCAL', localTerms, [target('92026', 'NB')])}
      />,
    );
    expect(screen.getAllByRole('radio')).toHaveLength(5);
    expect(screen.getAllByRole('checkbox').every((input) => !(input as HTMLInputElement).checked)).toBe(true);
  });

  it('uses a localized term label when an unpublished visible term has no Discovery target', () => {
    const terms = [
      visible('72026', 0),
      { ...visible('92026', 1), discovered: false },
    ];
    render(
      <ScopeHarness
        catalogDiscovery={discovery(['72026'], ['NB'])}
        serviceStatus={status('PUBLIC', terms, [target('72026', 'NB')])}
      />,
    );

    expect(screen.getByRole('radio', { name: /Fall 2026/u })).toBeTruthy();
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

  it('enables Apply for a partial-ready selection and keeps READY plus retry usable', () => {
    const terms = [visible('72026', 0), visible('92026', 1)];
    const serviceStatus = status('PUBLIC', terms, [
      target('72026', 'NB'),
      target('72026', 'NK', {
        snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
        workState: 'RETRY_WAIT',
        usable: false,
        catalogContentVersion: null,
        lastCompleteAt: null,
        nextRetryAt: '2026-07-17T00:01:00Z',
      }),
      target('72026', 'CM', {
        workState: 'RETRY_WAIT',
        nextRetryAt: '2026-07-17T00:01:00Z',
      }),
    ]);
    render(
      <ScopeHarness
        catalogDiscovery={discovery(['72026', '92026'], ['NB', 'NK', 'CM', 'ONLINE_NB'])}
        serviceStatus={serviceStatus}
      />,
    );

    const campusGroup = screen.getByRole('group', { name: '02 Campus' });
    const nb = within(campusGroup).getByRole('checkbox', { name: /New Brunswick/u });
    const nk = within(campusGroup).getByRole('checkbox', { name: /Newark/u });
    const cm = within(campusGroup).getByRole('checkbox', { name: /CM/u });
    expect((nk as HTMLInputElement).disabled).toBe(true);
    expect((cm as HTMLInputElement).disabled).toBe(false);
    expect(screen.getByText(/Refresh failed; retry scheduled/u)).toBeTruthy();
    fireEvent.click(nb);
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    expect(screen.getByText('Current range')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('uses one button for Apply or Local Pull and disables it while the term is already requested', async () => {
    const terms = [
      visible('12026', -2, true), visible('72026', -1, true), visible('92026', 0),
      visible('02027', 1), visible('12027', 2, true),
    ];
    const onPull = vi.fn(async () => undefined);
    const ready = status('LOCAL', terms, [
      target('92026', 'NB'),
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
    expect(screen.queryByRole('button', { name: /Continue|Loading|Applied/u })).toBeNull();

    view.rerender(
      <ScopeHarness
        catalogDiscovery={discovery(terms.map(({ term }) => term), ['NB'])}
        initialCandidate={{ term: '12026', campuses: [] }}
        local
        onPull={onPull}
        serviceStatus={status('LOCAL', terms, [target('12026', 'NB', {
          snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
          workState: 'QUEUED',
          stage: 'CATALOG_FETCH',
          usable: false,
          catalogContentVersion: null,
          lastCompleteAt: null,
        })])}
      />,
    );
    expect((screen.getByRole('button', { name: 'Pull' }) as HTMLButtonElement).disabled).toBe(true);
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
    const campusGroup = screen.getByRole('group', { name: '02 Campus' });
    expect(within(campusGroup).getAllByRole('checkbox')).toHaveLength(1);
    expect(campusGroup.textContent).not.toMatch(/ONLINE_(NB|NK|CM)/u);
  });
});
