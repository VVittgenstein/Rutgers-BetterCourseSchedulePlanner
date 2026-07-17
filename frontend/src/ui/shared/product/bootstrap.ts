const CANONICAL_V4_UUID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

export type ProductBootstrapFailureReason =
  | 'BOOTSTRAP_UNAVAILABLE'
  | 'BOOTSTRAP_INVALID'
  | 'BOOTSTRAP_REQUEST_FAILED'
  | 'RUNTIME_FAILED';

export interface ProductSessionBootstrap {
  readonly sessionNonce: string;
}

export class ProductBootstrapError extends Error {
  readonly reason: ProductBootstrapFailureReason;

  constructor(reason: ProductBootstrapFailureReason) {
    super(reason);
    this.name = 'ProductBootstrapError';
    this.reason = reason;
  }
}

export function parseProductSessionBootstrap(value: unknown): ProductSessionBootstrap {
  if (
    !isRecord(value)
    || Object.keys(value).length !== 2
    || value.protocolVersion !== 1
    || !Object.hasOwn(value, 'data')
    || !isRecord(value.data)
    || !Object.hasOwn(value.data, 'sessionNonce')
    || typeof value.data.sessionNonce !== 'string'
    || !CANONICAL_V4_UUID.test(value.data.sessionNonce)
  ) {
    throw new ProductBootstrapError('BOOTSTRAP_INVALID');
  }
  return { sessionNonce: value.data.sessionNonce };
}

export function readEmbeddedProductBootstrap(source: Document): unknown {
  const elements = source.querySelectorAll('[id="bcsp-bootstrap"]');
  if (elements.length !== 1) {
    throw new ProductBootstrapError('BOOTSTRAP_UNAVAILABLE');
  }
  const element = elements.item(0);
  if (
    element.tagName !== 'SCRIPT'
    || element.getAttribute('type') !== 'application/json'
    || element.textContent === null
    || element.textContent.trim().length === 0
  ) {
    throw new ProductBootstrapError('BOOTSTRAP_INVALID');
  }
  try {
    return JSON.parse(element.textContent) as unknown;
  } catch {
    throw new ProductBootstrapError('BOOTSTRAP_INVALID');
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
