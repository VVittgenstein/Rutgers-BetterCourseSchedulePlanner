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
  type WatchSessionGate,
  type WatchSocketFactory,
  type WatchTimersPort,
} from './WatchClient';

export interface ProductRuntimePort {
  readonly product: ProductApiPort;
  readonly watch: WatchClientPort;
  dispose(): void;
}

export interface ProductRuntimeOptions {
  readonly baseUrl?: string;
  readonly clock?: () => number;
  readonly fetch?: typeof fetch;
  readonly messageId?: WatchMessageIdSource;
  readonly session: ProductSessionSource;
  /**
   * Answered before every watch connection attempt on a target that has to
   * confirm its session ticket first. A target whose ticket is good for the
   * life of the process passes nothing.
   */
  readonly sessionGate?: WatchSessionGate | undefined;
  readonly socket?: WatchSocketFactory;
  readonly timers?: WatchTimersPort;
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
    ...(options.clock === undefined ? {} : { clock: options.clock }),
    ...(options.messageId === undefined ? {} : { messageId: options.messageId }),
    ...(options.sessionGate === undefined ? {} : { sessionGate: options.sessionGate }),
    ...(options.socket === undefined ? {} : { socket: options.socket }),
    ...(options.timers === undefined ? {} : { timers: options.timers }),
  };
  const product = new ProductApi(new ProductClient(productOptions));
  const watch = new WatchClient(watchOptions);
  return {
    product,
    watch,
    dispose() {
      // Disposal is a harder barrier than a Disconnect: a torn-down page must
      // not be reconnected by anything, its own recovery included.
      watch.dispose();
    },
  };
}
