import {
  ProductClient,
  type ProductClientOptions,
  type ProductSessionSource,
} from './ProductClient';
import { ProductApi, type ProductApiPort } from './ProductApi';
import {
  WatchClient,
  type WatchClientOptions,
  type WatchClientPort,
  type WatchMessageIdSource,
  type WatchSocketFactory,
} from './WatchClient';

export interface ProductRuntimePort {
  readonly product: ProductApiPort;
  readonly watch: WatchClientPort;
  dispose(): void;
}

export interface ProductRuntimeOptions {
  readonly baseUrl?: string;
  readonly fetch?: typeof fetch;
  readonly messageId?: WatchMessageIdSource;
  readonly session: ProductSessionSource;
  readonly socket?: WatchSocketFactory;
}

export function createProductRuntimePort(options: ProductRuntimeOptions): ProductRuntimePort {
  const shared = {
    ...(options.baseUrl === undefined ? {} : { baseUrl: options.baseUrl }),
    session: options.session,
  };
  const productOptions: ProductClientOptions = {
    ...shared,
    ...(options.fetch === undefined ? {} : { fetch: options.fetch }),
  };
  const watchOptions: WatchClientOptions = {
    ...shared,
    ...(options.messageId === undefined ? {} : { messageId: options.messageId }),
    ...(options.socket === undefined ? {} : { socket: options.socket }),
  };
  const product = new ProductApi(new ProductClient(productOptions));
  const watch = new WatchClient(watchOptions);
  return {
    product,
    watch,
    dispose() {
      watch.disconnect();
    },
  };
}
