// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi, type Mock } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  CourseQueryRequestV1,
  CourseQueryResponseV1,
  FilterStateV1,
  FilterOptionsResponseV2,
  FilterSchemaV1,
  ProductApiPort,
  ProductRuntimePort,
  ServiceStatusV2,
} from '../src/ui/shared/product';
import { ProductClientError } from '../src/ui/shared/product';
import { createNeutralFilterState } from '../src/ui/shared/product';
import {
  SearchSessionProvider,
  SearchWorkspace,
  useSearchSession,
} from '../src/ui/shared/search';
import { AppRouterProvider } from '../src/ui/shared/routing';
import type { ShellDataState } from '../src/ui/shared/shell';

const SCHEMA = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;
const TERM = '72026';
const NEXT_TERM = '92026';
const known = (value: string) => ({
  knowledge: 'KNOWN',
  presence: { presence: 'PRESENT', value },
} as const);

function discovery(): CatalogDiscoveryResponseV1 {
  const point = {
    contentVersion: 1,
    observationId: '20000000-0000-4000-8000-000000000001',
    observedAt: '2026-07-15T00:00:00Z',
  } as const;
  const provenance = {
    ...point,
    payloadDigest: 'a'.repeat(64),
    sourceId: 'synthetic-selector',
    sourceKind: 'SELECTOR',
  } as const;
  return {
    contractVersion: 1,
    observedAt: point.observedAt,
    sources: [],
    status: {
      availability: 'CURRENT', error: null, isStale: false,
      lastSuccess: point, latestAttempt: point,
    },
    coreCodeDictionaries: [],
    subjects: [{
      code: '198',
      label: known('Computer Science'),
      provenance: { kind: 'DISCOVERY', discovery: provenance },
      target: { campus: 'NB', term: TERM },
    }],
    targets: [{
      campusLabel: known('New Brunswick'),
      key: { campus: 'NB', term: TERM },
      provenance,
      termLabel: known('This upstream label must not render'),
    }],
  };
}

const shellState: Extract<ShellDataState, { status: 'READY' }> = {
  discovery: discovery(),
  discoveryState: 'CURRENT',
  filterCount: 18,
  filterSchema: SCHEMA,
  status: 'READY',
};

const emptyCourses: CourseQueryResponseV1 = {
  contractVersion: 3,
  items: [],
  page: { page: 1, pageSize: 25, total: 0, totalPages: 0 },
};

