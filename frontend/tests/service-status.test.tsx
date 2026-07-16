// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  ProductApiPort,
  ProductRuntimePort,
  ServiceStatusV1,
} from '../src/ui/shared/product';
import {
  SERVICE_STATUS_POLLING,
  useServiceStatus,
  type ServiceStatusPollingSchedule,
} from '../src/ui/shared/shell';

const OBSERVED_AT = '2026-07-16T00:00:00Z';
const TEST_POLLING: ServiceStatusPollingSchedule = {
  activeMilliseconds: 15,
  idleMilliseconds: 200,
  requestTimeoutMilliseconds: 100,
  failureBackoffMilliseconds: [30, 60, 90],
};

function status(
  level: ServiceStatusV1['level'],
  phase: ServiceStatusV1['operation']['phase'],
): ServiceStatusV1 {
  return {
    catalog: { availableTargetCount: level === 'INITIALIZING' ? 0 : 1, totalTargetCount: 1 },
    contractVersion: 1,
    discovery: {
      availability: level === 'INITIALIZING' ? 'UNAVAILABLE_NO_FIRST_SUCCESS' : 'CURRENT',
      error: null,
      isStale: false,
      lastSuccess: null,
      latestAttempt: null,
    },
    issues: [],
    level,
    observedAt: OBSERVED_AT,
    open: { availableTargetCount: level === 'READY' ? 1 : 0, totalTargetCount: 1 },
    operation: { nextRetryAt: null, phase, startedAt: OBSERVED_AT, target: null },
    runtime: 'LOCAL',
    targets: [{
      catalogAvailability: level === 'INITIALIZING' ? 'UNAVAILABLE' : 'CURRENT',
      catalogContentVersion: level === 'INITIALIZING' ? null : 1,
      openAvailability: level === 'READY' ? 'CURRENT' : 'UNAVAILABLE',
      primary: true,
      searchAvailable: level !== 'INITIALIZING',
      target: { campus: 'NB', term: '92026' },
    }],
  };
}

function runtime(serviceStatus: ProductApiPort['serviceStatus']): ProductRuntimePort {
  return {
    dispose() {},
    product: { serviceStatus } as ProductApiPort,
    watch: {} as ProductRuntimePort['watch'],
  };
}

function setVisibility(value: DocumentVisibilityState): void {
  Object.defineProperty(document, 'visibilityState', { configurable: true, value });
  document.dispatchEvent(new Event('visibilitychange'));
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
});

