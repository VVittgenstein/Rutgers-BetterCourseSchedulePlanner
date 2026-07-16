// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  CourseQueryResponseV1,
  FilterSchemaV1,
  ProductApiPort,
  ProductRuntimePort,
  SectionQueryResponseV1,
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
  filterCount: 22,
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
  contractVersion: 1,
  items: [],
  page: { page: 1, pageSize: 25, total: 0, totalPages: 0 },
};

const emptySections: SectionQueryResponseV1 = {
  contractVersion: 1,
  items: [],
  page: { page: 1, pageSize: 25, total: 0, totalPages: 0 },
};

function serviceStatus(searchAvailable: boolean): ServiceStatusV1 {
  return {
    contractVersion: 1,
    observedAt: '2026-07-15T00:00:01Z',
    runtime: 'LOCAL',
    level: searchAvailable ? 'PARTIALLY_READY' : 'INITIALIZING',
    operation: {
      phase: searchAvailable ? 'OPEN_FETCH' : 'CATALOG_FETCH',
      target: { campus: 'NB', term: '2026-9' },
      startedAt: '2026-07-15T00:00:00Z',
      nextRetryAt: null,
    },
    discovery: shellState.discovery.status,
    catalog: { availableTargetCount: searchAvailable ? 1 : 0, totalTargetCount: 1 },
    open: { availableTargetCount: 0, totalTargetCount: 1 },
    targets: [{
      target: { campus: 'NB', term: '2026-9' },
      primary: true,
      catalogAvailability: searchAvailable ? 'CURRENT' : 'UNAVAILABLE',
      catalogContentVersion: searchAvailable ? 1 : null,
      openAvailability: 'UNAVAILABLE',
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
  status?: ServiceStatusV1 | null,
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

describe('Course and Section workspace controller', () => {
  it('disables search while the selected Catalog target is not published', () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue(emptyCourses);
    renderWorkspace(productRuntime({ searchCourses }), '/', shellState, serviceStatus(false));

    const submit = screen.getByRole('button', { name: 'Search' });
    expect(submit.hasAttribute('disabled')).toBe(true);
    expect(screen.getByText(/Search unlocks after every selected Catalog target/u)).toBeTruthy();
    fireEvent.click(submit);
    expect(searchCourses).not.toHaveBeenCalled();
  });

  it('returns to the initialization prompt when readiness races with CATALOG_NOT_READY', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockRejectedValue(
      new ProductClientError(409, {
        protocolVersion: 1,
        error: {
          code: 'CATALOG_NOT_READY',
          messageKey: 'error.catalog_not_ready',
          traceId: 'trace-catalog-race',
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
    expect(screen.getByText(/every selected Catalog target/u)).toBeTruthy();
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
          <SearchWorkspace runtime={runtime} shellState={hydrated} />
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
    renderWorkspace(productRuntime({ searchCourses }), '/');

    fireEvent.change(screen.getByLabelText('Search text'), { target: { value: 'data structures' } });
    fireEvent.click(screen.getByText('Same-Section constraints'));
    fireEvent.click(within(screen.getByRole('group', { name: 'Open status' }))
      .getByRole('checkbox', { name: 'Open' }));
    fireEvent.change(screen.getByLabelText('Instructor names'), { target: { value: 'Ada Lovelace' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Instructor names' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => expect(searchCourses).toHaveBeenCalledTimes(1));
    const request = searchCourses.mock.calls[0]?.[0];
    expect(request?.filters.values).toMatchObject({
      campuses: ['NB'],
      instructors: ['Ada Lovelace'],
      openStatuses: ['OPEN'],
      term: '2026-9',
      text: 'data structures',
    });
    expect(request?.page).toEqual({ page: 1, pageSize: 25 });
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
  });

  it('uses the independent Section query endpoint without an automatic request', async () => {
    const searchSections = vi.fn<ProductApiPort['searchSections']>().mockResolvedValue(emptySections);
    renderWorkspace(productRuntime({ searchSections }), '/sections');

    expect(searchSections).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Section indexes'), { target: { value: '12345' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Section indexes' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));

    await waitFor(() => expect(searchSections).toHaveBeenCalledTimes(1));
    expect(searchSections.mock.calls[0]?.[0].filters.values.sectionIndexes).toEqual(['12345']);
  });

  it('keeps an invalid Section index local, identifies its row, and focuses its control', async () => {
    const searchSections = vi.fn<ProductApiPort['searchSections']>().mockResolvedValue(emptySections);
    const view = renderWorkspace(productRuntime({ searchSections }), '/sections');

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
      expect(searchSections).not.toHaveBeenCalled();
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
      contractVersion: 1,
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