function serviceStatus(usable: boolean, retrying = false): ServiceStatusV2 {
  return {
    contractVersion: 2,
    observedAt: '2026-07-15T00:00:01Z',
    runtime: 'LOCAL',
    level: usable ? 'PARTIALLY_READY' : 'INITIALIZING',
    discovery: shellState.discovery.status,
    termWindow: {
      currentTerm: TERM,
      nextTerm: NEXT_TERM,
      visibleTerms: [
        { term: '02026', relativeOffset: -2, publication: 'PUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
        { term: '12026', relativeOffset: -1, publication: 'PUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
        { term: TERM, relativeOffset: 0, publication: 'PUBLISHED', autoManaged: true, manualPullAllowed: false, watchable: true },
        { term: NEXT_TERM, relativeOffset: 1, publication: 'UNPUBLISHED', autoManaged: true, manualPullAllowed: false, watchable: true },
        { term: '02027', relativeOffset: 2, publication: 'UNPUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
      ],
    },
    automaticTermSummaries: [
      { term: TERM, readyTargetCount: usable ? 1 : 0, totalTargetCount: 1 },
      { term: NEXT_TERM, readyTargetCount: 0, totalTargetCount: 0 },
    ],
    operations: usable ? [] : [{
      target: { campus: 'NB', term: TERM },
      stage: 'CATALOG_FETCH',
      startedAt: '2026-07-15T00:00:00Z',
    }],
    targets: [{
      target: { campus: 'NB', term: TERM },
      primary: true,
      snapshotAvailability: usable ? 'READY' : 'NO_COMPLETE_SNAPSHOT',
      workState: retrying ? 'RETRY_WAIT' : usable ? 'IDLE' : 'RUNNING',
      stage: usable ? null : 'CATALOG_FETCH',
      usable,
      catalogContentVersion: usable ? 1 : null,
      lastCompleteAt: usable ? '2026-07-15T00:00:00Z' : null,
      nextRetryAt: retrying ? '2026-07-15T00:01:00Z' : null,
      error: null,
    }],
    issues: [],
  };
}

function productRuntime(overrides: Partial<ProductApiPort>): ProductRuntimePort {
  return {
    dispose() {},
    product: overrides as ProductApiPort,
    watch: {} as ProductRuntimePort['watch'],
  };
}

function renderWorkspace(
  runtime: ProductRuntimePort,
  path = '/',
  status: ServiceStatusV2 | null = serviceStatus(true),
) {
  return render(
    <BcspI18nProvider initialLocale="en-US">
      <AppRouterProvider initialPath={path}>
        <SearchWorkspace runtime={runtime} serviceStatus={status} shellState={shellState} />
      </AppRouterProvider>
    </BcspI18nProvider>,
  );
}

interface StoredWorkspaceProps {
  readonly initialFilters?: FilterStateV1 | undefined;
  readonly onFiltersChange?: ((filters: FilterStateV1) => void) | undefined;
  readonly runtime: ProductRuntimePort;
  readonly status?: ServiceStatusV2 | null;
}

/** Renders the workspace the way LocalApplication does: with stored filters and
 * a session probe so restore and diagnosis effects can be observed. */
function storedWorkspace({ initialFilters, onFiltersChange, runtime, status = serviceStatus(true) }: StoredWorkspaceProps) {
  return (
    <BcspI18nProvider initialLocale="en-US">
      <AppRouterProvider initialPath="/">
        <SearchSessionProvider>
          <SearchWorkspace
            initialFilters={initialFilters}
            onFiltersChange={onFiltersChange}
            runtime={runtime}
            serviceStatus={status}
            shellState={shellState}
          />
          <SearchSessionProbe />
        </SearchSessionProvider>
      </AppRouterProvider>
    </BcspI18nProvider>
  );
}

function applyNewBrunswick(): void {
  fireEvent.click(screen.getByRole('checkbox', { name: /NB/u }));
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
}

type SearchCoursesMock = Mock<ProductApiPort['searchCourses']>;

/** Main searches use pageSize 25; diagnosis probes use pageSize 1. */
function mainSearches(searchCourses: SearchCoursesMock): CourseQueryRequestV1[] {
  return searchCourses.mock.calls
    .map(([request]) => request)
    .filter((request) => request.page.pageSize === 25);
}

function probeSearches(searchCourses: SearchCoursesMock): CourseQueryRequestV1[] {
  return searchCourses.mock.calls
    .map(([request]) => request)
    .filter((request) => request.page.pageSize === 1);
}

function probeResponse(total: number): CourseQueryResponseV1 {
  return {
    contractVersion: 3,
    items: [],
    page: { page: 1, pageSize: 1, total, totalPages: total },
  };
}

function notReadyError(): ProductClientError {
  return new ProductClientError(503, {
    protocolVersion: 1,
    error: {
      code: 'SEARCH_DATA_NOT_READY',
      messageKey: 'error.search_data_not_ready',
      traceId: 'trace-probe',
      details: [],
    },
  });
}

/** Waits until the diagnosis for the current empty result has settled. */
async function diagnosisSettled(container: HTMLElement): Promise<void> {
  await waitFor(() => expect(container.querySelector(
    '[data-diagnosis-state="READY"], [data-diagnosis-state="UNAVAILABLE"]',
  )).not.toBeNull());
}

function SearchSessionProbe() {
  const session = useSearchSession();
  return (
    <output data-testid="search-session-probe">
      {JSON.stringify({
        appliedScope: session.state.appliedScope,
        hasSubmittedQuery: session.state.lastSubmittedRequest !== null,
        hasSuccessfulQuery: session.state.lastSuccessfulResponse !== null,
        successfulIncludeIncomplete: session.state.lastSuccessfulRequest?.filters.values.includeIncomplete ?? null,
      })}
    </output>
  );
}

function readSearchSessionProbe() {
  return JSON.parse(screen.getByTestId('search-session-probe').textContent ?? '{}') as {
    readonly appliedScope: { readonly campuses: readonly string[]; readonly term: string } | null;
    readonly hasSubmittedQuery: boolean;
    readonly hasSuccessfulQuery: boolean;
    readonly successfulIncludeIncomplete: FilterStateV1['includeIncomplete'] | null;
  };
}

afterEach(cleanup);

describe('RC3 unified Course workspace controller', () => {
  it('keeps scope controls available while disabling 03–18 until a selected target is READY', () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    renderWorkspace(productRuntime({ searchCourses }), '/', serviceStatus(false));

    expect((screen.getByRole('radio', { name: /Summer 2026/u }) as HTMLInputElement).disabled).toBe(false);
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByLabelText('Term')).toBeNull();
    expect(screen.getByText(/Loading complete course and availability data/u)).toBeTruthy();
  });

  it('starts with current term and no auto-selected Campus, then enables one partial-ready target', () => {
    renderWorkspace(productRuntime({}));
    expect((screen.getByRole('radio', { name: /Summer 2026/u }) as HTMLInputElement).checked).toBe(true);
    const campus = screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement;
    expect(campus.checked).toBe(false);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(true);

    applyNewBrunswick();
    expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('keeps a READY snapshot queryable while its refresh is in retry wait', () => {
    renderWorkspace(productRuntime({}), '/', serviceStatus(true, true));
    applyNewBrunswick();
    expect(screen.getByText(/Refresh failed; retry scheduled/u)).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('invalidates an in-flight response when externally applied filters change the scope state', async () => {
    let resolveSearch: ((response: CourseQueryResponseV1) => void) | undefined;
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>((_request, signal) => (
      new Promise<CourseQueryResponseV1>((resolve) => {
        resolveSearch = resolve;
        expect(signal?.aborted).toBe(false);
      })
    ));
    const runtime = productRuntime({ searchCourses });
    const initialFilters = createNeutralFilterState(TERM);
    const application = (filters: typeof initialFilters) => (
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchWorkspace
            initialFilters={filters}
            runtime={runtime}
            serviceStatus={serviceStatus(true)}
            shellState={shellState}
          />
        </AppRouterProvider>
      </BcspI18nProvider>
    );
    const view = render(application(initialFilters));
    applyNewBrunswick();
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(1));
    const signal = searchCourses.mock.calls[0]?.[1];

    view.rerender(application({
      ...createNeutralFilterState(TERM),
      campuses: ['NB'],
      courseNumberBands: [900],
    }));
    await waitFor(() => expect(signal?.aborted).toBe(true));
    expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();

    await act(async () => {
      resolveSearch?.({
        ...emptyCourses,
        page: { ...emptyCourses.page, total: 4559, totalPages: 183 },
      });
    });
    expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
    expect(view.container.textContent).not.toContain('4,559');
  });

  it('rejects an invalid target-bound definition without partially applying or deleting values', async () => {
    const onFiltersChange = vi.fn();
    const filterOptions = vi.fn<ProductApiPort['filterOptions']>();
    const validateDynamicFilters = vi.fn<ProductApiPort['validateDynamicFilters']>().mockResolvedValue({
      contractVersion: 3,
      targetVersions: [{ target: { term: TERM, campus: 'NB' }, contentVersion: 1 }],
      invalidValues: [
        { field: 'FLT-C03', value: '999' },
        { field: 'FLT-C04', value: 'retired-token' },
        { field: 'FLT-C05', value: '900' },
        { field: 'FLT-C06', value: 'G' },
        { field: 'FLT-C08', value: 'BAD-CORE' },
        { field: 'FLT-S05', value: 'Retired Instructor' },
        { field: 'FLT-S07', value: 'OLD-CAMPUS' },
        { field: 'FLT-S09', value: 'Z' },
      ],
    });
    const initialFilters = {
      ...createNeutralFilterState(TERM),
      keywords: ['data', 'retired-token'],
      subjects: ['198', '999'],
      courseNumberBands: [200, 900],
      levels: ['U', 'G'],
      core: { codes: ['CC', 'BAD-CORE'], mode: 'ANY' as const },
      instructors: ['Ada Lovelace', 'Retired Instructor'],
      meetingLocations: { locations: ['CAC', 'OLD-CAMPUS'], mode: 'ANY_MEETING' as const },
      examCodes: ['A', 'Z'],
    };
    render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchWorkspace
            initialFilters={initialFilters}
            onFiltersChange={onFiltersChange}
            runtime={productRuntime({ filterOptions, validateDynamicFilters })}
            serviceStatus={serviceStatus(true)}
            shellState={shellState}
          />
        </AppRouterProvider>
      </BcspI18nProvider>,
    );

    fireEvent.click(screen.getByRole('checkbox', { name: /NB/u }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));

    await waitFor(() => expect(validateDynamicFilters).toHaveBeenCalledTimes(1));
    const alert = (await screen.findByRole('alert')).textContent ?? '';
    for (const invalid of ['999', 'retired-token', '900', 'G', 'BAD-CORE', 'Retired Instructor', 'OLD-CAMPUS', 'Z']) {
      expect(alert).toContain(invalid);
    }
    expect(alert).toContain('Subject: 999');
    expect(alert).toContain('Keyword match: retired-token');
    expect(alert).not.toContain('FLT-');
    expect(onFiltersChange).not.toHaveBeenCalled();
    expect(screen.getAllByText(/retired-token/u).length).toBeGreaterThan(0);
    expect(filterOptions).not.toHaveBeenCalled();
    expect(validateDynamicFilters.mock.calls[0]?.[0]).toEqual({
      filters: expect.objectContaining({
        contractVersion: 3,
        values: expect.objectContaining({
          campuses: ['NB'], courseNumberBands: [200, 900], examCodes: ['A', 'Z'], levels: ['G', 'U'],
          subjects: ['198', '999'],
        }),
      }),
    });
  });

  it('keeps the old applied scope, results, and Search usable when exact validation rejects a new candidate', async () => {
    const baseStatus = serviceStatus(true);
    const nb = baseStatus.targets[0];
    if (nb === undefined) throw new Error('Expected NB fixture target.');
    const multiCampusStatus: ServiceStatusV2 = {
      ...baseStatus,
      targets: [nb, { ...nb, primary: false, target: { term: TERM, campus: 'NK' } }],
    };
    const validateDynamicFilters = vi.fn<ProductApiPort['validateDynamicFilters']>()
      .mockResolvedValueOnce({ contractVersion: 3, invalidValues: [], targetVersions: [] })
      .mockResolvedValueOnce({
        contractVersion: 3,
        invalidValues: [{ field: 'FLT-C04', value: 'retired-token' }],
        targetVersions: [],
      });
    const filterOptions = vi.fn<ProductApiPort['filterOptions']>();
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const onFiltersChange = vi.fn();
    render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchWorkspace
            initialFilters={{ ...createNeutralFilterState(TERM), keywords: ['retired-token'] }}
            onFiltersChange={onFiltersChange}
            runtime={productRuntime({ filterOptions, searchCourses, validateDynamicFilters })}
            serviceStatus={multiCampusStatus}
            shellState={shellState}
          />
        </AppRouterProvider>
      </BcspI18nProvider>,
    );

    fireEvent.click(screen.getByRole('checkbox', { name: /^NB/u }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy());
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();

    fireEvent.click(screen.getByRole('checkbox', { name: /^NK/u }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    await waitFor(() => expect(
      screen.getAllByRole('alert').some((alert) => alert.textContent?.includes('retired-token')),
    ).toBe(true));
    expect(validateDynamicFilters).toHaveBeenCalledTimes(2);
    expect(onFiltersChange).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('heading', { name: 'No matching records' })).toBeTruthy();
    const search = screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement;
    expect(search.disabled).toBe(false);
    fireEvent.click(search);
    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(2));
    expect(mainSearches(searchCourses)[1]?.filters.values.campuses).toEqual(['NB']);
    await diagnosisSettled(document.body);
  });

  it('keeps old results during candidate edits, then clears the full query session after a successful scope change without auto-searching', async () => {
    const baseStatus = serviceStatus(true);
    const nb = baseStatus.targets[0];
    if (nb === undefined) throw new Error('Expected NB fixture target.');
    const multiCampusStatus: ServiceStatusV2 = {
      ...baseStatus,
      targets: [nb, { ...nb, primary: false, target: { term: TERM, campus: 'NK' } }],
    };
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const filterOptions = vi.fn<ProductApiPort['filterOptions']>().mockImplementation(async (request) => ({
      contractVersion: 3,
      field: request.field,
      options: [],
      targetVersions: [],
      truncated: false,
    }));
    const runtime = productRuntime({ filterOptions, searchCourses });
    const view = render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchSessionProvider>
            <SearchWorkspace
              runtime={runtime}
              serviceStatus={multiCampusStatus}
              shellState={shellState}
            />
            <SearchSessionProbe />
          </SearchSessionProvider>
        </AppRouterProvider>
      </BcspI18nProvider>,
    );

    applyNewBrunswick();
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(searchCourses).toHaveBeenCalledTimes(1);
    expect(readSearchSessionProbe()).toMatchObject({
      appliedScope: { campuses: ['NB'], term: TERM },
      hasSubmittedQuery: true,
      hasSuccessfulQuery: true,
    });

    fireEvent.click(screen.getByRole('checkbox', { name: /^NK/u }));
    expect(screen.getByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(false);
    expect(searchCourses).toHaveBeenCalledTimes(1);
    expect(readSearchSessionProbe().appliedScope?.campuses).toEqual(['NB']);

    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy());
    expect(screen.queryByRole('heading', { name: 'No matching records' })).toBeNull();
    expect(view.container.querySelector('.bcsp-search-workspace')?.hasAttribute('data-results-visible')).toBe(false);
    expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
    expect(screen.getByRole('heading', { name: 'Build a precise search' })).toBeTruthy();
    expect(view.container.querySelector('.bcsp-search-workspace__scope')
      ?.closest('.bcsp-search-workspace__filters')).not.toBeNull();
    // Spec v2 section 7 #6: the single Search cell lives in the rail's sticky
    // submit footer (the last direct child of the rail) and stays form-associated.
    const searchCells = view.container.querySelectorAll('[data-scope-cell="search"]');
    expect(searchCells).toHaveLength(1);
    const submitFooter = searchCells[0]?.closest('.bcsp-search-workspace__submit');
    expect(submitFooter).not.toBeNull();
    expect(submitFooter?.parentElement?.classList.contains('bcsp-search-workspace__filters')).toBe(true);
    expect(submitFooter?.parentElement?.lastElementChild).toBe(submitFooter);
    expect(searchCells[0]?.querySelector('button')?.getAttribute('form'))
      .toBe(view.container.querySelector('form.filter-panel')?.getAttribute('id'));
    expect(searchCourses).toHaveBeenCalledTimes(1);
    expect(readSearchSessionProbe()).toMatchObject({
      appliedScope: { campuses: ['NB', 'NK'], term: TERM },
      hasSubmittedQuery: false,
      hasSuccessfulQuery: false,
    });
  });

  it('returns to the initialization prompt when readiness races with SEARCH_DATA_NOT_READY', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockRejectedValue(
      new ProductClientError(503, {
        protocolVersion: 1,
        error: {
          code: 'SEARCH_DATA_NOT_READY',
          messageKey: 'error.catalog_not_ready',
          traceId: 'trace-search-race',
          details: [],
        },
      }),
    );
    const view = renderWorkspace(productRuntime({ searchCourses }));
    applyNewBrunswick();
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(1));
    expect(await screen.findByRole('heading', { name: 'Opening the catalog console' })).toBeTruthy();
    expect(view.container.querySelector('[data-query-state="not_ready"]')).not.toBeNull();
    expect(screen.queryByRole('heading', { name: 'Search failed' })).toBeNull();
  });

  it('submits one typed Course query with combined Course and same-Section filters', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const filterOptions = vi.fn(async (
      request: Parameters<ProductApiPort['filterOptions']>[0],
    ): Promise<FilterOptionsResponseV2> => ({
      contractVersion: 3,
      field: request.field,
      options: request.field === 'KEYWORD'
        ? [{ value: 'data', label: 'data' }]
        : request.field === 'INSTRUCTOR'
          ? [{ value: 'Ada Lovelace', label: 'Ada Lovelace' }]
          : [],
      targetVersions: [],
      truncated: false,
    }));
    renderWorkspace(productRuntime({ filterOptions, searchCourses }));
    applyNewBrunswick();

    const keyword = screen.getByRole('combobox', { name: 'Keyword match' });
    fireEvent.change(keyword, { target: { value: 'data' } });
    await screen.findByRole('option', { name: 'data' });
    fireEvent.keyDown(keyword, { key: 'Enter' });
    fireEvent.click(within(screen.getByRole('group', { name: 'Open status' }))
      .getByRole('checkbox', { name: 'Open' }));
    const instructor = screen.getByRole('combobox', { name: 'Instructor' });
    fireEvent.change(instructor, { target: { value: 'ADA' } });
    await screen.findByRole('option', { name: 'Ada Lovelace' });
    fireEvent.keyDown(instructor, { key: 'Enter' });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(1));
    expect(mainSearches(searchCourses)[0]?.filters.values).toMatchObject({
      campuses: ['NB'], instructors: ['Ada Lovelace'], keywords: ['data'],
      openStatuses: ['OPEN'], term: TERM,
    });
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await diagnosisSettled(document.body);
    expect(mainSearches(searchCourses)).toHaveLength(1);
  });

  it('keeps Section constraints inside Course search and never calls the legacy Section endpoint', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const searchSections = vi.fn<ProductApiPort['searchSections']>();
    renderWorkspace(productRuntime({ searchCourses, searchSections }));
    applyNewBrunswick();
    fireEvent.change(screen.getByLabelText('Section indexes'), { target: { value: '12345' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Section indexes' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(1));
    expect(mainSearches(searchCourses)[0]?.filters.values.sectionIndexes).toEqual(['12345']);
    expect(searchSections).not.toHaveBeenCalled();
    await diagnosisSettled(document.body);
    expect(probeSearches(searchCourses).every((request) => request.page.pageSize === 1)).toBe(true);
    expect(searchSections).not.toHaveBeenCalled();
  });

  it('keeps an invalid Section index local, identifies its row, and focuses its control', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const view = renderWorkspace(productRuntime({ searchCourses }));
    applyNewBrunswick();
    const sectionIndexInput = screen.getByLabelText('Section indexes');
    fireEvent.change(sectionIndexInput, { target: { value: '1234' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Section indexes' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    const row = view.container.querySelector<HTMLElement>('[data-filter-row="FLT-S01"]');
    expect(row).not.toBeNull();
    await waitFor(() => {
      expect(searchCourses).not.toHaveBeenCalled();
      expect(row?.dataset.filterError).toBe('true');
      expect(document.activeElement).toBe(sectionIndexInput);
    });
  });

  it('reloads a safe direct Section URL and rejects an invalid one before a request', async () => {
    const sectionDetail = vi.fn<ProductApiPort['sectionDetail']>()
      .mockImplementation(() => new Promise(() => undefined));
    const first = renderWorkspace(productRuntime({ sectionDetail }), `/sections/${TERM}/NB/12345`);
    await waitFor(() => expect(sectionDetail).toHaveBeenCalledTimes(1));
    expect(sectionDetail.mock.calls[0]?.[0]).toEqual({
      contractVersion: 3,
      key: { campus: 'NB', index: '12345', term: TERM },
    });
    first.unmount();

    const invalidDetail = vi.fn<ProductApiPort['sectionDetail']>();
    renderWorkspace(productRuntime({ sectionDetail: invalidDetail }), `/sections/${TERM}/NB/not-an-index`);
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(invalidDetail).not.toHaveBeenCalled();
  });
});

describe('stored scope restore on load', () => {
  it('restores a stored ready scope on load without Apply or revalidation', async () => {
    const validateDynamicFilters = vi.fn<ProductApiPort['validateDynamicFilters']>();
    const filterOptions = vi.fn<ProductApiPort['filterOptions']>().mockImplementation(async (request) => ({
      contractVersion: 3, field: request.field, options: [], targetVersions: [], truncated: false,
    }));
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const onFiltersChange = vi.fn();
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['data'] },
      onFiltersChange,
      runtime: productRuntime({ filterOptions, searchCourses, validateDynamicFilters }),
    }));

    expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy();
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('radio', { name: /Summer 2026/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(false);
    expect(screen.getByRole('combobox', { name: 'Keyword match' }).matches(':disabled')).toBe(false);
    expect(view.container.querySelector('[data-filter-chip="FLT-C04"]')?.textContent).toContain('data');
    expect(validateDynamicFilters).not.toHaveBeenCalled();
    expect(onFiltersChange).not.toHaveBeenCalled();
    expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
    expect(screen.getByRole('heading', { name: 'Build a precise search' })).toBeTruthy();
    expect(readSearchSessionProbe()).toMatchObject({
      appliedScope: { campuses: ['NB'], term: TERM },
      hasSubmittedQuery: false,
    });

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(1));
    expect(mainSearches(searchCourses)[0]?.filters.values).toMatchObject({
      campuses: ['NB'], keywords: ['data'], term: TERM,
    });
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await diagnosisSettled(view.container);
    expect(validateDynamicFilters).not.toHaveBeenCalled();
  });

  it('keeps the restored applied scope when the campus is un-ticked before Apply', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'] },
      runtime: productRuntime({ searchCourses }),
    }));
    expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy();

    fireEvent.click(screen.getByRole('checkbox', { name: /NB/u }));
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(false);
    expect(screen.queryByRole('button', { name: 'Applied' })).toBeNull();
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(false);
    expect(readSearchSessionProbe().appliedScope).toEqual({ campuses: ['NB'], term: TERM });

    // A later status poll must not re-apply over the user's un-tick either.
    view.rerender(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'] },
      runtime: productRuntime({ searchCourses }),
    }));
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(false);

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(1));
    expect(mainSearches(searchCourses)[0]?.filters.values.campuses).toEqual(['NB']);
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
  });

  it('keeps a stored scope pending until its targets become usable', async () => {
    const runtime = productRuntime({});
    const initialFilters = { ...createNeutralFilterState(TERM), campuses: ['NB'] };
    const view = render(storedWorkspace({ initialFilters, runtime, status: serviceStatus(false) }));
    const campus = screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement;
    expect(campus.checked).toBe(false);
    expect(campus.disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(true);
    expect(readSearchSessionProbe().appliedScope).toBeNull();

    view.rerender(storedWorkspace({ initialFilters, runtime, status: serviceStatus(true) }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy());
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(false);
    expect(readSearchSessionProbe().appliedScope).toEqual({ campuses: ['NB'], term: TERM });

    // A later poll with the same stored filters is a no-op.
    view.rerender(storedWorkspace({ initialFilters, runtime, status: serviceStatus(true) }));
    expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy();
  });

  it('does not restore a stored scope once the user touched the candidate before readiness', async () => {
    const runtime = productRuntime({});
    const initialFilters = { ...createNeutralFilterState(TERM), campuses: ['NB'] };
    const view = render(storedWorkspace({ initialFilters, runtime, status: serviceStatus(false) }));
    fireEvent.click(screen.getByRole('radio', { name: /Fall 2026/u }));
    expect((screen.getByRole('radio', { name: /Fall 2026/u }) as HTMLInputElement).checked).toBe(true);

    view.rerender(storedWorkspace({ initialFilters, runtime, status: serviceStatus(true) }));
    await waitFor(() => expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).disabled).toBe(true));
    expect(screen.queryByRole('button', { name: 'Applied' })).toBeNull();
    expect((screen.getByRole('radio', { name: /Fall 2026/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(true);
    expect(readSearchSessionProbe().appliedScope).toBeNull();
  });

  it('selects the stored term when the stored scope is not ready', () => {
    const first = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(NEXT_TERM), campuses: ['NB'] },
      runtime: productRuntime({}),
    }));
    expect((screen.getByRole('radio', { name: /Fall 2026/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(false);
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
    expect(readSearchSessionProbe().appliedScope).toBeNull();
    first.unmount();

    render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState('12025'), campuses: ['NB'] },
      runtime: productRuntime({}),
    }));
    expect((screen.getByRole('radio', { name: /Summer 2026/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(false);
    expect(readSearchSessionProbe().appliedScope).toBeNull();
  });

  it('selects the current term once status arrives when the stored term is not visible', async () => {
    const runtime = productRuntime({});
    const initialFilters = { ...createNeutralFilterState('12025'), campuses: ['NB'] };
    const view = render(storedWorkspace({ initialFilters, runtime, status: null }));
    expect(screen.queryByRole('radio', { name: /Summer 2026/u })).toBeNull();
    expect(screen.getByText('Waiting for the authoritative Rutgers term window.')).toBeTruthy();
    expect(readSearchSessionProbe().appliedScope).toBeNull();

    view.rerender(storedWorkspace({ initialFilters, runtime, status: serviceStatus(true) }));
    await waitFor(() => expect((screen.getByRole('radio', { name: /Summer 2026/u }) as HTMLInputElement).checked).toBe(true));
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(false);
    expect(screen.queryByRole('button', { name: 'Applied' })).toBeNull();
    expect(readSearchSessionProbe().appliedScope).toBeNull();
  });

  it('restores a stored ready scope once status arrives after mount', async () => {
    const runtime = productRuntime({});
    const initialFilters = { ...createNeutralFilterState(TERM), campuses: ['NB'] };
    const view = render(storedWorkspace({ initialFilters, runtime, status: null }));
    expect(screen.queryByRole('radio', { name: /Summer 2026/u })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Applied' })).toBeNull();

    view.rerender(storedWorkspace({ initialFilters, runtime, status: serviceStatus(true) }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy());
    expect((screen.getByRole('radio', { name: /Summer 2026/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(true);
    expect(readSearchSessionProbe().appliedScope).toEqual({ campuses: ['NB'], term: TERM });
  });

  it('keeps today\'s neutral start when no stored filters are supplied', () => {
    render(storedWorkspace({ runtime: productRuntime({}) }));
    expect((screen.getByRole('radio', { name: /Summer 2026/u }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole('checkbox', { name: /NB/u }) as HTMLInputElement).checked).toBe(false);
    expect(screen.queryByRole('button', { name: 'Applied' })).toBeNull();
    expect(readSearchSessionProbe().appliedScope).toBeNull();
  });

  it('still lets an external saved definition apply after a restore', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const runtime = productRuntime({ searchCourses });
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'] },
      runtime,
    }));
    expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();

    view.rerender(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], courseNumberBands: [900] },
      runtime,
    }));
    await waitFor(() => expect(screen.queryByRole('heading', { name: 'No matching records' })).toBeNull());
    expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
    expect(view.container.querySelector('[data-filter-chip="FLT-C05"]')?.textContent).toContain('900');
    expect(readSearchSessionProbe()).toMatchObject({
      appliedScope: { campuses: ['NB'], term: TERM },
      hasSuccessfulQuery: false,
    });
  });

  it('keeps results when a persisted canonical round-trip returns the same definition', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const runtime = productRuntime({ searchCourses });
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], levels: ['U', 'G'] },
      runtime,
    }));
    expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await diagnosisSettled(view.container);
    expect(mainSearches(searchCourses)).toHaveLength(1);

    view.rerender(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], levels: ['G', 'U'] },
      runtime,
    }));
    await act(async () => { await Promise.resolve(); });
    expect(screen.getByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(screen.queryByText(/outside the current ready query range/u)).toBeNull();
    expect(mainSearches(searchCourses)).toHaveLength(1);
    expect(readSearchSessionProbe()).toMatchObject({
      appliedScope: { campuses: ['NB'], term: TERM },
      hasSuccessfulQuery: true,
    });
  });

  it('surfaces the rejected saved value when the first search returns INVALID_FILTER_OPTION', async () => {
    const invalidOption = (details: { kind: 'INVALID_FIELD'; field: string }[]) => new ProductClientError(400, {
      protocolVersion: 1,
      error: {
        code: 'INVALID_FILTER_OPTION',
        messageKey: 'error.invalid_filter_option',
        traceId: 'trace-invalid-option',
        details,
      },
    });
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>()
      .mockRejectedValueOnce(invalidOption([]))
      .mockRejectedValueOnce(invalidOption([{ kind: 'INVALID_FIELD', field: 'FLT-C04' }]));
    const view = render(storedWorkspace({
      initialFilters: {
        ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['retired-token'], subjects: ['198'],
      },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Keyword match: retired-token');
    expect(alert.textContent).toContain('Subject: 198');
    expect(alert.textContent).not.toContain('FLT-');
    expect(view.container.querySelector('[data-query-state="invalid_option"]')).not.toBeNull();
    expect(screen.queryByRole('heading', { name: 'Search could not be completed' })).toBeNull();
    expect(readSearchSessionProbe().hasSuccessfulQuery).toBe(false);

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(2));
    const named = await screen.findByRole('alert');
    expect(named.textContent).toContain('Keyword match: retired-token');
    expect(named.textContent).not.toContain('Subject');
  });
});

