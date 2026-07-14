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
} from '../src/ui/shared/product';
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

function productRuntime(overrides: Partial<ProductApiPort>): ProductRuntimePort {
  return {
    dispose() {},
    product: overrides as ProductApiPort,
    watch: {} as ProductRuntimePort['watch'],
  };
}

function renderWorkspace(runtime: ProductRuntimePort, path: string) {
  return render(
    <BcspI18nProvider initialLocale="en-US">
      <AppRouterProvider initialPath={path}>
        <SearchWorkspace runtime={runtime} shellState={shellState} />
      </AppRouterProvider>
    </BcspI18nProvider>,
  );
}

afterEach(cleanup);

describe('Course and Section workspace controller', () => {
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
