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
import { SearchWorkspace } from '../src/ui/shared/search';
import { AppRouterProvider } from '../src/ui/shared/routing';
import type { ShellDataState } from '../src/ui/shared/shell';

const SCHEMA = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;
const TERM = '2026-9';
const NEXT_TERM = '2027-0';
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
      termLabel: known('Fall 2026'),
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
  contractVersion: 2,
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
        { term: TERM, relativeOffset: 0, discovered: true, autoManaged: true, manualPullAllowed: false, watchable: true },
        { term: NEXT_TERM, relativeOffset: 1, discovered: false, autoManaged: true, manualPullAllowed: false, watchable: true },
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
  fireEvent.click(screen.getByRole('checkbox', { name: /New Brunswick/u }));
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
}

afterEach(cleanup);

describe('RC3 unified Course workspace controller', () => {
  it('keeps scope controls available while disabling 03–18 until a selected target is READY', () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    renderWorkspace(productRuntime({ searchCourses }), '/', serviceStatus(false));

    expect((screen.getByRole('radio', { name: /Fall 2026/u }) as HTMLInputElement).disabled).toBe(false);
    expect((screen.getByRole('checkbox', { name: /New Brunswick/u }) as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Apply' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByLabelText('Term')).toBeNull();
    expect(screen.getByText(/Loading complete course and availability data/u)).toBeTruthy();
  });

  it('starts with current term and no auto-selected Campus, then enables one partial-ready target', () => {
    renderWorkspace(productRuntime({}));
    expect((screen.getByRole('radio', { name: /Fall 2026/u }) as HTMLInputElement).checked).toBe(true);
    const campus = screen.getByRole('checkbox', { name: /New Brunswick/u }) as HTMLInputElement;
    expect(campus.checked).toBe(false);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(true);

    applyNewBrunswick();
    expect(screen.getByText('Current range')).toBeTruthy();
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
      courseNumbers: ['999'],
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

  it('preserves valid target-bound values and removes only invalid options when applying a scope', async () => {
    const onFiltersChange = vi.fn();
    const filterOptions = vi.fn(async (
      request: Parameters<NonNullable<ProductApiPort['filterOptions']>>[0],
    ): Promise<FilterOptionsResponseV2> => ({
      contractVersion: 2,
      field: request.field,
      options: request.query === 'data' ? [{ value: 'data', label: 'data' }] : [],
      targetVersions: [{ target: { term: TERM, campus: 'NB' }, contentVersion: 1 }],
      truncated: false,
    }));
    const initialFilters = {
      ...createNeutralFilterState(TERM),
      keywords: ['data', 'retired-token'],
      subjects: ['198', '999'],
    };
    render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchWorkspace
            initialFilters={initialFilters}
            onFiltersChange={onFiltersChange}
            runtime={productRuntime({ filterOptions })}
            serviceStatus={serviceStatus(true)}
            shellState={shellState}
          />
        </AppRouterProvider>
      </BcspI18nProvider>,
    );

    fireEvent.click(screen.getByRole('checkbox', { name: /New Brunswick/u }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));

    await waitFor(() => expect(onFiltersChange).toHaveBeenCalledTimes(1));
    expect(onFiltersChange.mock.calls[0]?.[0]).toMatchObject({
      campuses: ['NB'],
      keywords: ['data'],
      subjects: ['198'],
      term: TERM,
    });
    expect(screen.getByText(/Unavailable dictionary options were removed/u)).toBeTruthy();
    expect(filterOptions.mock.calls.filter(([request]) => (
      request.field === 'KEYWORD' && request.query === 'data'
    ))).toHaveLength(1);
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
      contractVersion: 2,
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
    fireEvent.click(screen.getByText('Same-Section constraints'));
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
    fireEvent.click(screen.getByText('Same-Section constraints'));
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
    fireEvent.click(screen.getByText('Same-Section constraints'));
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
      contractVersion: 2,
      key: { campus: 'NB', index: '12345', term: TERM },
    });
    first.unmount();

    const invalidDetail = vi.fn<ProductApiPort['sectionDetail']>();
    renderWorkspace(productRuntime({ sectionDetail: invalidDetail }), `/sections/${TERM}/NB/not-an-index`);
    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(invalidDetail).not.toHaveBeenCalled();
  });
});