describe('service status polling', () => {
  it('keeps the production 1s/5s, 10s deadline, and 2/4/8/16/30s schedule explicit', () => {
    expect(SERVICE_STATUS_POLLING).toEqual({
      activeMilliseconds: 1_000,
      idleMilliseconds: 5_000,
      requestTimeoutMilliseconds: 10_000,
      failureBackoffMilliseconds: [2_000, 4_000, 8_000, 16_000, 30_000],
    });
  });

  it('never overlaps active reads, pauses while hidden, resumes immediately, and aborts in flight', async () => {
    let resolveFirst!: (value: ServiceStatusV1) => void;
    const first = new Promise<ServiceStatusV1>((resolve) => { resolveFirst = resolve; });
    let secondSignal: AbortSignal | undefined;
    const serviceStatus = vi.fn<ProductApiPort['serviceStatus']>((signal) => {
      if (serviceStatus.mock.calls.length === 1) return first;
      if (serviceStatus.mock.calls.length === 2) {
        secondSignal = signal;
        return new Promise<ServiceStatusV1>((_resolve, reject) => {
          signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')));
        });
      }
      return Promise.resolve(status('READY', 'IDLE'));
    });
    const testRuntime = runtime(serviceStatus);
    const view = renderHook(() => useServiceStatus(testRuntime, TEST_POLLING));
    expect(serviceStatus).toHaveBeenCalledTimes(1);

    await new Promise((resolve) => globalThis.setTimeout(resolve, 45));
    expect(serviceStatus).toHaveBeenCalledTimes(1);

    act(() => resolveFirst(status('INITIALIZING', 'CATALOG_FETCH')));
    await waitFor(() => expect(view.result.current.connection).toBe('CONNECTED'));
    await waitFor(() => expect(serviceStatus).toHaveBeenCalledTimes(2));
    expect(secondSignal?.aborted).toBe(false);

    act(() => setVisibility('hidden'));
    await waitFor(() => expect(secondSignal?.aborted).toBe(true));
    await new Promise((resolve) => globalThis.setTimeout(resolve, 45));
    expect(serviceStatus).toHaveBeenCalledTimes(2);

    act(() => setVisibility('visible'));
    await waitFor(() => expect(serviceStatus).toHaveBeenCalledTimes(3));
    view.unmount();
  });

  it('uses idle polling and exponential transport backoff while retaining the last snapshot', async () => {
    const ready = status('READY', 'IDLE');
    let rejectThird!: (reason: unknown) => void;
    let resolveFourth!: (value: ServiceStatusV1) => void;
    const third = new Promise<ServiceStatusV1>((_resolve, reject) => { rejectThird = reject; });
    const fourth = new Promise<ServiceStatusV1>((resolve) => { resolveFourth = resolve; });
    const serviceStatus = vi.fn<ProductApiPort['serviceStatus']>()
      .mockResolvedValueOnce(ready)
      .mockRejectedValueOnce(new Error('offline'))
      .mockReturnValueOnce(third)
      .mockReturnValueOnce(fourth);
    const testRuntime = runtime(serviceStatus);
    const view = renderHook(() => useServiceStatus(testRuntime, TEST_POLLING));

    await waitFor(() => expect(view.result.current.snapshot).toBe(ready));
    await waitFor(() => expect(view.result.current.connection).toBe('INTERRUPTED'));
    expect(view.result.current.snapshot).toBe(ready);
    await waitFor(() => expect(serviceStatus.mock.calls.length).toBeGreaterThanOrEqual(3));
    act(() => rejectThird(new Error('still offline')));
    await waitFor(() => expect(serviceStatus).toHaveBeenCalledTimes(4));
    act(() => resolveFourth(ready));
    await waitFor(() => expect(view.result.current.connection).toBe('CONNECTED'));
    view.unmount();
  });

  it('turns a hung read into an interrupted retained snapshot without overlapping requests', async () => {
    const ready = status('READY', 'IDLE');
    let hungSignal: AbortSignal | undefined;
    let inFlight = 0;
    let maximumInFlight = 0;
    const serviceStatus = vi.fn<ProductApiPort['serviceStatus']>((signal) => {
      inFlight += 1;
      maximumInFlight = Math.max(maximumInFlight, inFlight);
      if (serviceStatus.mock.calls.length === 1) {
        inFlight -= 1;
        return Promise.resolve(ready);
      }
      if (serviceStatus.mock.calls.length === 2) {
        hungSignal = signal;
        return new Promise<ServiceStatusV1>((_resolve, reject) => {
          signal?.addEventListener('abort', () => {
            inFlight -= 1;
            reject(new DOMException('aborted', 'AbortError'));
          }, { once: true });
        });
      }
      inFlight -= 1;
      return Promise.resolve(ready);
    });
    const polling: ServiceStatusPollingSchedule = {
      activeMilliseconds: 10,
      idleMilliseconds: 10,
      requestTimeoutMilliseconds: 20,
      failureBackoffMilliseconds: [30],
    };
    const testRuntime = runtime(serviceStatus);
    const view = renderHook(() => useServiceStatus(testRuntime, polling));

    await waitFor(() => expect(view.result.current.snapshot).toBe(ready));
    await waitFor(() => expect(view.result.current.connection).toBe('INTERRUPTED'));
    expect(hungSignal?.aborted).toBe(true);
    expect(view.result.current.snapshot).toBe(ready);
    expect(maximumInFlight).toBe(1);
    await waitFor(() => expect(serviceStatus).toHaveBeenCalledTimes(3));
    await waitFor(() => expect(view.result.current.connection).toBe('CONNECTED'));
    view.unmount();
  });
});
