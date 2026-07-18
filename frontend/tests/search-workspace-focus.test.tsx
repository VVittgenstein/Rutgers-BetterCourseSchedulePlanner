// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { useState } from 'react';
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
        onClick={() => onCourseDetail({ campus: 'NB', courseString: '01:198:211', term: '72026' })}
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
        onClick={() => onSectionNavigate?.({ campus: 'NB', index: '12345', term: '72026' })}
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
const TERM = '72026';
const NEXT_TERM = '92026';

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
  coreCodeDictionaries: [],
  subjects: [],
  targets: [{
    campusLabel: known('New Brunswick'),
    key: { campus: 'NB', term: TERM },
    provenance: {
      ...point,
      payloadDigest: 'a'.repeat(64),
      sourceId: 'synthetic-selector',
      sourceKind: 'SELECTOR',
    },
    termLabel: known('Upstream label must not render'),
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
    contractVersion: 3,
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
  automaticTermSummaries: [{ term: TERM, readyTargetCount: 1, totalTargetCount: 3 }],
  operations: [],
  targets: [{
    target: { campus: 'NB', term: TERM }, primary: true,
    snapshotAvailability: 'READY', workState: 'IDLE', stage: null, usable: true,
    catalogContentVersion: 1, lastCompleteAt: point.observedAt, nextRetryAt: null, error: null,
  }],
  issues: [],
} satisfies ServiceStatusV2;

function applyScope(): void {
  fireEvent.click(screen.getByRole('checkbox', { name: /NB/u }));
  fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('search focus continuity', () => {
  it('restores the independently scrolling filter rail after the Course workspace remounts', async () => {
    const scrollTo = vi.spyOn(globalThis, 'scrollTo').mockImplementation(() => undefined);
    const runtime = runtimeWith({});

    function RemountHarness() {
      const [mounted, setMounted] = useState(true);
      return (
        <>
          <button onClick={() => setMounted((current) => !current)} type="button">
            {mounted ? 'Leave Courses' : 'Return to Courses'}
          </button>
          {mounted ? (
            <SearchWorkspace runtime={runtime} serviceStatus={readyStatus} shellState={shellState} />
          ) : <p>Another workspace</p>}
        </>
      );
    }

    render(
      <BcspI18nProvider initialLocale="en-US">
        <AppRouterProvider initialPath="/">
          <SearchSessionProvider>
            <RemountHarness />
          </SearchSessionProvider>
        </AppRouterProvider>
      </BcspI18nProvider>,
    );

    const firstFilterRail = document.querySelector<HTMLElement>('.bcsp-search-workspace__filters');
    expect(firstFilterRail).not.toBeNull();
    if (firstFilterRail === null) throw new Error('Expected the filter rail.');
    firstFilterRail.scrollTop = 731;
    fireEvent.scroll(firstFilterRail);

    fireEvent.click(screen.getByRole('button', { name: 'Leave Courses' }));
    expect(screen.getByText('Another workspace')).toBeTruthy();
    scrollTo.mockClear();
    fireEvent.click(screen.getByRole('button', { name: 'Return to Courses' }));

    const restoredFilterRail = document.querySelector<HTMLElement>('.bcsp-search-workspace__filters');
    expect(restoredFilterRail).not.toBe(firstFilterRail);
    await waitFor(() => expect(restoredFilterRail?.scrollTop).toBe(731));
    expect(scrollTo).not.toHaveBeenCalled();
  });

  it('moves through search, pagination, detail, and back without losing the output context', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>()
      .mockResolvedValueOnce(response(1))
      .mockResolvedValueOnce(response(2));
    const courseDetail = vi.fn<ProductApiPort['courseDetail']>()
      .mockResolvedValue({ contractVersion: 3, course: {} } as CourseDetailResponseV1);
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
    const outputHeading = view.container.querySelector<HTMLElement>('#bcsp-search-results-title')!;
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
      contractVersion: 3,
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
          <main id="bcsp-workspace" tabIndex={-1}>
            {courseRoute ? (
              <SearchWorkspace
                runtime={runtime}
                serviceStatus={readyStatus}
                shellState={shellState}
              />
            ) : <p>Top-level page {pathname}</p>}
          </main>
        </>
      );
    }

    render(
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
    let pageScroll = 176;
    vi.spyOn(globalThis, 'scrollY', 'get').mockImplementation(() => pageScroll);
    const scrollTo = vi.spyOn(globalThis, 'scrollTo').mockImplementation((options) => {
      if (typeof options === 'object' && options !== null) pageScroll = options.top ?? 0;
    });

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
      await waitFor(() => expect(scrollTo).toHaveBeenCalledWith({ behavior: 'auto', left: 0, top: 176 }));
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
