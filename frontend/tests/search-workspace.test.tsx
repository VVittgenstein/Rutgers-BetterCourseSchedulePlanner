// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  CourseQueryResponseV1,
  FilterOptionsFieldV2,
  FilterOptionsResponseV2,
  FilterSchemaV1,
  ProductApiPort,
  ProductRuntimePort,
  ServiceStatusV1,
} from '../src/ui/shared/product';
import { ProductClientError } from '../src/ui/shared/product';
import { SearchWorkspace } from '../src/ui/shared/search';
import { AppRouterProvider } from '../src/ui/shared/routing';
import type { ShellDataState } from '../src/ui/shared/shell';

const SCHEMA = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;

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
      availability: 'CURRENT',
      error: null,
      isStale: false,
      lastSuccess: point,
      latestAttempt: point,
    },
    coreCodeDictionaries: [],
    subjects: [{
      code: '198',
      label: known('Computer Science'),
      provenance: { kind: 'DISCOVERY', discovery: provenance },
      target: { campus: 'NB', term: '2026-9' },
    }],
    targets: [{
      campusLabel: known('New Brunswick'),
      key: { campus: 'NB', term: '2026-9' },
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

function target(
  term: string,
  campus: string,
  termLabel: string,
  campusLabel: string,
): CatalogDiscoveryResponseV1['targets'][number] {
  const provenance = discovery().targets[0]?.provenance;
  if (provenance === undefined) throw new Error('synthetic discovery target is required');
  return {
    campusLabel: known(campusLabel),
    key: { campus, term },
    provenance,
    termLabel: known(termLabel),
  };
}

function shellStateWithTargets(
  targets: CatalogDiscoveryResponseV1['targets'],
): Extract<ShellDataState, { status: 'READY' }> {
  return {
    ...shellState,
    discovery: { ...shellState.discovery, subjects: [], targets },
  };
}

const emptyCourses: CourseQueryResponseV1 = {
  contractVersion: 2,
  items: [],
  page: { page: 1, pageSize: 25, total: 0, totalPages: 0 },
};

function serviceStatus(searchAvailable: boolean): ServiceStatusV1 {
  const totalTargetCount = 135;
  return {
    contractVersion: 1,
    observedAt: '2026-07-15T00:00:01Z',
    runtime: 'LOCAL',
    level: searchAvailable ? 'READY' : 'INITIALIZING',
    operation: {
      phase: searchAvailable ? 'OPEN_FETCH' : 'CATALOG_FETCH',
      target: { campus: 'NB', term: '2026-9' },
      startedAt: '2026-07-15T00:00:00Z',
      nextRetryAt: null,
    },
    discovery: shellState.discovery.status,
    catalog: {
      availableTargetCount: totalTargetCount,
      currentTargetCount: totalTargetCount,
      staleTargetCount: 0,
      totalTargetCount,
      unavailableTargetCount: 0,
    },
    open: {
      availableTargetCount: searchAvailable ? totalTargetCount : totalTargetCount - 1,
      currentTargetCount: searchAvailable ? totalTargetCount : totalTargetCount - 1,
      staleTargetCount: 0,
      totalTargetCount,
      unavailableTargetCount: searchAvailable ? 0 : 1,
    },
    targets: [{
      target: { campus: 'NB', term: '2026-9' },
      primary: true,
      catalogAvailability: searchAvailable ? 'CURRENT' : 'UNAVAILABLE',
      catalogContentVersion: searchAvailable ? 1 : null,
      openAvailability: searchAvailable ? 'CURRENT' : 'UNAVAILABLE',
      searchAvailable,
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
  path: string,
  state: Extract<ShellDataState, { status: 'READY' }> = shellState,
  status: ServiceStatusV1 | null = serviceStatus(true),
) {
  return render(
    <BcspI18nProvider initialLocale="en-US">
      <AppRouterProvider initialPath={path}>
        <SearchWorkspace runtime={runtime} serviceStatus={status} shellState={state} />
      </AppRouterProvider>
    </BcspI18nProvider>,
  );
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

afterEach(cleanup);

describe('unified Course workspace controller', () => {
  it('disables the full search surface until Catalog and Open are both 135/135', () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    renderWorkspace(productRuntime({ searchCourses }), '/', shellState, serviceStatus(false));

    const submit = screen.getByRole('button', { name: 'Search' });
    expect(submit.hasAttribute('disabled')).toBe(true);
    expect((screen.getByLabelText('Term') as HTMLSelectElement).disabled).toBe(true);
    expect((screen.getByRole('checkbox', { name: /New Brunswick/u }) as HTMLInputElement).disabled)
      .toBe(true);
    expect(screen.getByText(/complete course catalog and live availability data/u)).toBeTruthy();
    fireEvent.click(submit);
    expect(searchCourses).not.toHaveBeenCalled();
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
    const view = renderWorkspace(
      productRuntime({ searchCourses }),
      '/',
      shellState,
      serviceStatus(true),
    );

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(1));
    expect(await screen.findByRole('heading', { name: 'Opening the catalog console' })).toBeTruthy();
    expect(view.container.querySelector('[data-query-state="not_ready"]')).not.toBeNull();
    expect(screen.getByText(/complete course catalog and live availability data/u)).toBeTruthy();
    expect(screen.queryByRole('heading', { name: 'Search failed' })).toBeNull();
  });

  it('defaults to the same latest Rutgers term selected by the refresh coordinator', () => {
    const state = shellStateWithTargets([
      target('92025', 'OLD', 'Fall 2025', 'Historical campus'),
      target('72026', 'SUMMER', 'Summer 2026', 'Summer campus'),
      target('92026', 'CURRENT', 'Fall 2026', 'Current campus'),
      target('12026', 'SPRING', 'Spring 2026', 'Spring campus'),
    ]);
    renderWorkspace(productRuntime({}), '/', state);

    expect((screen.getByLabelText('Term') as HTMLSelectElement).value).toBe('92026');
    expect((screen.getByRole('checkbox', { name: /Current campus/ }) as HTMLInputElement).checked)
      .toBe(true);
  });

  it('keeps the first discovered target as the stable fallback for non-Rutgers term identities', () => {
    const state = shellStateWithTargets([
      target('SYNTHETIC_B', 'FIRST', 'Synthetic B', 'First synthetic campus'),
      target('SYNTHETIC_A', 'SECOND', 'Synthetic A', 'Second synthetic campus'),
    ]);
    renderWorkspace(productRuntime({}), '/', state);

    expect((screen.getByLabelText('Term') as HTMLSelectElement).value).toBe('SYNTHETIC_B');
    expect((screen.getByRole('checkbox', { name: /First synthetic campus/ }) as HTMLInputElement).checked)
      .toBe(true);
  });

  it('moves an untouched default to a hydrated target without accepting the old search', async () => {
    const targets = [
      target('92026', 'EMPTY', 'Fall 2026', 'Empty campus'),
      target('92026', 'READY', 'Fall 2026', 'Ready campus'),
    ];
    const initial = shellStateWithTargets(targets);
    const pending = deferred<CourseQueryResponseV1>();
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockReturnValue(pending.promise);
    const runtime = productRuntime({ searchCourses });
    const view = renderWorkspace(runtime, '/', initial);
    expect((screen.getByRole('checkbox', { name: /Empty campus/ }) as HTMLInputElement).checked)
      .toBe(true);

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(1));
    const oldSignal = searchCourses.mock.calls[0]?.[1];
    expect(oldSignal?.aborted).toBe(false);

    const subject = discovery().subjects[0];
    if (subject === undefined || targets[1] === undefined) {
      throw new Error('synthetic subject and target are required');
    }
    const hydrated: Extract<ShellDataState, { status: 'READY' }> = {
      ...initial,
      discovery: {
        ...initial.discovery,
        subjects: [{ ...subject, target: targets[1].key }],
      },
    };
    view.rerender(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchWorkspace runtime={runtime} serviceStatus={serviceStatus(true)} shellState={hydrated} />
        </AppRouterProvider>
      </BcspI18nProvider>,
    );

    await waitFor(() => {
      expect((screen.getByRole('checkbox', { name: /Ready campus/ }) as HTMLInputElement).checked)
        .toBe(true);
      expect(oldSignal?.aborted).toBe(true);
      expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
    });

    pending.resolve(emptyCourses);
    await waitFor(() => {
      expect(view.container.querySelector('[data-query-state="idle"]')).not.toBeNull();
      expect(screen.queryByRole('heading', { name: 'No matching records' })).toBeNull();
    });
  });

  it('submits one typed Course query with combined Course and same-Section filters', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const filterOptions = vi.fn(async (
      request: Parameters<ProductApiPort['filterOptions']>[0],
    ): Promise<FilterOptionsResponseV2> => {
      const options = request.field === 'KEYWORD' && request.query?.toLocaleLowerCase() === 'data'
        ? [{ value: 'data', label: 'data' }]
        : request.field === 'INSTRUCTOR' && request.query?.toLocaleLowerCase() === 'ada'
          ? [{ value: 'Ada Lovelace', label: 'Ada Lovelace' }]
          : [];
      return {
        contractVersion: 2,
        field: request.field,
        options,
        targetVersions: [],
        truncated: false,
      };
    });
    renderWorkspace(productRuntime({ filterOptions, searchCourses }), '/');

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
    const request = searchCourses.mock.calls[0]?.[0];
    expect(request?.filters.values).toMatchObject({
      campuses: ['NB'],
      instructors: ['Ada Lovelace'],
      keywords: ['data'],
      openStatuses: ['OPEN'],
      term: '2026-9',
    });
    expect(request?.page).toEqual({ page: 1, pageSize: 25 });
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
  });

  it('keeps Section constraints inside Course search and never calls the legacy Section endpoint', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const searchSections = vi.fn<ProductApiPort['searchSections']>();
    const view = renderWorkspace(productRuntime({ searchCourses, searchSections }), '/');

    expect(view.container.querySelector('[data-search-mode="courses"]')).not.toBeNull();
    expect(view.container.querySelector('[data-search-mode="sections"]')).toBeNull();
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
    const view = renderWorkspace(productRuntime({ searchCourses }), '/');

    fireEvent.click(screen.getByText('Same-Section constraints'));
    const sectionIndexInput = screen.getByLabelText('Section indexes');
    fireEvent.change(sectionIndexInput, { target: { value: '1234' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Section indexes' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    const row = view.container.querySelector<HTMLElement>('[data-filter-row="FLT-S01"]');
    expect(row).not.toBeNull();
    const scrollIntoView = vi.fn();
    Object.defineProperty(row as HTMLElement, 'scrollIntoView', {
      configurable: true,
      value: scrollIntoView,
    });
    await waitFor(() => {
      expect(searchCourses).not.toHaveBeenCalled();
      expect(row?.dataset.filterError).toBe('true');
      expect(row?.getAttribute('aria-invalid')).toBe('true');
      expect(within(row as HTMLElement).getByText('A Section index must contain five digits.')).toBeTruthy();
      expect(document.activeElement).toBe(sectionIndexInput);
      expect(scrollIntoView).toHaveBeenCalledWith({ behavior: 'auto', block: 'nearest' });
    });
  });

  it('keeps a reversed credit range local and returns focus to the credit row', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    const view = renderWorkspace(productRuntime({ searchCourses }), '/');

    const minimumInput = screen.getByLabelText('Minimum credits');
    fireEvent.change(minimumInput, { target: { value: '4' } });
    fireEvent.change(screen.getByLabelText('Maximum credits'), { target: { value: '3' } });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    const row = view.container.querySelector<HTMLElement>('[data-filter-row="FLT-C07"]');
    expect(row).not.toBeNull();
    await waitFor(() => {
      expect(searchCourses).not.toHaveBeenCalled();
      expect(row?.dataset.filterError).toBe('true');
      expect(row?.getAttribute('aria-invalid')).toBe('true');
      expect(within(row as HTMLElement).getByText('Credits must form a nonempty ordered range.')).toBeTruthy();
      expect(document.activeElement).toBe(minimumInput);
    });
  });

  it('reloads a safe direct Section URL through the typed detail endpoint', async () => {
    const sectionDetail = vi.fn<ProductApiPort['sectionDetail']>()
      .mockImplementation(() => new Promise(() => undefined));
    renderWorkspace(productRuntime({ sectionDetail }), '/sections/2026-9/NB/12345');

    await waitFor(() => expect(sectionDetail).toHaveBeenCalledTimes(1));
    expect(sectionDetail.mock.calls[0]?.[0]).toEqual({
      contractVersion: 2,
      key: { campus: 'NB', index: '12345', term: '2026-9' },
    });
    expect(screen.getByRole('heading', { name: 'Section detail' })).toBeTruthy();
  });

  it('rejects an invalid direct Section URL before any API request', async () => {
    const sectionDetail = vi.fn<ProductApiPort['sectionDetail']>();
    renderWorkspace(productRuntime({ sectionDetail }), '/sections/2026-9/NB/not-an-index');

    expect(await screen.findByRole('alert')).toBeTruthy();
    expect(sectionDetail).not.toHaveBeenCalled();
  });
});
