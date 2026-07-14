import type {
  ApiErrorEnvelope,
  HttpRequestEnvelope,
  HttpSuccessEnvelope,
} from './contracts/common';

export type ProductSessionSource = () => string | null;
export type ProductHttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

export interface ProductClientOptions {
  readonly baseUrl?: string;
  readonly fetch?: typeof fetch;
  readonly session?: ProductSessionSource;
}

export interface ProductRequestOptions<TRequest> {
  readonly method: ProductHttpMethod;
  readonly payload?: TRequest;
  readonly signal?: AbortSignal;
}

export interface ProductClientPort {
  get<TResponse>(path: string, signal?: AbortSignal): Promise<TResponse>;
  post<TRequest, TResponse>(path: string, payload: TRequest, signal?: AbortSignal): Promise<TResponse>;
  request<TRequest, TResponse>(
    path: string,
    options: ProductRequestOptions<TRequest>,
  ): Promise<TResponse>;
}

export class ProductClientError extends Error {
  readonly status: number;
  readonly apiError: ApiErrorEnvelope | null;

  constructor(status: number, apiError: ApiErrorEnvelope | null) {
    super(apiError?.error.messageKey ?? `HTTP_${status}`);
    this.name = 'ProductClientError';
    this.status = status;
    this.apiError = apiError;
  }
}

export class ProductClient implements ProductClientPort {
  readonly #baseUrl: string;
  readonly #fetch: typeof fetch;
  readonly #session: ProductSessionSource;

  constructor(options: ProductClientOptions = {}) {
    this.#baseUrl = (options.baseUrl ?? '').replace(/\/$/u, '');
    this.#fetch = options.fetch ?? globalThis.fetch.bind(globalThis);
    this.#session = options.session ?? (() => null);
  }

  get<TResponse>(path: string, signal?: AbortSignal): Promise<TResponse> {
    return this.request<never, TResponse>(path, signal === undefined
      ? { method: 'GET' }
      : { method: 'GET', signal });
  }

  post<TRequest, TResponse>(
    path: string,
    payload: TRequest,
    signal?: AbortSignal,
  ): Promise<TResponse> {
    return this.request<TRequest, TResponse>(path, signal === undefined
      ? { method: 'POST', payload }
      : { method: 'POST', payload, signal });
  }

  async request<TRequest, TResponse>(
    path: string,
    options: ProductRequestOptions<TRequest>,
  ): Promise<TResponse> {
    assertApiPath(path);
    const headers = new Headers({ accept: 'application/json' });
    const changesState = options.method !== 'GET';
    if (changesState) {
      headers.set('content-type', 'application/json');
      const session = this.#session();
      if (session !== null) headers.set('x-bcsp-session', session);
    }
    const body = options.payload === undefined
      ? undefined
      : JSON.stringify({ protocolVersion: 1, payload: options.payload } satisfies HttpRequestEnvelope<TRequest>);
    const response = await this.#fetch(`${this.#baseUrl}${path}`, {
      method: options.method,
      headers,
      credentials: 'same-origin',
      ...(body === undefined ? {} : { body }),
      ...(options.signal === undefined ? {} : { signal: options.signal }),
    });
    const decoded = await decodeJson(response);
    if (!response.ok) {
      throw new ProductClientError(response.status, isApiErrorEnvelope(decoded) ? decoded : null);
    }
    if (!isSuccessEnvelope<TResponse>(decoded)) {
      throw new ProductClientError(response.status, null);
    }
    return decoded.data;
  }
}

function assertApiPath(path: string): void {
  if (!path.startsWith('/api/') || path.startsWith('//') || path.includes('#')) {
    throw new TypeError('ProductClient paths must be same-host /api/ paths');
  }
}

async function decodeJson(response: Response): Promise<unknown> {
  try {
    return await response.json() as unknown;
  } catch {
    throw new ProductClientError(response.status, null);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isSuccessEnvelope<T>(value: unknown): value is HttpSuccessEnvelope<T> {
  return isRecord(value) && value.protocolVersion === 1 && Object.hasOwn(value, 'data');
}

function isApiErrorEnvelope(value: unknown): value is ApiErrorEnvelope {
  return isRecord(value)
    && value.protocolVersion === 1
    && isRecord(value.error)
    && typeof value.error.code === 'string'
    && typeof value.error.messageKey === 'string'
    && typeof value.error.traceId === 'string'
    && Array.isArray(value.error.details);
}
