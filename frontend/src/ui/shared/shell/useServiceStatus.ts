import { useCallback, useEffect, useMemo, useState } from 'react';

import type { ProductRuntimePort, ServiceStatusV1 } from '../product';

export interface ServiceStatusPollingSchedule {
  readonly activeMilliseconds: number;
  readonly idleMilliseconds: number;
  readonly requestTimeoutMilliseconds: number;
  readonly failureBackoffMilliseconds: readonly number[];
}

export const SERVICE_STATUS_POLLING: ServiceStatusPollingSchedule = {
  activeMilliseconds: 1_000,
  idleMilliseconds: 5_000,
  requestTimeoutMilliseconds: 10_000,
  failureBackoffMilliseconds: [2_000, 4_000, 8_000, 16_000, 30_000],
};

export type ServiceConnectionState = 'CONNECTING' | 'CONNECTED' | 'INTERRUPTED';

export interface ServiceStatusResource {
  readonly connection: ServiceConnectionState;
  readonly retry: () => void;
  readonly revision: string;
  readonly snapshot: ServiceStatusV1 | null;
}

function isActive(snapshot: ServiceStatusV1): boolean {
  return snapshot.level === 'INITIALIZING'
    || snapshot.level === 'PARTIALLY_READY'
    || (snapshot.operation.phase !== 'IDLE' && snapshot.operation.phase !== 'STOPPED');
}

function pageIsHidden(): boolean {
  return globalThis.document?.visibilityState === 'hidden';
}

function discoveryRevision(snapshot: ServiceStatusV1 | null): string {
  if (snapshot === null) return 'unobserved';
  return JSON.stringify({
    availability: snapshot.discovery.availability,
    contentVersion: snapshot.discovery.lastSuccess?.contentVersion ?? null,
    targets: snapshot.targets.map(({ target, catalogContentVersion, searchAvailable }) => ({
      target,
      catalogContentVersion,
      searchAvailable,
    })),
  });
}

export function useServiceStatus(
  runtime: ProductRuntimePort,
  polling: ServiceStatusPollingSchedule = SERVICE_STATUS_POLLING,
): ServiceStatusResource {
  const [generation, setGeneration] = useState(0);
  const [connection, setConnection] = useState<ServiceConnectionState>('CONNECTING');
  const [snapshot, setSnapshot] = useState<ServiceStatusV1 | null>(null);

  const retry = useCallback(() => {
    setConnection('CONNECTING');
    setGeneration((value) => value + 1);
  }, []);

  useEffect(() => {
    let active = true;
    let controller: AbortController | null = null;
    let failureIndex = 0;
    let inFlight = false;
    let resumePending = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let requestTimer: ReturnType<typeof setTimeout> | null = null;
    let deadlineExpired = false;

    const clearTimer = () => {
      if (timer !== null) globalThis.clearTimeout(timer);
      timer = null;
    };
    const schedule = (delay: number, poll: () => Promise<void>) => {
      clearTimer();
      if (!active || pageIsHidden()) return;
      timer = globalThis.setTimeout(() => {
        timer = null;
        void poll();
      }, delay);
    };
    const clearRequestTimer = () => {
      if (requestTimer !== null) globalThis.clearTimeout(requestTimer);
      requestTimer = null;
    };
    const poll = async () => {
      if (!active || pageIsHidden()) return;
      if (inFlight) {
        resumePending = true;
        return;
      }
      inFlight = true;
      controller = new AbortController();
      deadlineExpired = false;
      try {
        const request = runtime.product.serviceStatus(controller.signal);
        const deadline = new Promise<never>((_resolve, reject) => {
          requestTimer = globalThis.setTimeout(() => {
            deadlineExpired = true;
            controller?.abort();
            reject(new Error('service status request deadline exceeded'));
          }, polling.requestTimeoutMilliseconds);
          controller?.signal.addEventListener('abort', () => {
            reject(new DOMException('aborted', 'AbortError'));
          }, { once: true });
        });
        const next = await Promise.race([request, deadline]);
        if (!active) return;
        failureIndex = 0;
        setSnapshot(next);
        setConnection('CONNECTED');
        schedule(isActive(next) ? polling.activeMilliseconds : polling.idleMilliseconds, poll);
      } catch {
        if (!active || (controller.signal.aborted && !deadlineExpired)) return;
        setConnection('INTERRUPTED');
        const delay = polling.failureBackoffMilliseconds[
          Math.min(failureIndex, polling.failureBackoffMilliseconds.length - 1)
        ] ?? 30_000;
        failureIndex += 1;
        schedule(delay, poll);
      } finally {
        clearRequestTimer();
        inFlight = false;
        controller = null;
        if (resumePending && active && !pageIsHidden()) {
          resumePending = false;
          schedule(0, poll);
        }
      }
    };
    const onVisibilityChange = () => {
      if (pageIsHidden()) {
        clearTimer();
        controller?.abort();
        return;
      }
      failureIndex = 0;
      if (inFlight) resumePending = true;
      else void poll();
    };
    const onOnline = () => {
      failureIndex = 0;
      if (inFlight) resumePending = true;
      else void poll();
    };

    setConnection('CONNECTING');
    void poll();
    globalThis.document?.addEventListener('visibilitychange', onVisibilityChange);
    globalThis.addEventListener?.('online', onOnline);
    return () => {
      active = false;
      clearTimer();
      clearRequestTimer();
      controller?.abort();
      globalThis.document?.removeEventListener('visibilitychange', onVisibilityChange);
      globalThis.removeEventListener?.('online', onOnline);
    };
  }, [generation, polling, runtime]);

  return {
    connection,
    retry,
    revision: useMemo(() => discoveryRevision(snapshot), [snapshot]),
    snapshot,
  };
}
