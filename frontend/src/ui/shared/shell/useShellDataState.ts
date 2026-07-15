import { useCallback, useEffect, useState } from 'react';

import type { CatalogDiscoveryResponseV1 } from '../product/contracts/catalog';
import type { FilterSchemaV1 } from '../product/contracts/query';
import type { ProductRuntimePort } from '../product/runtime';

const DISCOVERY_HYDRATION_INTERVAL_MILLISECONDS = 2_000;

export type ShellDiscoveryState =
  | 'CURRENT'
  | 'STALE_LAST_SUCCESS'
  | 'UNAVAILABLE';

interface ShellDataSnapshot {
  readonly discovery: CatalogDiscoveryResponseV1;
  readonly discoveryState: ShellDiscoveryState;
  readonly filterCount: number;
  readonly filterSchema: FilterSchemaV1;
}

export type ShellDataState =
  | { readonly status: 'LOADING' }
  | ({ readonly status: 'READY' } & ShellDataSnapshot)
  | ({ readonly status: 'EMPTY' } & ShellDataSnapshot)
  | {
      readonly status: 'ERROR';
      readonly reason: 'SHELL_DATA_REQUEST_FAILED';
      readonly message: string;
    };

export interface ShellDataResource {
  readonly retry: () => void;
  readonly state: ShellDataState;
}

function classifyDiscovery(discovery: CatalogDiscoveryResponseV1): ShellDiscoveryState {
  if (discovery.status.availability === 'CURRENT') return 'CURRENT';
  if (discovery.status.availability === 'STALE_LAST_SUCCESS') return 'STALE_LAST_SUCCESS';
  return 'UNAVAILABLE';
}

export function useShellDataState(runtime: ProductRuntimePort): ShellDataResource {
  const [requestGeneration, setRequestGeneration] = useState(0);
  const [state, setState] = useState<ShellDataState>({ status: 'LOADING' });

  const retry = useCallback(() => {
    setRequestGeneration((generation) => generation + 1);
  }, []);

  useEffect(() => {
    const abort = new AbortController();
    let active = true;
    setState({ status: 'LOADING' });

    void Promise.all([
      runtime.product.filterSchema(abort.signal),
      runtime.product.catalogDiscovery({ contractVersion: 1 }, abort.signal),
    ]).then(([filterSchema, discovery]) => {
      if (!active) return;
      const snapshot: ShellDataSnapshot = {
        discovery,
        discoveryState: classifyDiscovery(discovery),
        filterCount: filterSchema.fields.length,
        filterSchema,
      };
      setState(
        discovery.targets.length === 0 || snapshot.discoveryState === 'UNAVAILABLE'
          ? { status: 'EMPTY', ...snapshot }
          : { status: 'READY', ...snapshot },
      );
    }).catch((error: unknown) => {
      if (!active || abort.signal.aborted) return;
      setState({
        status: 'ERROR',
        reason: 'SHELL_DATA_REQUEST_FAILED',
        message: error instanceof Error ? error.message : 'Unknown shell data error',
      });
    });

    return () => {
      active = false;
      abort.abort();
    };
  }, [requestGeneration, runtime]);

  useEffect(() => {
    if (
      state.status !== 'READY'
      || state.discoveryState !== 'CURRENT'
      || state.discovery.targets.length === 0
      || state.discovery.subjects.length > 0
    ) {
      return undefined;
    }

    const abort = new AbortController();
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const schedule = () => {
      timer = globalThis.setTimeout(() => {
        timer = undefined;
        void poll();
      }, DISCOVERY_HYDRATION_INTERVAL_MILLISECONDS);
    };
    const poll = async () => {
      try {
        // This product route reads the local operational store. It never starts
        // a Rutgers refresh, so multiple browser sessions cannot fan out to the
        // upstream origin while waiting for the first Catalog publication.
        const discovery = await runtime.product.catalogDiscovery(
          { contractVersion: 1 },
          abort.signal,
        );
        if (!active) return;
        if (
          discovery.status.availability === 'CURRENT'
          && discovery.targets.length > 0
          && discovery.subjects.length > 0
        ) {
          setState((current) => {
            if (current.status !== 'READY' || current.discovery !== state.discovery) {
              return current;
            }
            return {
              ...current,
              discovery,
              discoveryState: classifyDiscovery(discovery),
            };
          });
          return;
        }
      } catch {
        if (!active || abort.signal.aborted) return;
      }
      if (active) schedule();
    };

    schedule();
    return () => {
      active = false;
      if (timer !== undefined) globalThis.clearTimeout(timer);
      abort.abort();
    };
  }, [runtime, state]);

  return { retry, state };
}
