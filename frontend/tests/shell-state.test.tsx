// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  CatalogDiscoveryAvailability,
  CatalogDiscoveryResponseV1,
  FilterFieldSchemaV1,
  FilterSchemaV1,
  ProductApiPort,
  ProductRuntimePort,
} from '../src/ui/shared/product';
import { useShellDataState } from '../src/ui/shared/shell';

const OBSERVED_AT = '2026-07-15T00:00:00Z';

const TERM_FIELD = {
  stableId: 'FLT-C01',
  requestField: 'term',
  scope: 'COURSE',
  valueKind: 'TERM_ID',
  neutral: 'REQUIRED',
  default: 'REQUIRED',
  canonicalNeutral: { kind: 'REQUIRED' },
  normalization: ['CANONICAL_IDENTITY'],
  validation: ['REQUIRED'],
  queryEncoding: 'EXACT_ONE',
  i18nKey: 'filters.term',
  chipOrder: 1,
} satisfies FilterFieldSchemaV1;

const FILTER_SCHEMA = {
  contractVersion: 1,
  fields: [
    TERM_FIELD,
    {
      ...TERM_FIELD,
      stableId: 'FLT-C02',
      requestField: 'campuses',
      valueKind: 'CAMPUS_CODE_SET',
      i18nKey: 'filters.campuses',
      chipOrder: 2,
    },
  ],
} satisfies FilterSchemaV1;

interface Deferred<T> {
  readonly promise: Promise<T>;
  resolve(value: T): void;
  reject(error: unknown): void;
}

function deferred<T>(): Deferred<T> {
  let resolvePromise!: (value: T) => void;
  let rejectPromise!: (error: unknown) => void;
  const promise = new Promise<T>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });
  return { promise, reject: rejectPromise, resolve: resolvePromise };
}

function discovery(
  availability: CatalogDiscoveryAvailability,
  targetCount = 1,
  observationId = 'synthetic-observation',
): CatalogDiscoveryResponseV1 {
  const point = {
    observationId,
    observedAt: OBSERVED_AT,
    contentVersion: 1,
  };
  const provenance = {
    observationId,
    sourceId: 'synthetic-selector',
    sourceKind: 'SELECTOR' as const,
    observedAt: OBSERVED_AT,
    payloadDigest: 'synthetic-digest',
  };
  return {
    contractVersion: 1,
    observedAt: OBSERVED_AT,
    status: {
      availability,
      latestAttempt: point,
      lastSuccess: availability === 'UNAVAILABLE_NO_FIRST_SUCCESS' ? null : point,
      isStale: availability !== 'CURRENT',
      error: availability === 'CURRENT' ? null : { class: 'TRANSPORT', code: 'SYNTHETIC' },
    },
    sources: [],
    targets: Array.from({ length: targetCount }, (_, index) => ({
      key: { term: `SYNTHETIC-${index}`, campus: 'TEST' },
      termLabel: { knowledge: 'KNOWN', presence: { presence: 'PRESENT', value: 'Synthetic term' } },
      campusLabel: { knowledge: 'KNOWN', presence: { presence: 'PRESENT', value: 'Test campus' } },
      provenance,
    })),
    subjects: [],
  };
}

function unused(): never {
  throw new Error('unexpected ProductApiPort call');
}

