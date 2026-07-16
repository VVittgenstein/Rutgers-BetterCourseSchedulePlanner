import { useCallback, useEffect, useRef, useState } from 'react';

import type { CatalogDiscoveryResponseV1 } from '../product/contracts/catalog';
import type { FilterSchemaV1 } from '../product/contracts/query';
import type { ProductRuntimePort } from '../product/runtime';

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

export function useShellDataState(
  runtime: ProductRuntimePort,
  discoveryRevision = 'initial',
): ShellDataResource {
  const [requestGeneration, setRequestGeneration] = useState(0);
  const [state, setState] = useState<ShellDataState>({ status: 'LOADING' });
  const lastDiscoveryRevision = useRef(discoveryRevision);

  const retry = useCallback(() => {
    setState({ status: 'LOADING' });
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
    if (state.status === 'LOADING' || state.status === 'ERROR') return undefined;
    if (lastDiscoveryRevision.current === discoveryRevision) return undefined;
    lastDiscoveryRevision.current = discoveryRevision;

    const abort = new AbortController();
    let active = true;
    void Promise.resolve().then(() => runtime.product.catalogDiscovery(
      { contractVersion: 1 },
      abort.signal,
    )).then((discovery) => {
      if (!active) return;
      const discoveryState = classifyDiscovery(discovery);
      setState((current) => {
        if (current.status !== 'READY' && current.status !== 'EMPTY') return current;
        const snapshot = {
          discovery,
          discoveryState,
          filterCount: current.filterCount,
          filterSchema: current.filterSchema,
        };
        return discovery.targets.length === 0 || discoveryState === 'UNAVAILABLE'
          ? { status: 'EMPTY', ...snapshot }
          : { status: 'READY', ...snapshot };
      });
    }).catch(() => {
      // Service status owns retry cadence. Keep the last discovery snapshot.
    });
    return () => {
      active = false;
      abort.abort();
    };
  }, [discoveryRevision, runtime, state.status]);

  return { retry, state };
}
