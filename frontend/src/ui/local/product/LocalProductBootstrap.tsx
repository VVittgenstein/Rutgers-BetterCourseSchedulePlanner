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
import type { WatchIntentPort } from '../../shared/watch';
import { createLocalDesiredWatchApi } from '../desired';
import {
  createLocalPresenceClient,
  type LocalPresenceClient,
  type LocalPresenceSocketFactory,
} from '../presence';
import {
  createLocalPersonalApi,
  LocalPersonalProvider,
  parseLocalBootstrapEnvelope,
  type LocalBootstrapData,
  type LocalPersonalApiPort,
} from '../personal';
import { LocalDesiredWatchProvider } from './LocalDesiredWatchContext';

const BOOTSTRAP_PATH = '/api/v1/local/bootstrap';

export type LocalProductRuntimeFactory = (options: ProductRuntimeOptions) => ProductRuntimePort;

export interface LocalProductBootstrapProps {
  readonly baseUrl?: string;
  readonly children: ReactNode;
  readonly fetch?: typeof fetch;
  readonly messageId?: WatchMessageIdSource;
  /** Deterministic mutation ids for tests; production mints UUIDv4s. */
  readonly mutationId?: () => string;
  readonly runtimeFactory?: LocalProductRuntimeFactory;
  readonly socket?: WatchSocketFactory;
  /** The presence transport. Injected by tests; production opens a real one. */
  readonly presenceSocket?: LocalPresenceSocketFactory;
  /** Set false where a test mounts the tree without a runtime to talk to. */
  readonly presence?: boolean;
}

export type LocalProductBootstrapConfiguration = Omit<LocalProductBootstrapProps, 'children'>;

export function LocalProductBootstrap({
  baseUrl,
  children,
  fetch: fetchImplementation,
  messageId,
  mutationId,
  presence = true,
  presenceSocket,
  runtimeFactory = createProductRuntimePort,
  socket,
}: LocalProductBootstrapProps) {
  const [state, setState] = useState<ProductRuntimeState>(PRODUCT_RUNTIME_LOADING);
  const [personal, setPersonal] = useState<{
    readonly api: LocalPersonalApiPort;
    readonly bootstrap: LocalBootstrapData;
    readonly watchIntent: WatchIntentPort;
  } | null>(null);

  useEffect(() => {
    const abort = new AbortController();
    let active = true;
    let runtime: ProductRuntimePort | null = null;
    let presenceClient: LocalPresenceClient | null = null;
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
        const watchIntent = createLocalDesiredWatchApi({
          ...(baseUrl === undefined ? {} : { baseUrl }),
          fetch: request,
          ...(mutationId === undefined ? {} : { mutationId }),
          session: () => sessionBootstrap.sessionNonce,
        });
        if (presence) {
          // Started once the page has a session, and kept alive for as long as
          // this tab is mounted. It is the ONLY thing that tells the runtime
          // somebody is here, and it is deliberately not tied to whether the
          // user has started watching anything.
          presenceClient = createLocalPresenceClient({
            baseUrl: baseUrl ?? globalThis.location?.href ?? 'http://127.0.0.1/',
            session: () => sessionBootstrap.sessionNonce,
            ...(presenceSocket === undefined ? {} : { socket: presenceSocket }),
          });
          presenceClient.start();
        }
        setPersonal({ api, bootstrap: localBootstrap, watchIntent });
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
      presenceClient?.dispose();
      runtime?.dispose();
    };
  }, [
    baseUrl,
    fetchImplementation,
    messageId,
    mutationId,
    presence,
    presenceSocket,
    runtimeFactory,
    socket,
  ]);

  return (
    <ProductRuntimeProvider state={state}>
      {personal === null
        ? children
        : (
          <LocalDesiredWatchProvider watchIntent={personal.watchIntent}>
            <LocalPersonalProvider api={personal.api} initialBootstrap={personal.bootstrap}>
              {children}
            </LocalPersonalProvider>
          </LocalDesiredWatchProvider>
        )}
    </ProductRuntimeProvider>
  );
}
