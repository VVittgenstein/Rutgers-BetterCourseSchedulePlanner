// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  CourseQueryResponseV1,
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

function applyNewBrunswick(): void {
  fireEvent.click(screen.getByRole('checkbox', { name: /NB/u }));
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
}

function SearchSessionProbe() {
  const session = useSearchSession();
  return (
    <output data-testid="search-session-probe">
      {JSON.stringify({
        appliedScope: session.state.appliedScope,
        hasSubmittedQuery: session.state.lastSubmittedRequest !== null,
        hasSuccessfulQuery: session.state.lastSuccessfulResponse !== null,
      })}
    </output>
  );
}

function readSearchSessionProbe() {
  return JSON.parse(screen.getByTestId('search-session-probe').textContent ?? '{}') as {
    readonly appliedScope: { readonly campuses: readonly string[]; readonly term: string } | null;
    readonly hasSubmittedQuery: boolean;
    readonly hasSuccessfulQuery: boolean;
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
    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(2));
    expect(searchCourses.mock.calls[1]?.[0].filters.values.campuses).toEqual(['NB']);
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
    expect(view.container.querySelector('.bcsp-search-workspace')?.getAttribute('data-results-visible')).toBe('false');
    expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
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

    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(1));
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

    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(1));
    expect(searchCourses.mock.calls[0]?.[0].filters.values).toMatchObject({
      campuses: ['NB'], instructors: ['Ada Lovelace'], keywords: ['data'],
      openStatuses: ['OPEN'], term: TERM,
    });
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
  });

  it('keeps Section constraints inside Course search and never calls the legacy Section endpoint', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const searchSections = vi.fn<ProductApiPort['searchSections']>();
    renderWorkspace(productRuntime({ searchCourses, searchSections }));
    applyNewBrunswick();
    fireEvent.change(screen.getByLabelText('Section indexes'), { target: { value: '12345' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Section indexes' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(1));
    expect(searchCourses.mock.calls[0]?.[0].filters.values.sectionIndexes).toEqual(['12345']);
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
