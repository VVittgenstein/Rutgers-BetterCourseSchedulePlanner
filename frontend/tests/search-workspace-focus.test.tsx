// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  CourseDetailResponseV1,
  CourseQueryResponseV1,
  FilterSchemaV1,
  ProductApiPort,
  ProductRuntimePort,
} from '../src/ui/shared/product';
import { AppRouterProvider } from '../src/ui/shared/routing';
import type {
  CourseDetailViewProps,
  CourseResultsViewProps,
  SectionDetailViewProps,
  SectionResultsViewProps,
} from '../src/ui/shared/search/results';
import { SearchWorkspace } from '../src/ui/shared/search';
import type { ShellDataState } from '../src/ui/shared/shell';

vi.mock('../src/ui/shared/search/results', () => ({
  CourseDetailView: (_props: CourseDetailViewProps) => <h2>Resolved course detail</h2>,
  CourseResultsView: ({ onCourseDetail, onPageChange, response }: CourseResultsViewProps) => (
    <div data-course-group="01:198:211">
      <p>Course results page {response.page.page}</p>
      <button
        className="search-results__button"
        onClick={() => onCourseDetail({ campus: 'NB', courseString: '01:198:211', term: '2026-9' })}
        type="button"
      >
        Open course detail
      </button>
      <button onClick={() => onPageChange(2)} type="button">Next result page</button>
    </div>
  ),
  SectionDetailView: (_props: SectionDetailViewProps) => <h2>Resolved section detail</h2>,
  SectionResultsView: ({ response }: SectionResultsViewProps) => (
    <p>Section results page {response.page.page}</p>
  ),
}));

const SCHEMA = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;

const known = (value: string) => ({
  knowledge: 'KNOWN',
  presence: { presence: 'PRESENT', value },
} as const);

const point = {
  contentVersion: 1,
  observationId: '20000000-0000-4000-8000-000000000001',
  observedAt: '2026-07-15T00:00:00Z',
} as const;

const discovery: CatalogDiscoveryResponseV1 = {
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
  subjects: [],
  targets: [{
    campusLabel: known('New Brunswick'),
    key: { campus: 'NB', term: '2026-9' },
    provenance: {
      ...point,
      payloadDigest: 'a'.repeat(64),
      sourceId: 'synthetic-selector',
      sourceKind: 'SELECTOR',
    },
    termLabel: known('Fall 2026'),
  }],
};

const shellState: Extract<ShellDataState, { status: 'READY' }> = {
  discovery,
  discoveryState: 'CURRENT',
  filterCount: 22,
  filterSchema: SCHEMA,
  status: 'READY',
};

function response(page: number): CourseQueryResponseV1 {
  return {
    contractVersion: 1,
    items: [{}] as CourseQueryResponseV1['items'],
    page: { page, pageSize: 25, total: 50, totalPages: 2 },
  };
}

function runtimeWith(product: Partial<ProductApiPort>): ProductRuntimePort {
  return {
    dispose() {},
    product: product as ProductApiPort,
    watch: {} as ProductRuntimePort['watch'],
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('search focus continuity', () => {
  it('moves through search, pagination, detail, and back without losing the output context', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>()
      .mockResolvedValueOnce(response(1))
      .mockResolvedValueOnce(response(2));
    const courseDetail = vi.fn<ProductApiPort['courseDetail']>()
      .mockResolvedValue({ contractVersion: 1, course: {} } as CourseDetailResponseV1);
    const view = render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchWorkspace
            runtime={runtimeWith({ courseDetail, searchCourses })}
            shellState={shellState}
          />
        </AppRouterProvider>
      </BcspI18nProvider>,
    );
    const output = view.container.querySelector<HTMLElement>('.bcsp-search-workspace__results')!;
    const outputHeading = screen.getByRole('heading', { name: 'Result index' });
    const scrollIntoView = vi.fn();
    Object.defineProperty(output, 'scrollIntoView', { configurable: true, value: scrollIntoView });

    expect(document.activeElement).not.toBe(outputHeading);
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await screen.findByText('Course results page 1');
    await waitFor(() => expect(document.activeElement).toBe(outputHeading));

    fireEvent.click(screen.getByRole('button', { name: 'Next result page' }));
    await screen.findByText('Course results page 2');
    await waitFor(() => expect(document.activeElement).toBe(outputHeading));
    expect(searchCourses.mock.calls[1]?.[0].page).toEqual({ page: 2, pageSize: 25 });

    const detailTrigger = screen.getByRole('button', { name: 'Open course detail' });
    detailTrigger.focus();
    fireEvent.click(detailTrigger);
    await screen.findByRole('heading', { name: 'Resolved course detail' });
    await waitFor(() => expect(document.activeElement).toBe(outputHeading));

    fireEvent.click(screen.getByRole('button', { name: /Back/u }));
    await screen.findByText('Course results page 2');
    await waitFor(() => expect(document.activeElement).toBe(
      screen.getByRole('button', { name: 'Open course detail' }),
    ));
    expect(scrollIntoView).toHaveBeenCalledWith({ behavior: 'auto', block: 'start' });
  });
});
