import { useEffect, useState, type ReactNode } from 'react';

import {
  createProductRuntimePort,
  parseProductSessionBootstrap,
  ProductBootstrapError,
  PRODUCT_RUNTIME_LOADING,
  ProductRuntimeProvider,
  type ProductRuntimeOptions,
  type ProductRuntimePort,
  type ProductRuntimeState,
  type WatchMessageIdSource,
  type WatchSocketFactory,
} from '../../shared/product';
import {
  createLocalPersonalApi,
  LocalPersonalProvider,
  parseLocalBootstrapEnvelope,
  type LocalBootstrapData,
  type LocalPersonalApiPort,
} from '../personal';

const BOOTSTRAP_PATH = '/api/v1/local/bootstrap';

export type LocalProductRuntimeFactory = (options: ProductRuntimeOptions) => ProductRuntimePort;

export interface LocalProductBootstrapProps {
  readonly baseUrl?: string;
  readonly children: ReactNode;
  readonly fetch?: typeof fetch;
  readonly messageId?: WatchMessageIdSource;
  readonly runtimeFactory?: LocalProductRuntimeFactory;
  readonly socket?: WatchSocketFactory;
}

export type LocalProductBootstrapConfiguration = Omit<LocalProductBootstrapProps, 'children'>;

export function LocalProductBootstrap({
  baseUrl,
  children,
  fetch: fetchImplementation,
  messageId,
  runtimeFactory = createProductRuntimePort,
  socket,
}: LocalProductBootstrapProps) {
  const [state, setState] = useState<ProductRuntimeState>(PRODUCT_RUNTIME_LOADING);
  const [personal, setPersonal] = useState<{
    readonly api: LocalPersonalApiPort;
    readonly bootstrap: LocalBootstrapData;
  } | null>(null);

  useEffect(() => {
    const abort = new AbortController();
    let active = true;
    let runtime: ProductRuntimePort | null = null;
    setState(PRODUCT_RUNTIME_LOADING);
    setPersonal(null);

    void (async () => {
      try {
        const request = fetchImplementation ?? globalThis.fetch.bind(globalThis);
        const normalizedBaseUrl = (baseUrl ?? '').replace(/\/$/u, '');
        const response = await request(`${normalizedBaseUrl}${BOOTSTRAP_PATH}`, {
          cache: 'no-store',
          credentials: 'same-origin',
          headers: { accept: 'application/json' },
          method: 'GET',
          signal: abort.signal,
        });
        if (!response.ok) throw new ProductBootstrapError('BOOTSTRAP_REQUEST_FAILED');
        let rawBootstrap: unknown;
        try {
          rawBootstrap = await response.json() as unknown;
        } catch {
          throw new ProductBootstrapError('BOOTSTRAP_INVALID');
        }
        const sessionBootstrap = parseProductSessionBootstrap(rawBootstrap);
        const localBootstrap = parseLocalBootstrapEnvelope(rawBootstrap);
        if (!active) return;
        try {
          runtime = runtimeFactory({
            ...(baseUrl === undefined ? {} : { baseUrl }),
            fetch: request,
            ...(messageId === undefined ? {} : { messageId }),
            session: () => sessionBootstrap.sessionNonce,
            ...(socket === undefined ? {} : { socket }),
          });
        } catch {
          throw new ProductBootstrapError('RUNTIME_FAILED');
        }
        const api = createLocalPersonalApi({
          ...(baseUrl === undefined ? {} : { baseUrl }),
          fetch: request,
          session: () => sessionBootstrap.sessionNonce,
        });
        setPersonal({ api, bootstrap: localBootstrap });
        setState({ status: 'READY', runtime });
      } catch (error) {
        if (!active) return;
        const reason = error instanceof ProductBootstrapError
          ? error.reason
          : 'BOOTSTRAP_REQUEST_FAILED';
        setState({ status: 'ERROR', reason });
      }
    })();

    return () => {
      active = false;
      abort.abort();
      runtime?.dispose();
    };
  }, [baseUrl, fetchImplementation, messageId, runtimeFactory, socket]);

  return (
    <ProductRuntimeProvider state={state}>
      {personal === null
        ? children
        : (
          <LocalPersonalProvider api={personal.api} initialBootstrap={personal.bootstrap}>
            {children}
          </LocalPersonalProvider>
        )}
    </ProductRuntimeProvider>
  );
}
