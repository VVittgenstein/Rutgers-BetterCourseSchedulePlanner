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
  ServiceStatusV2,
  SectionDetailResponseV1,
} from '../src/ui/shared/product';
import { AppRouterProvider, RouterLink, useAppRouter } from '../src/ui/shared/routing';
import type {
  CourseDetailViewProps,
  CourseResultsViewProps,
  SectionDetailViewProps,
  SectionResultsViewProps,
} from '../src/ui/shared/search/results';
import { SearchSessionProvider, SearchWorkspace } from '../src/ui/shared/search';
import type { ShellDataState } from '../src/ui/shared/shell';

vi.mock('../src/ui/shared/search/results', () => ({
  CourseDetailView: (_props: CourseDetailViewProps) => <h2>Resolved course detail</h2>,
  CourseResultsView: ({
    expandedSectionDisclosures,
    onCourseDetail,
    onPageChange,
    onSectionDisclosureChange,
    onSectionNavigate,
    response,
  }: CourseResultsViewProps) => (
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
      <button
        aria-expanded={expandedSectionDisclosures?.has('mock-variant') ?? false}
        onClick={() => onSectionDisclosureChange?.(
          'mock-variant',
          !(expandedSectionDisclosures?.has('mock-variant') ?? false),
        )}
        type="button"
      >
        Toggle Section disclosure
      </button>
      <button
        onClick={() => onSectionNavigate?.({ campus: 'NB', index: '12345', term: '2026-9' })}
        type="button"
      >
        Open Section detail
      </button>
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
  filterCount: 18,
  filterSchema: SCHEMA,
  status: 'READY',
};

function response(page: number): CourseQueryResponseV1 {
  return {
    contractVersion: 2,
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

const readyStatus = {
  contractVersion: 2,
  observedAt: point.observedAt,
  runtime: 'LOCAL',
  level: 'PARTIALLY_READY',
  discovery: discovery.status,
  termWindow: {
    currentTerm: '2026-9',
    nextTerm: '2027-0',
    visibleTerms: [
      { term: '2026-9', relativeOffset: 0, discovered: true, autoManaged: true, manualPullAllowed: false, watchable: true },
      { term: '2027-0', relativeOffset: 1, discovered: false, autoManaged: true, manualPullAllowed: false, watchable: true },
    ],
  },
  automaticTermSummaries: [{ term: '2026-9', readyTargetCount: 1, totalTargetCount: 1 }],
  operations: [],
  targets: [{
    target: { campus: 'NB', term: '2026-9' }, primary: true,
    snapshotAvailability: 'READY', workState: 'IDLE', stage: null, usable: true,
    catalogContentVersion: 1, lastCompleteAt: point.observedAt, nextRetryAt: null, error: null,
  }],
  issues: [],
} satisfies ServiceStatusV2;

function applyScope(): void {
  fireEvent.click(screen.getByRole('checkbox', { name: /New Brunswick/u }));
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
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
      .mockResolvedValue({ contractVersion: 2, course: {} } as CourseDetailResponseV1);
    const view = render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchWorkspace
            runtime={runtimeWith({ courseDetail, searchCourses })}
            serviceStatus={readyStatus}
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
    applyScope();
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

  it('keeps the successful session across failures, every top-level page, and Section detail', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>()
      .mockResolvedValueOnce(response(1))
      .mockRejectedValueOnce(new Error('synthetic search failure'))
      .mockResolvedValueOnce(response(2));
    const sectionDetail = vi.fn<ProductApiPort['sectionDetail']>().mockResolvedValue({
      contractVersion: 2,
      section: {},
      variant: {},
    } as SectionDetailResponseV1);
    const runtime = runtimeWith({ searchCourses, sectionDetail });

    function RouteHarness() {
      const { pathname } = useAppRouter();
      const courseRoute = pathname === '/' || pathname.startsWith('/sections/');
      return (
        <>
          <nav>
            <RouterLink to="/">Courses</RouterLink>
            <RouterLink to="/watch">Watch</RouterLink>
            <RouterLink to="/saved-views">Saved</RouterLink>
            <RouterLink to="/history">History</RouterLink>
            <RouterLink to="/settings">Settings</RouterLink>
          </nav>
          {courseRoute ? (
            <SearchWorkspace
              runtime={runtime}
              serviceStatus={readyStatus}
              shellState={shellState}
            />
          ) : <p>Top-level page {pathname}</p>}
        </>
      );
    }

    const view = render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchSessionProvider>
            <RouteHarness />
          </SearchSessionProvider>
        </AppRouterProvider>
      </BcspI18nProvider>,
    );

    applyScope();
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    await screen.findByText('Course results page 1');

    fireEvent.click(screen.getByText('Same-Section constraints'));
    fireEvent.change(screen.getByLabelText('Section indexes'), { target: { value: '54321' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Section indexes' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'Search could not be completed' })).toBeTruthy();
    expect(screen.getByText('Course results page 1')).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Next result page' }));
    await screen.findByText('Course results page 2');
    expect(searchCourses.mock.calls[2]?.[0].filters.values.sectionIndexes).toEqual([]);
    expect(searchCourses.mock.calls[2]?.[0].sort).toEqual({
      direction: 'DESCENDING',
      field: 'RELEVANCE',
    });

    const disclosure = screen.getByRole('button', { name: 'Toggle Section disclosure' });
    fireEvent.click(disclosure);
    expect(disclosure.getAttribute('aria-expanded')).toBe('true');
    const filterRegion = view.container.querySelector<HTMLElement>('.bcsp-search-workspace__filters');
    if (filterRegion === null) throw new Error('filter region is required');
    filterRegion.scrollTop = 176;
    fireEvent.scroll(filterRegion);

    for (const [label, path] of [
      ['Watch', '/watch'],
      ['Saved', '/saved-views'],
      ['History', '/history'],
      ['Settings', '/settings'],
    ] as const) {
      fireEvent.click(screen.getByRole('link', { name: label }));
      await screen.findByText(`Top-level page ${path}`);
      fireEvent.click(screen.getByRole('link', { name: 'Courses' }));
      await screen.findByText('Course results page 2');
      expect(screen.getAllByText('54321').length).toBeGreaterThan(0);
      expect(screen.getByRole('button', { name: 'Toggle Section disclosure' })
        .getAttribute('aria-expanded')).toBe('true');
      expect(view.container.querySelector<HTMLElement>('.bcsp-search-workspace__filters')?.scrollTop)
        .toBe(176);
    }

    fireEvent.click(screen.getByRole('button', { name: 'Open Section detail' }));
    expect(await screen.findByRole('heading', { name: 'Resolved section detail' })).toBeTruthy();
    fireEvent.click(screen.getByRole('link', { name: /Back to course results/iu }));
    await screen.findByText('Course results page 2');
    expect(screen.getAllByText('54321').length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: 'Toggle Section disclosure' })
      .getAttribute('aria-expanded')).toBe('true');
    expect(searchCourses).toHaveBeenCalledTimes(3);
  });
});
