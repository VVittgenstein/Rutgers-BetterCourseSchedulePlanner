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

function runtimeWith(
  catalog: ProductApiPort['catalogDiscovery'],
  schema: ProductApiPort['filterSchema'] = async () => filterSchema,
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
    renderShell(runtimeWith(async () => pending));

    expect((await screen.findByRole('status')).getAttribute('data-state')).toBe('loading');
    expect(screen.getByRole('heading', { name: 'Opening the catalog console' })).toBeTruthy();
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
    expect(screen.getByText('Catalog current')).toBeTruthy();
    expect(screen.getByText('Not observed yet')).toBeTruthy();
    expect(screen.getByRole('group', { name: 'Interface language' })).toBeTruthy();

    const accessibility = await axe.run(view.baseElement, {
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(accessibility.violations).toEqual([]);
  });

  it('distinguishes stale, valid-empty, and request-error states without fallback data', async () => {
    const stale = renderShell(runtimeWith(async () => discovery('STALE_LAST_SUCCESS')));
    expect(await screen.findByText('Catalog stale')).toBeTruthy();
    stale.unmount();

    const empty = renderShell(runtimeWith(async () => discovery('CURRENT', 0)));
    expect(await screen.findByRole('heading', { name: 'No catalog targets available' })).toBeTruthy();
    expect(screen.queryByText('Fall 2026')).toBeNull();
    empty.unmount();

    const catalogDiscovery = vi.fn<ProductApiPort['catalogDiscovery']>()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce(discovery());
    renderShell(runtimeWith(catalogDiscovery));
    expect(await screen.findByRole('alert')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(screen.getByText('Catalog current')).toBeTruthy());
    expect(catalogDiscovery).toHaveBeenCalledTimes(2);
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
