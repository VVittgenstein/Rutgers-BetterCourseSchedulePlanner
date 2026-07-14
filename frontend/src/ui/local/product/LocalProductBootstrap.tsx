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

  useEffect(() => {
    const abort = new AbortController();
    let active = true;
    let runtime: ProductRuntimePort | null = null;
    setState(PRODUCT_RUNTIME_LOADING);

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
        const bootstrap = parseProductSessionBootstrap(rawBootstrap);
        if (!active) return;
        try {
          runtime = runtimeFactory({
            ...(baseUrl === undefined ? {} : { baseUrl }),
            fetch: request,
            ...(messageId === undefined ? {} : { messageId }),
            session: () => bootstrap.sessionNonce,
            ...(socket === undefined ? {} : { socket }),
          });
        } catch {
          throw new ProductBootstrapError('RUNTIME_FAILED');
        }
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

  return <ProductRuntimeProvider state={state}>{children}</ProductRuntimeProvider>;
}
