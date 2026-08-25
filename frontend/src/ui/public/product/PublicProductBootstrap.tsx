import { useEffect, useState, type ReactNode } from 'react';

import {
  createProductRuntimePort,
  parseProductSessionBootstrap,
  ProductBootstrapError,
  PRODUCT_RUNTIME_LOADING,
  ProductRuntimeProvider,
  readEmbeddedProductBootstrap,
  type ProductRuntimeOptions,
  type ProductRuntimePort,
  type ProductRuntimeState,
  type WatchMessageIdSource,
  type WatchSessionGate,
  type WatchSocketFactory,
} from '../../shared/product';
import { createPublicSessionGate, createPublicSessionTicket } from './sessionTicket';

export type PublicProductRuntimeFactory = (options: ProductRuntimeOptions) => ProductRuntimePort;

export interface PublicProductBootstrapProps {
  readonly baseUrl?: string;
  readonly bootstrap?: unknown;
  readonly children: ReactNode;
  readonly fetch?: typeof fetch;
  readonly messageId?: WatchMessageIdSource;
  readonly runtimeFactory?: PublicProductRuntimeFactory;
  /** Overridden only by tests that drive the ticket exchange themselves. */
  readonly sessionGate?: WatchSessionGate;
  readonly socket?: WatchSocketFactory;
  readonly sourceDocument?: Document;
}

export type PublicProductBootstrapConfiguration = Omit<PublicProductBootstrapProps, 'children'>;

export function PublicProductBootstrap({
  baseUrl,
  bootstrap: injectedBootstrap,
  children,
  fetch: fetchImplementation,
  messageId,
  runtimeFactory = createProductRuntimePort,
  sessionGate,
  socket,
  sourceDocument,
}: PublicProductBootstrapProps) {
  const [state, setState] = useState<ProductRuntimeState>(PRODUCT_RUNTIME_LOADING);

  useEffect(() => {
    let runtime: ProductRuntimePort | null = null;
    setState(PRODUCT_RUNTIME_LOADING);
    try {
      const rawBootstrap = injectedBootstrap === undefined
        ? readEmbeddedProductBootstrap(sourceDocument ?? document)
        : injectedBootstrap;
      const bootstrap = parseProductSessionBootstrap(rawBootstrap);
      // The ticket becomes MUTABLE here, and this is the only place the page
      // ever holds it. A public session can be replaced by the server -- it
      // expires, or the server restarts and forgets every session it had --
      // and a page whose ticket was a bootstrap constant could only sit there
      // failing to connect for a reason the browser will not tell it.
      const ticket = createPublicSessionTicket(bootstrap.sessionNonce);
      runtime = runtimeFactory({
        ...(baseUrl === undefined ? {} : { baseUrl }),
        ...(fetchImplementation === undefined ? {} : { fetch: fetchImplementation }),
        ...(messageId === undefined ? {} : { messageId }),
        session: () => ticket.read(),
        sessionGate: sessionGate ?? createPublicSessionGate({
          ticket,
          ...(baseUrl === undefined ? {} : { baseUrl }),
          ...(fetchImplementation === undefined ? {} : { fetch: fetchImplementation }),
          locale: () => documentLocale(sourceDocument ?? document),
        }),
        ...(socket === undefined ? {} : { socket }),
      });
      setState({ status: 'READY', runtime });
    } catch (error) {
      const reason = error instanceof ProductBootstrapError
        ? error.reason
        : 'RUNTIME_FAILED';
      setState({ status: 'ERROR', reason });
    }

    return () => runtime?.dispose();
  }, [baseUrl, fetchImplementation, injectedBootstrap, messageId, runtimeFactory, sessionGate, socket, sourceDocument]);

  return <ProductRuntimeProvider state={state}>{children}</ProductRuntimeProvider>;
}

/**
 * The language a renewed ticket should be issued in.
 *
 * Read from the document the page was served as, which is the same source the
 * server used when it issued the first one. A null answer simply omits the
 * field and lets the server negotiate from Accept-Language, exactly as it
 * does for the index page.
 */
function documentLocale(source: Document): string | null {
  const tag = source.documentElement.getAttribute('lang');
  return tag === null || tag.length === 0 ? null : tag;
}