function runtimeWith(product: Pick<ProductApiPort, 'filterSchema' | 'catalogDiscovery'>) {
  const api: ProductApiPort = {
    ...product,
    courseDetail: unused,
    openSectionStatus: unused,
    openStatus: unused,
    searchCourses: unused,
    searchSections: unused,
    sectionDetail: unused,
  };
  return {
    product: api,
    watch: {
      state: 'IDLE',
      connect: unused,
      disconnect: unused,
      send: unused,
      subscribe: () => () => undefined,
      subscribeState: () => () => undefined,
    },
    dispose: unused,
  } satisfies ProductRuntimePort;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('shell data state', () => {
  it('loads schema and discovery concurrently and publishes the exact ready snapshot', async () => {
    const schemaRequest = deferred<FilterSchemaV1>();
    const discoveryRequest = deferred<CatalogDiscoveryResponseV1>();
    const signals: AbortSignal[] = [];
    const runtime = runtimeWith({
      filterSchema: vi.fn((_signal) => {
        if (_signal !== undefined) signals.push(_signal);
        return schemaRequest.promise;
      }),
      catalogDiscovery: vi.fn((request, signal) => {
        expect(request).toEqual({ contractVersion: 1 });
        if (signal !== undefined) signals.push(signal);
        return discoveryRequest.promise;
      }),
    });

    const { result } = renderHook(() => useShellDataState(runtime));
    expect(result.current.state).toEqual({ status: 'LOADING' });
    expect(signals).toHaveLength(2);
    expect(signals[0]).toBe(signals[1]);

    const response = discovery('CURRENT');
    await act(async () => {
      schemaRequest.resolve(FILTER_SCHEMA);
      discoveryRequest.resolve(response);
      await Promise.all([schemaRequest.promise, discoveryRequest.promise]);
    });

    expect(result.current.state).toMatchObject({
      status: 'READY',
      discoveryState: 'CURRENT',
      discovery: response,
      filterCount: 2,
      filterSchema: FILTER_SCHEMA,
    });
  });

  it.each([
    ['STALE_LAST_SUCCESS', 1, 'READY', 'STALE_LAST_SUCCESS'],
    ['UNAVAILABLE_NO_FIRST_SUCCESS', 1, 'EMPTY', 'UNAVAILABLE'],
    ['CURRENT', 0, 'EMPTY', 'CURRENT'],
  ] as const)(
    'classifies %s with %i targets as %s without hiding provenance',
    async (availability, targetCount, status, discoveryState) => {
      const response = discovery(availability, targetCount);
      const runtime = runtimeWith({
        filterSchema: async () => FILTER_SCHEMA,
        catalogDiscovery: async () => response,
      });

      const { result } = renderHook(() => useShellDataState(runtime));
      await waitFor(() => expect(result.current.state.status).toBe(status));
      expect(result.current.state).toMatchObject({
        discovery: response,
        discoveryState,
        filterCount: 2,
      });
    },
  );

  it('aborts failed and retried generations and ignores promises that settle late', async () => {
    const firstSchema = deferred<FilterSchemaV1>();
    const firstDiscovery = deferred<CatalogDiscoveryResponseV1>();
    const secondSchema = deferred<FilterSchemaV1>();
    const secondDiscovery = deferred<CatalogDiscoveryResponseV1>();
    const schemas = [firstSchema, secondSchema];
    const discoveries = [firstDiscovery, secondDiscovery];
    const signals: AbortSignal[] = [];
    let schemaCall = 0;
    let discoveryCall = 0;
    const runtime = runtimeWith({
      filterSchema: (_signal) => {
        if (_signal !== undefined) signals.push(_signal);
        const request = schemas[schemaCall];
        schemaCall += 1;
        if (request === undefined) throw new Error('unexpected schema retry');
        return request.promise;
      },
      catalogDiscovery: (_request, signal) => {
        if (signal !== undefined) signals.push(signal);
        const request = discoveries[discoveryCall];
        discoveryCall += 1;
        if (request === undefined) throw new Error('unexpected discovery retry');
        return request.promise;
      },
    });

    const { result } = renderHook(() => useShellDataState(runtime));
    act(() => result.current.retry());
    await waitFor(() => expect(schemaCall).toBe(2));
    expect(signals[0]?.aborted).toBe(true);
    expect(signals[1]?.aborted).toBe(true);

    const current = discovery('CURRENT', 1, 'current-generation');
    await act(async () => {
      secondSchema.resolve(FILTER_SCHEMA);
      secondDiscovery.resolve(current);
      await Promise.all([secondSchema.promise, secondDiscovery.promise]);
    });
    expect(result.current.state).toMatchObject({
      status: 'READY',
      discovery: current,
    });

    const stale = discovery('STALE_LAST_SUCCESS', 1, 'aborted-generation');
    await act(async () => {
      firstSchema.resolve({ ...FILTER_SCHEMA, fields: [] });
      firstDiscovery.resolve(stale);
      await Promise.all([firstSchema.promise, firstDiscovery.promise]);
    });
    expect(result.current.state).toMatchObject({
      status: 'READY',
      discovery: current,
      filterCount: 2,
    });
  });

  it('exposes request failures as retryable errors', async () => {
    let attempt = 0;
    const runtime = runtimeWith({
      filterSchema: async () => FILTER_SCHEMA,
      catalogDiscovery: async () => {
        attempt += 1;
        if (attempt === 1) throw new Error('synthetic outage');
        return discovery('CURRENT');
      },
    });

    const { result } = renderHook(() => useShellDataState(runtime));
    await waitFor(() => expect(result.current.state).toEqual({
      status: 'ERROR',
      reason: 'SHELL_DATA_REQUEST_FAILED',
      message: 'synthetic outage',
    }));

    act(() => result.current.retry());
    await waitFor(() => expect(result.current.state.status).toBe('READY'));
    expect(attempt).toBe(2);
  });
});
