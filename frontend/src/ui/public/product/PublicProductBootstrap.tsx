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
  type WatchSocketFactory,
} from '../../shared/product';

export type PublicProductRuntimeFactory = (options: ProductRuntimeOptions) => ProductRuntimePort;

export interface PublicProductBootstrapProps {
  readonly baseUrl?: string;
  readonly bootstrap?: unknown;
  readonly children: ReactNode;
  readonly fetch?: typeof fetch;
  readonly messageId?: WatchMessageIdSource;
  readonly runtimeFactory?: PublicProductRuntimeFactory;
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
      runtime = runtimeFactory({
        ...(baseUrl === undefined ? {} : { baseUrl }),
        ...(fetchImplementation === undefined ? {} : { fetch: fetchImplementation }),
        ...(messageId === undefined ? {} : { messageId }),
        session: () => bootstrap.sessionNonce,
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
  }, [baseUrl, fetchImplementation, injectedBootstrap, messageId, runtimeFactory, socket, sourceDocument]);

  return <ProductRuntimeProvider state={state}>{children}</ProductRuntimeProvider>;
}
