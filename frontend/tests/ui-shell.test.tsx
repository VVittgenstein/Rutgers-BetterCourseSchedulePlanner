// @vitest-environment jsdom

import axe from 'axe-core';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { PublicCompositionRoot } from '../src/ui/public/PublicCompositionRoot';
import { BCSP_SHELL_CSS } from '../src/ui/shared/application';
import { BCSP_DESIGN_SYSTEM_CSS } from '../src/ui/shared/design-system';
import type {
  CatalogDiscoveryResponseV1,
  FilterSchemaV1,
  ProductApiPort,
  ProductRuntimePort,
  ServiceStatusV2,
} from '../src/ui/shared/product';

const BOOTSTRAP = {
  protocolVersion: 1,
  data: { sessionNonce: '10000000-0000-4000-8000-000000000001' },
};
const OBSERVED_AT = '2026-07-14T16:30:00.000Z';

const filterSchema = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;

function known(value: string) {
  return { knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } } as const;
}

function luminance(hex: string): number {
  const channels = hex.match(/[0-9a-f]{2}/giu)?.map((value) => Number.parseInt(value, 16) / 255);
  if (channels?.length !== 3) throw new Error(`invalid color ${hex}`);
  const linear = channels.map((value) => value <= 0.04045
    ? value / 12.92
    : ((value + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
}

function contrast(left: string, right: string): number {
  const values = [luminance(left), luminance(right)];
  return (Math.max(...values) + 0.05) / (Math.min(...values) + 0.05);
}

function discovery(
  availability: CatalogDiscoveryResponseV1['status']['availability'] = 'CURRENT',
  targetCount = 2,
): CatalogDiscoveryResponseV1 {
  const point = {
    contentVersion: 7,
    observationId: '10000000-0000-4000-8000-000000000007',
    observedAt: OBSERVED_AT,
  } as const;
  const provenance = {
    observationId: point.observationId,
    observedAt: OBSERVED_AT,
    payloadDigest: 'a'.repeat(64),
    sourceId: 'rutgers-selector',
    sourceKind: 'SELECTOR',
  } as const;
  return {
    contractVersion: 1,
    observedAt: OBSERVED_AT,
    sources: [],
    coreCodeDictionaries: [],
    status: {
      availability,
      error: availability === 'UNAVAILABLE_NO_FIRST_SUCCESS'
        ? { class: 'TRANSPORT', code: 'DISCOVERY_UNAVAILABLE' }
        : null,
      isStale: availability !== 'CURRENT',
      lastSuccess: availability === 'UNAVAILABLE_NO_FIRST_SUCCESS' ? null : point,
      latestAttempt: point,
    },
    subjects: [],
    targets: Array.from({ length: targetCount }, (_, index) => ({
      campusLabel: known(index === 0
        ? 'New Brunswick'
        : index === 1 ? 'Newark' : `Published campus ${index + 1}`),
      key: {
        campus: index === 0 ? 'NB' : index === 1 ? 'NK' : `C${String(index + 1).padStart(3, '0')}`,
        term: '2026-9',
      },
      provenance,
      termLabel: known('Fall 2026'),
    })),
  };
}

function serviceStatus(
  level: ServiceStatusV2['level'] = 'READY',
  availability: 'CURRENT' | 'STALE' | 'UNAVAILABLE' = 'CURRENT',
  targetCount = 2,
): ServiceStatusV2 {
  const targets = discovery('CURRENT', targetCount).targets.map(({ key }) => ({
    catalogContentVersion: availability === 'UNAVAILABLE' ? null : 7,
    error: null,
    lastCompleteAt: availability === 'UNAVAILABLE' ? null : OBSERVED_AT,
    nextRetryAt: availability === 'STALE' ? '2026-07-14T16:31:00.000Z' : null,
    primary: key.campus === 'NB',
    snapshotAvailability: availability === 'UNAVAILABLE' ? 'NO_COMPLETE_SNAPSHOT' as const : 'READY' as const,
    stage: availability === 'UNAVAILABLE' ? 'CATALOG_FETCH' as const : null,
    target: key,
    usable: availability !== 'UNAVAILABLE',
    workState: availability === 'STALE' ? 'RETRY_WAIT' as const
      : availability === 'UNAVAILABLE' ? 'RUNNING' as const : 'IDLE' as const,
  }));
  const readyTargetCount = targets.filter(({ usable }) => usable).length;
  return {
    contractVersion: 2,
    discovery: discovery(targetCount === 0 ? 'CURRENT' : availability === 'STALE'
      ? 'STALE_LAST_SUCCESS' : 'CURRENT', targetCount).status,
    issues: level === 'DEGRADED' ? [{
      code: 'SYNTHETIC_STALE', component: 'CATALOG', recovery: 'AUTOMATIC_RETRY',
      retryAt: '2026-07-14T16:31:00.000Z', severity: 'DEGRADED', target: targets[0]?.target ?? null,
    }] : [],
    level,
    observedAt: OBSERVED_AT,
    operations: availability === 'UNAVAILABLE' && targets[0] !== undefined ? [{
      target: targets[0].target,
      stage: 'CATALOG_FETCH',
      startedAt: OBSERVED_AT,
    }] : [],
    runtime: 'PUBLIC',
    termWindow: {
      currentTerm: '2026-9',
      nextTerm: '2027-0',
      visibleTerms: [
        { term: '2026-9', relativeOffset: 0, discovered: targetCount > 0, autoManaged: true, manualPullAllowed: false, watchable: true },
        { term: '2027-0', relativeOffset: 1, discovered: false, autoManaged: true, manualPullAllowed: false, watchable: true },
      ],
    },
    automaticTermSummaries: [
      { term: '2026-9', readyTargetCount, totalTargetCount: targetCount },
      { term: '2027-0', readyTargetCount: 0, totalTargetCount: 0 },
    ],
    targets,
  };
}

function runtimeWith(
  catalog: ProductApiPort['catalogDiscovery'],
  schema: ProductApiPort['filterSchema'] = async () => filterSchema,
  status: ProductApiPort['serviceStatus'] = async () => serviceStatus(),
  overrides: Partial<ProductApiPort> = {},
): ProductRuntimePort {
  const uncalled = async () => {
    throw new Error('not used by the shell foundation');
  };
  return {
    dispose: vi.fn(),
    product: {
      catalogDiscovery: catalog,
      courseDetail: uncalled,
      filterSchema: schema,
      openSectionStatus: uncalled,
      openStatus: uncalled,
      serviceStatus: status,
      searchCourses: uncalled,
      searchSections: uncalled,
      sectionDetail: uncalled,
      ...overrides,
    } as ProductApiPort,
    watch: {
      state: 'IDLE',
      connect: vi.fn(),
      disconnect: vi.fn(),
      send: vi.fn(() => '00000000-0000-4000-8000-000000000001'),
      subscribe: vi.fn(() => () => undefined),
      subscribeState: vi.fn(() => () => undefined),
    },
  };
}

function renderShell(runtime: ProductRuntimePort, locale = 'en-US') {
  return render(
    <PublicCompositionRoot
      localeSource={{ languages: [locale] }}
      product={{ bootstrap: BOOTSTRAP, runtimeFactory: () => runtime }}
    />,
  );
}

afterEach(() => {
  cleanup();
  document.documentElement.lang = '';
  vi.restoreAllMocks();
});

describe('P7.2 responsive product shell', () => {
  it('renders a truthful loading state while both shell resources are pending', async () => {
    const pending = new Promise<CatalogDiscoveryResponseV1>(() => undefined);
    renderShell(runtimeWith(
      async () => pending,
      undefined,
      async () => serviceStatus('INITIALIZING', 'UNAVAILABLE', 0),
    ));

    expect(await screen.findByRole('heading', { name: 'Opening the catalog console' })).toBeTruthy();
    expect(document.querySelector('[data-state="loading"]')).not.toBeNull();
    expect(screen.getByText('Preparing course data')).toBeTruthy();
    expect(screen.getByText('Starting the local service')).toBeTruthy();
    expect(screen.getByRole('progressbar', {
      name: 'Overall course data loading progress',
    })).toBeTruthy();
  });

  it('renders current metrics and lets native controls select published targets', async () => {
    const view = renderShell(runtimeWith(async () => discovery()));

    const term = await screen.findByRole('radio', { name: /Fall 2026/u }) as HTMLInputElement;
    const newark = screen.getByRole('checkbox', { name: /Newark \/ NK/i }) as HTMLInputElement;
    const newBrunswick = screen.getByRole('checkbox', { name: /New Brunswick \/ NB/i }) as HTMLInputElement;
    expect(term.checked).toBe(true);
    expect(newBrunswick.checked).toBe(false);
    expect(newark.checked).toBe(false);
    fireEvent.click(newark);
    fireEvent.click(newBrunswick);
    expect(newark.checked).toBe(true);
    expect(newBrunswick.checked).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));

    expect(view.container.querySelectorAll('[data-filter-row]')).toHaveLength(16);
    expect(screen.getByText('All course data is ready')).toBeTruthy();
    expect(screen.getAllByText('2/2')).toHaveLength(1);
    expect(screen.getByRole('group', { name: 'Interface language' })).toBeTruthy();
    expect(screen.queryByRole('link', { name: /Sections/u })).toBeNull();
    expect(screen.getAllByRole('link', { name: /Courses|Watch desk/u })).toHaveLength(2);
    const status = screen.getByRole('region', { name: 'System status' });
    expect(status.closest('.bcsp-workspace__heading')).not.toBeNull();

    const accessibility = await axe.run(view.baseElement, {
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(accessibility.violations).toEqual([]);
  }, 15_000);

  it('reports partial target readiness without claiming that all data is ready', async () => {
    const complete = serviceStatus('PARTIALLY_READY', 'CURRENT', 2);
    const second = complete.targets[1]!;
    const partial: ServiceStatusV2 = {
      ...complete,
      automaticTermSummaries: [
        { term: '2026-9', readyTargetCount: 1, totalTargetCount: 2 },
        { term: '2027-0', readyTargetCount: 0, totalTargetCount: 0 },
      ],
      targets: [complete.targets[0]!, {
        ...second,
        catalogContentVersion: null,
        lastCompleteAt: null,
        snapshotAvailability: 'NO_COMPLETE_SNAPSHOT',
        stage: 'CATALOG_FETCH',
        usable: false,
        workState: 'RETRY_WAIT',
      }],
    };
    renderShell(runtimeWith(async () => discovery(), undefined, async () => partial));

    expect(await screen.findByText(
      /1 Campus target is available|1 Campus targets are available/u,
      { selector: '.bcsp-service-status__headline' },
    )).toBeTruthy();
    expect(screen.queryByText('All course data is ready')).toBeNull();
    expect(screen.queryByText('Loading is incomplete; retrying')).toBeNull();
    expect(screen.getByText('1/2')).toBeTruthy();
  });

  it('expands a retained ready snapshot when the service connection is interrupted', async () => {
    const statusRequest = vi.fn<ProductApiPort['serviceStatus']>()
      .mockResolvedValueOnce(serviceStatus())
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue(serviceStatus());
    renderShell(runtimeWith(async () => discovery(), undefined, statusRequest));

    expect(await screen.findByText('All course data is ready')).toBeTruthy();
    globalThis.dispatchEvent(new Event('online'));

    expect(await screen.findByText(
      'Service connection interrupted',
      { selector: '.bcsp-service-status__headline' },
    )).toBeTruthy();
    expect(screen.getByText(
      'Service connection interrupted',
      { selector: '.bcsp-service-status__diagnostics p' },
    )).toBeTruthy();
    expect(screen.queryByText('All course data is ready')).toBeNull();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy();
  });

  it('distinguishes stale, valid-empty, and request-error states without fallback data', async () => {
    const stale = renderShell(runtimeWith(
      async () => discovery('STALE_LAST_SUCCESS'),
      undefined,
      async () => serviceStatus('DEGRADED', 'STALE'),
    ));
    expect(await screen.findByText('2/2')).toBeTruthy();
    expect(screen.getAllByText(/Course data remains available; a refresh failed and will retry/u).length)
      .toBeGreaterThan(0);
    expect(screen.queryByText('All course data is ready')).toBeNull();
    expect(screen.getAllByText(/Refresh failed; retry scheduled/u).length).toBeGreaterThan(0);
    stale.unmount();

    const empty = renderShell(runtimeWith(
      async () => discovery('CURRENT', 0),
      undefined,
      async () => serviceStatus('INITIALIZING', 'UNAVAILABLE', 0),
    ));
    expect(await screen.findByRole('heading', { name: 'Search courses' })).toBeTruthy();
    expect(screen.getAllByRole('radio')).toHaveLength(2);
    expect(screen.getByText('Course targets have not been published for this term.')).toBeTruthy();
    empty.unmount();

    const catalogDiscovery = vi.fn<ProductApiPort['catalogDiscovery']>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue(discovery());
    renderShell(runtimeWith(catalogDiscovery));
    expect(await screen.findByRole('alert')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(screen.getByRole('radio', { name: /Fall 2026/u })).toBeTruthy());
    expect(catalogDiscovery.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it('keeps the submitted Course result state while visiting Watch and returning', async () => {
    const searchCourses = vi.fn<ProductApiPort['searchCourses']>().mockResolvedValue({
      contractVersion: 2,
      items: [],
      page: { page: 1, pageSize: 25, total: 0, totalPages: 0 },
    });
    renderShell(runtimeWith(
      async () => discovery(),
      undefined,
      async () => serviceStatus(),
      { searchCourses },
    ));

    await screen.findByText('All course data is ready');
    fireEvent.click(screen.getByRole('checkbox', { name: /New Brunswick \/ NB/i }));
    fireEvent.click(screen.getByRole('button', { name: 'Apply' }));
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(searchCourses).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('link', { name: /Watch desk/u }));
    await waitFor(() => expect(
      screen.getByRole('link', { name: /Watch desk/u }).getAttribute('aria-current'),
    ).toBe('page'));
    expect(screen.queryByRole('heading', { name: 'No matching records' })).toBeNull();

    fireEvent.click(screen.getByRole('link', { name: /Courses/u }));
    expect(await screen.findByRole('heading', { name: 'No matching records' })).toBeTruthy();
    expect(searchCourses).toHaveBeenCalledTimes(1);
  });

  it('keeps the chosen ink, paper, accent, focus, and reduced-motion rules in the shared token layer', () => {
    expect(BCSP_DESIGN_SYSTEM_CSS).toContain('--bcsp-paper: #efeee8');
    expect(BCSP_DESIGN_SYSTEM_CSS).toContain('--bcsp-ink: #11110e');
    expect(BCSP_DESIGN_SYSTEM_CSS).toContain('--bcsp-accent: #d42b1e');
    expect(BCSP_DESIGN_SYSTEM_CSS).toContain(':focus-visible');
    expect(BCSP_DESIGN_SYSTEM_CSS).toContain('@media (prefers-reduced-motion: reduce)');
    expect(
      [...BCSP_DESIGN_SYSTEM_CSS.matchAll(/border-radius:\s*([^;]+);/gu)]
        .map((match) => match[1]?.trim()),
    ).toEqual(['0']);
    expect(BCSP_DESIGN_SYSTEM_CSS).not.toMatch(/linear-gradient|box-shadow/u);
    expect(contrast('11110e', 'efeee8')).toBeGreaterThan(7);
    expect(contrast('5b5a53', 'efeee8')).toBeGreaterThan(4.5);
    expect(contrast('ffffff', 'd42b1e')).toBeGreaterThan(4.5);
    expect(BCSP_SHELL_CSS).toMatch(/\.bcsp-navigation\s*\{[\s\S]*?position:\s*sticky;/u);
    expect(BCSP_SHELL_CSS).toMatch(/\.bcsp-navigation\s*\{[\s\S]*?top:\s*0;/u);
    expect(BCSP_SHELL_CSS).toContain('scroll-margin-top: var(--bcsp-navigation-height');
  });
});