describe('empty-result diagnosis', () => {
  function diagnosingMock(): SearchCoursesMock {
    return vi.fn<ProductApiPort['searchCourses']>(async (request) => {
      if (request.page.pageSize === 25) return emptyCourses;
      const { values } = request.filters;
      return probeResponse(
        values.synchronicities.length === 0 || values.includeIncomplete.synchronicity ? 4 : 0,
      );
    });
  }

  it('diagnoses an empty result with pageSize-1 probes and applies a one-click relaxation', async () => {
    const searchCourses = diagnosingMock();
    const onFiltersChange = vi.fn();
    const initialFilters = { ...createNeutralFilterState(TERM), campuses: ['NB'] };
    const runtime = productRuntime({ searchCourses });
    const view = render(storedWorkspace({ initialFilters, onFiltersChange, runtime }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Online' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Asynchronous' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    const diagnosis = await screen.findByRole('status', { name: 'Why is this empty?' });
    await diagnosisSettled(view.container);
    const buttons = within(diagnosis).getAllByRole('button');
    expect(buttons.map((button) => button.textContent)).toEqual([
      'Include records with incomplete Meeting timing data 4 courses',
      'Remove Meeting timing 4 courses',
    ]);
    expect(within(diagnosis).getByRole('button', { name: 'Include records with incomplete Meeting timing data 4 courses' })).toBeTruthy();
    expect(within(diagnosis).queryByRole('button', { name: /Remove Class format/u })).toBeNull();
    expect(within(diagnosis).getByText('Relaxing one condition would return results:')).toBeTruthy();
    const probes = probeSearches(searchCourses);
    expect(probes).toHaveLength(4);
    expect(probes.every((request) => request.page.page === 1 && request.page.pageSize === 1)).toBe(true);
    expect(probes.map((request) => JSON.stringify([
      request.filters.values.includeIncomplete, request.filters.values.synchronicities, request.filters.values.modalities,
    ]))).toEqual([
      JSON.stringify([{ prerequisite: false, modality: false, synchronicity: true }, ['ASYNC'], ['ONLINE']]),
      JSON.stringify([{ prerequisite: false, modality: true, synchronicity: false }, ['ASYNC'], ['ONLINE']]),
      JSON.stringify([{ prerequisite: false, modality: false, synchronicity: false }, [], ['ONLINE']]),
      JSON.stringify([{ prerequisite: false, modality: false, synchronicity: false }, ['ASYNC'], []]),
    ]);
    expect(mainSearches(searchCourses)).toHaveLength(1);
    const changesBeforeRelaxation = onFiltersChange.mock.calls.length;

    fireEvent.click(buttons[0] as HTMLElement);
    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(2));
    expect(mainSearches(searchCourses)[1]?.filters.values).toMatchObject({
      includeIncomplete: { prerequisite: false, modality: false, synchronicity: true },
      modalities: ['ONLINE'],
      synchronicities: ['ASYNC'],
    });
    expect(onFiltersChange).toHaveBeenCalledTimes(changesBeforeRelaxation + 1);
    expect(onFiltersChange.mock.calls.at(-1)?.[0]).toMatchObject({
      campuses: ['NB'],
      includeIncomplete: { prerequisite: false, modality: false, synchronicity: true },
    });
    const timingRow = view.container.querySelector<HTMLElement>('[data-filter-row="FLT-S04b"]');
    if (timingRow === null) throw new Error('Expected the Meeting timing row.');
    expect((within(timingRow).getByRole('checkbox', { name: /Complete data display/u }) as HTMLInputElement).checked).toBe(true);
    await waitFor(() => expect(readSearchSessionProbe()).toMatchObject({
      hasSuccessfulQuery: true,
      successfulIncludeIncomplete: { prerequisite: false, modality: false, synchronicity: true },
    }));
    await diagnosisSettled(view.container);
  });

  it('keeps the relaxed result after the persisted round-trip returns the canonical definition', async () => {
    const searchCourses = diagnosingMock();
    const runtime = productRuntime({ searchCourses });
    const initialFilters = {
      ...createNeutralFilterState(TERM), campuses: ['NB'], modalities: ['ONLINE' as const], synchronicities: ['ASYNC' as const],
    };
    const view = render(storedWorkspace({ initialFilters, runtime }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await diagnosisSettled(view.container);
    fireEvent.click(screen.getByRole('button', { name: 'Include records with incomplete Meeting timing data 4 courses' }));
    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(2));
    await waitFor(() => expect(readSearchSessionProbe().successfulIncludeIncomplete?.synchronicity).toBe(true));
    await diagnosisSettled(view.container);

    view.rerender(storedWorkspace({
      initialFilters: {
        ...initialFilters,
        includeIncomplete: { prerequisite: false, modality: false, synchronicity: true },
      },
      runtime,
    }));
    await act(async () => { await Promise.resolve(); });
    expect(screen.getByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(mainSearches(searchCourses)).toHaveLength(2);
    expect(readSearchSessionProbe()).toMatchObject({
      appliedScope: { campuses: ['NB'], term: TERM },
      hasSuccessfulQuery: true,
      successfulIncludeIncomplete: { prerequisite: false, modality: false, synchronicity: true },
    });
  });

  it('aborts in-flight probes when a new search starts', async () => {
    const pending: { signal: AbortSignal | undefined; resolve: (response: CourseQueryResponseV1) => void }[] = [];
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>((request, signal) => {
      if (request.page.pageSize === 25) return Promise.resolve(emptyCourses);
      return new Promise<CourseQueryResponseV1>((resolve) => {
        pending.push({ resolve, signal });
      });
    });
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['data'], modalities: ['ONLINE'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await waitFor(() => expect(pending).toHaveLength(3));
    expect(view.container.querySelector('[data-diagnosis-state="PROBING"]')).not.toBeNull();
    const firstWave = [...pending];
    expect(firstWave.every(({ signal }) => signal?.aborted === false)).toBe(true);

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await waitFor(() => expect(firstWave.every(({ signal }) => signal?.aborted === true)).toBe(true));
    await waitFor(() => expect(mainSearches(searchCourses)).toHaveLength(2));
    await waitFor(() => expect(pending).toHaveLength(6));
    await act(async () => {
      for (const probe of firstWave) probe.resolve(probeResponse(9));
    });
    expect(screen.queryByRole('button', { name: /9 courses/u })).toBeNull();
    expect(view.container.querySelector('[data-diagnosis-state="PROBING"]')).not.toBeNull();
  });

  it('aborts probes and unmounts the diagnosis when a new scope is applied', async () => {
    const baseStatus = serviceStatus(true);
    const nb = baseStatus.targets[0];
    if (nb === undefined) throw new Error('Expected NB fixture target.');
    const multiCampusStatus: ServiceStatusV2 = {
      ...baseStatus,
      targets: [nb, { ...nb, primary: false, target: { term: TERM, campus: 'NK' } }],
    };
    const signals: (AbortSignal | undefined)[] = [];
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>((request, signal) => {
      if (request.page.pageSize === 25) return Promise.resolve(emptyCourses);
      signals.push(signal);
      return new Promise<CourseQueryResponseV1>(() => undefined);
    });
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], openStatuses: ['OPEN'] },
      runtime: productRuntime({ searchCourses }),
      status: multiCampusStatus,
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await waitFor(() => expect(signals).toHaveLength(1));
    expect(screen.getByRole('status', { name: 'Why is this empty?' })).toBeTruthy();

    fireEvent.click(screen.getByRole('checkbox', { name: /^NK/u }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Applied' })).toBeTruthy());
    await waitFor(() => expect(signals[0]?.aborted).toBe(true));
    expect(screen.queryByRole('status', { name: 'Why is this empty?' })).toBeNull();
    expect(screen.queryByRole('heading', { name: 'No matching records' })).toBeNull();
    expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
    expect(signals).toHaveLength(1);
  });

  it('retries a not-ready probe once and shows the row when the retry succeeds', async () => {
    const attempts = new Map<string, number>();
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>(async (request) => {
      if (request.page.pageSize === 25) return emptyCourses;
      const key = JSON.stringify(request.filters);
      const attempt = (attempts.get(key) ?? 0) + 1;
      attempts.set(key, attempt);
      if (attempt === 1) throw notReadyError();
      return probeResponse(3);
    });
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['data'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(await screen.findByRole('button', { name: 'Remove Keyword match 3 courses' }, { timeout: 5000 })).toBeTruthy();
    expect(probeSearches(searchCourses)).toHaveLength(2);
    expect(mainSearches(searchCourses)).toHaveLength(1);
    expect(view.container.querySelector('[data-diagnosis-state="READY"]')).not.toBeNull();
  }, 10_000);

  it('shows the unavailable line when every probe keeps failing and never disturbs the retained result', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>(async (request) => {
      if (request.page.pageSize === 25) return emptyCourses;
      throw notReadyError();
    });
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['data'], levels: ['U'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(await screen.findByText('The cause could not be checked right now.', undefined, { timeout: 5000 })).toBeTruthy();
    expect(probeSearches(searchCourses)).toHaveLength(4);
    expect(screen.getByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(view.container.querySelector('[data-query-state="courses"]')).not.toBeNull();
    expect(screen.queryByRole('heading', { name: 'Search could not be completed' })).toBeNull();
    expect(readSearchSessionProbe().hasSuccessfulQuery).toBe(true);
  }, 10_000);

  it('stops as unavailable when the retry is rate limited', async () => {
    let attempt = 0;
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>(async (request) => {
      if (request.page.pageSize === 25) return emptyCourses;
      attempt += 1;
      if (attempt === 1) throw notReadyError();
      throw new ProductClientError(429, {
        protocolVersion: 1,
        error: { code: 'RATE_LIMITED', messageKey: 'error.rate_limited', traceId: 'trace-probe-429', details: [] },
      });
    });
    render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['data'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByText('The cause could not be checked right now.', undefined, { timeout: 5000 })).toBeTruthy();
    expect(probeSearches(searchCourses)).toHaveLength(2);
  }, 10_000);

  it('does not retry a non-transient probe failure', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>(async (request) => {
      if (request.page.pageSize === 25) return emptyCourses;
      throw new ProductClientError(400, {
        protocolVersion: 1,
        error: { code: 'INVALID_FILTER', messageKey: 'error.invalid_filter', traceId: 'trace-probe-400', details: [] },
      });
    });
    render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['data'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByText('The cause could not be checked right now.')).toBeTruthy();
    expect(probeSearches(searchCourses)).toHaveLength(1);
  });

  it('explains that no single condition is responsible when every probe stays empty', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const view = render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], keywords: ['data'], levels: ['U'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByText(/No single condition explains it/u)).toBeTruthy();
    expect(within(screen.getByRole('status', { name: 'Why is this empty?' })).queryAllByRole('button')).toHaveLength(0);
    expect(view.container.querySelector('[data-diagnosis-state="READY"]')).not.toBeNull();
  });

  it('renders no diagnosis when only the scope was active', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await act(async () => { await Promise.resolve(); });
    expect(screen.queryByRole('status', { name: 'Why is this empty?' })).toBeNull();
    expect(probeSearches(searchCourses)).toHaveLength(0);
  });

  it('caps diagnosis at eight probes and omits the lowest-priority relaxations', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const view = render(storedWorkspace({
      initialFilters: {
        ...createNeutralFilterState(TERM),
        campuses: ['NB'],
        subjects: ['198'],
        keywords: ['data'],
        courseNumberBands: [100],
        levels: ['U'],
        core: { codes: ['QR'], mode: 'ANY' },
        prerequisite: 'NONE_REPORTED',
        openStatuses: ['CLOSED'],
        modalities: ['ONLINE'],
        synchronicities: ['ASYNC'],
      },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    await diagnosisSettled(view.container);
    const probes = probeSearches(searchCourses);
    expect(probes).toHaveLength(8);
    expect(searchCourses).toHaveBeenCalledTimes(9);
    for (const request of probes) {
      expect(request.filters.values.keywords).toEqual(['data']);
      expect(request.filters.values.subjects).toEqual(['198']);
      expect(request.filters.values.courseNumberBands).toEqual([100]);
      expect(request.filters.values.levels).toEqual(['U']);
    }
    expect(probes.filter((request) => request.filters.values.core.codes.length === 0)).toHaveLength(1);
    expect(probes.filter((request) => request.filters.values.includeIncomplete.synchronicity)).toHaveLength(1);
  });

  it('announces the diagnosis politely', async () => {
    const pending: ((response: CourseQueryResponseV1) => void)[] = [];
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>((request) => {
      if (request.page.pageSize === 25) return Promise.resolve(emptyCourses);
      return new Promise<CourseQueryResponseV1>((resolve) => {
        pending.push(resolve);
      });
    });
    render(storedWorkspace({
      initialFilters: { ...createNeutralFilterState(TERM), campuses: ['NB'], instructors: ['Ada Lovelace'] },
      runtime: productRuntime({ searchCourses }),
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    const diagnosis = await screen.findByRole('status', { name: 'Why is this empty?' });
    expect(diagnosis.getAttribute('aria-live')).toBe('polite');
    expect(diagnosis.getAttribute('aria-busy')).toBe('true');
    expect(within(diagnosis).getByText('Checking which condition removes every result…')).toBeTruthy();
    await waitFor(() => expect(pending).toHaveLength(1));

    await act(async () => {
      pending[0]?.(probeResponse(1234));
    });
    expect(diagnosis.getAttribute('aria-busy')).toBeNull();
    const row = within(diagnosis).getByRole('button', { name: 'Remove Instructor 1,234 courses' });
    expect(row.tagName).toBe('BUTTON');
    expect(row.getAttribute('data-relaxation')).toBe('CLEAR_FIELD:FLT-S05');
  });
});
