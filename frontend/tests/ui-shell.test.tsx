// @vitest-environment jsdom

import axe from 'axe-core';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { PublicCompositionRoot } from '../src/ui/public/PublicCompositionRoot';
import { BCSP_DESIGN_SYSTEM_CSS } from '../src/ui/shared/design-system';
import type {
  CatalogDiscoveryResponseV1,
  FilterSchemaV1,
  ProductApiPort,
  ProductRuntimePort,
  ServiceStatusV1,
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
    targets: [
      {
        campusLabel: known('New Brunswick'),
        key: { campus: 'NB', term: '2026-9' },
        provenance,
        termLabel: known('Fall 2026'),
      },
      {
        campusLabel: known('Newark'),
        key: { campus: 'NK', term: '2026-9' },
        provenance,
        termLabel: known('Fall 2026'),
      },
    ].slice(0, targetCount),
  };
}

function serviceStatus(
  level: ServiceStatusV1['level'] = 'READY',
  availability: ServiceStatusV1['targets'][number]['catalogAvailability'] = 'CURRENT',
  targetCount = 2,
): ServiceStatusV1 {
  const targets = discovery('CURRENT', targetCount).targets.map(({ key }) => ({
    catalogAvailability: availability,
    catalogContentVersion: availability === 'UNAVAILABLE' ? null : 7,
    openAvailability: availability,
    primary: true,
    searchAvailable: availability !== 'UNAVAILABLE',
    target: key,
  }));
  const availableTargetCount = targets.filter(({ searchAvailable }) => searchAvailable).length;
  return {
    catalog: { availableTargetCount, totalTargetCount: targets.length },
    contractVersion: 1,
    discovery: discovery(targetCount === 0 ? 'CURRENT' : availability === 'STALE'
      ? 'STALE_LAST_SUCCESS' : 'CURRENT', targetCount).status,
    issues: level === 'DEGRADED' ? [{
      code: 'SYNTHETIC_STALE',
      component: 'CATALOG',
      recovery: 'AUTOMATIC_RETRY',
      retryAt: null,
      severity: 'DEGRADED',
      target: targets[0]?.target ?? null,
    }] : [],
    level,
    observedAt: OBSERVED_AT,
    open: { availableTargetCount, totalTargetCount: targets.length },
    operation: { nextRetryAt: null, phase: 'IDLE', startedAt: null, target: null },
    runtime: 'PUBLIC',
    targets,
  };
}

function runtimeWith(
  catalog: ProductApiPort['catalogDiscovery'],
  schema: ProductApiPort['filterSchema'] = async () => filterSchema,
  status: ProductApiPort['serviceStatus'] = async () => serviceStatus(),
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
    expect(screen.getByText('Initializing')).toBeTruthy();
  });

  it('renders current metrics and lets native controls select published targets', async () => {
    const view = renderShell(runtimeWith(async () => discovery()));

    const term = await screen.findByRole('combobox', { name: 'Term' }) as HTMLSelectElement;
    const newark = screen.getByRole('checkbox', { name: /Newark \/ NK/i }) as HTMLInputElement;
    const newBrunswick = screen.getByRole('checkbox', { name: /New Brunswick \/ NB/i }) as HTMLInputElement;
    expect(term.value).toBe('2026-9');
    expect(newBrunswick.checked).toBe(true);
    expect(newark.checked).toBe(false);
    fireEvent.click(newark);
    expect(newark.checked).toBe(true);
    expect(newBrunswick.checked).toBe(true);

    expect(screen.getAllByText('22').length).toBeGreaterThan(0);
    expect(screen.getByText('Search ready')).toBeTruthy();
    expect(screen.getAllByText('2/2')).toHaveLength(2);
    expect(screen.getByRole('group', { name: 'Interface language' })).toBeTruthy();

    const accessibility = await axe.run(view.baseElement, {
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(accessibility.violations).toEqual([]);
  });

  it('expands a retained ready snapshot when the service connection is interrupted', async () => {
    const statusRequest = vi.fn<ProductApiPort['serviceStatus']>()
      .mockResolvedValueOnce(serviceStatus())
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue(serviceStatus());
    renderShell(runtimeWith(async () => discovery(), undefined, statusRequest));

    expect(await screen.findByText('Search ready')).toBeTruthy();
    globalThis.dispatchEvent(new Event('online'));

    expect(await screen.findAllByText('Service connection interrupted')).toHaveLength(2);
    expect(screen.getByText(
      'The last service snapshot is retained while the browser reconnects with backoff.',
    )).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Retry' })).toBeTruthy();
  });

  it('distinguishes stale, valid-empty, and request-error states without fallback data', async () => {
    const stale = renderShell(runtimeWith(
      async () => discovery('STALE_LAST_SUCCESS'),
      undefined,
      async () => serviceStatus('DEGRADED', 'STALE'),
    ));
    expect(await screen.findByText('Running with degraded data')).toBeTruthy();
    stale.unmount();

    const empty = renderShell(runtimeWith(
      async () => discovery('CURRENT', 0),
      undefined,
      async () => serviceStatus('INITIALIZING', 'UNAVAILABLE', 0),
    ));
    expect(await screen.findByRole('heading', { name: 'No catalog targets available' })).toBeTruthy();
    expect(screen.queryByText('Fall 2026')).toBeNull();
    empty.unmount();

    const catalogDiscovery = vi.fn<ProductApiPort['catalogDiscovery']>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue(discovery());
    renderShell(runtimeWith(catalogDiscovery));
    expect(await screen.findByRole('alert')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Term' })).toBeTruthy());
    expect(catalogDiscovery.mock.calls.length).toBeGreaterThanOrEqual(2);
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
  });
});
