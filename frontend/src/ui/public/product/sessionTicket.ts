import type { WatchSessionDecision, WatchSessionGate } from '../../shared/product';

/**
 * The public target's session ticket, held in memory and nowhere else.
 *
 * A ticket the server can replace at any time cannot be a bootstrap constant:
 * every request made after a renewal has to carry the NEW one, or the server
 * answers a page that thinks it is connected with a credential it has already
 * thrown away.
 *
 * It is never written to storage, a cookie, the DOM, or a log. The only place
 * it appears outside this module is the `session` header the shared client
 * already sends and the WebSocket query the transport already builds.
 */
export interface PublicSessionTicket {
  /** The ticket to use right now. */
  read(): string;
  /** Replaces it. Called only with a server-issued replacement. */
  replace(next: string): void;
}

export function createPublicSessionTicket(initial: string): PublicSessionTicket {
  let current = initial;
  return {
    read: () => current,
    replace(next: string) {
      current = next;
    },
  };
}

export const PUBLIC_SESSION_VALIDATE_PATH = '/api/v1/session/validate';

const CANONICAL_V4_UUID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

export interface PublicSessionGateOptions {
  readonly ticket: PublicSessionTicket;
  readonly baseUrl?: string | undefined;
  readonly fetch?: typeof fetch | undefined;
  /** Sent so a renewed ticket is issued in the language the page is in. */
  readonly locale?: (() => string | null) | undefined;
}

/**
 * What the server answered about a ticket.
 *
 * Only two of these mean the page may connect, and both name the ticket THAT
 * attempt must use. Everything else is "not now" -- the browser cannot see a
 * WebSocket handshake's status code, so guessing why a connection failed is
 * not available to us and is not attempted.
 */
type ValidateAnswer =
  | { readonly kind: 'VALID' }
  | { readonly kind: 'RENEWED'; readonly nonce: string }
  | { readonly kind: 'UNAVAILABLE'; readonly retryAfterSeconds: number | null };

function retryAfterFrom(response: Response, body: unknown): number | null {
  const header = response.headers.get('retry-after');
  const fromHeader = header === null ? Number.NaN : Number.parseInt(header, 10);
  if (Number.isFinite(fromHeader) && fromHeader >= 0) return fromHeader;
  if (
    typeof body === 'object'
    && body !== null
    && 'error' in body
    && typeof body.error === 'object'
    && body.error !== null
    && 'details' in body.error
    && Array.isArray(body.error.details)
  ) {
    for (const detail of body.error.details) {
      if (
        typeof detail === 'object'
        && detail !== null
        && 'kind' in detail
        && detail.kind === 'RETRY_AFTER_SECONDS'
        && 'seconds' in detail
        && typeof detail.seconds === 'number'
        && Number.isFinite(detail.seconds)
      ) {
        return detail.seconds;
      }
    }
  }
  return null;
}

/**
 * Reads the frozen response shape, strictly.
 *
 * A body that is not exactly `{"valid":true}` or `{"renewed":"<canonical>"}`
 * is not an answer this page may act on. Accepting a loose shape here is how
 * a page ends up connecting with a "nonce" that is an empty string.
 */
function readAnswer(data: unknown): ValidateAnswer | null {
  if (typeof data !== 'object' || data === null || Array.isArray(data)) return null;
  const keys = Object.keys(data);
  if (keys.length !== 1) return null;
  if (keys[0] === 'valid') {
    return (data as { valid: unknown }).valid === true ? { kind: 'VALID' } : null;
  }
  if (keys[0] !== 'renewed') return null;
  const renewed = (data as { renewed: unknown }).renewed;
  if (typeof renewed !== 'string' || !CANONICAL_V4_UUID.test(renewed)) return null;
  return { kind: 'RENEWED', nonce: renewed };
}

/**
 * Confirms the ticket with the server before a connection attempt.
 *
 * Single-flight over its own in-flight request as well: the transport only
 * ever has one attempt outstanding, but a second caller asking the same
 * question at the same moment would be two anonymous issuance requests
 * against a per-IP budget the index page shares.
 */
export function createPublicSessionGate(options: PublicSessionGateOptions): WatchSessionGate {
  const request = options.fetch ?? globalThis.fetch.bind(globalThis);
  const baseUrl = (options.baseUrl ?? '').replace(/\/$/u, '');
  let inFlight: Promise<ValidateAnswer> | null = null;

  const ask = async (): Promise<ValidateAnswer> => {
    const locale = options.locale?.() ?? null;
    let response: Response;
    try {
      response = await request(`${baseUrl}${PUBLIC_SESSION_VALIDATE_PATH}`, {
        method: 'POST',
        headers: { accept: 'application/json', 'content-type': 'application/json' },
        credentials: 'same-origin',
        // The frozen request contract is a BARE body: `{nonce, locale?}` with
        // unknown fields refused. The shared client's envelope would be a
        // 400 every time.
        body: JSON.stringify(locale === null
          ? { nonce: options.ticket.read() }
          : { nonce: options.ticket.read(), locale }),
      });
    } catch {
      return { kind: 'UNAVAILABLE', retryAfterSeconds: null };
    }
    let body: unknown = null;
    try {
      body = await response.json();
    } catch {
      body = null;
    }
    if (!response.ok) {
      // 429 is the only status that carries a wait. Everything else -- a
      // rejected body, a wrong Origin, a wrong Host, an exhausted registry --
      // is just "not now", and the approved backoff decides when to ask
      // again.
      return {
        kind: 'UNAVAILABLE',
        retryAfterSeconds: response.status === 429 ? retryAfterFrom(response, body) : null,
      };
    }
    const envelope = body as { protocolVersion?: unknown; data?: unknown } | null;
    if (envelope === null || envelope.protocolVersion !== 1) {
      return { kind: 'UNAVAILABLE', retryAfterSeconds: null };
    }
    return readAnswer(envelope.data) ?? { kind: 'UNAVAILABLE', retryAfterSeconds: null };
  };

  return async (): Promise<WatchSessionDecision> => {
    inFlight ??= ask().finally(() => { inFlight = null; });
    const answer = await inFlight;
    if (answer.kind === 'VALID') return { kind: 'SESSION', session: options.ticket.read() };
    if (answer.kind === 'RENEWED') {
      // Saved before the attempt uses it, so the HTTP client and the socket
      // are never a renewal apart: a page holding two tickets would be
      // authenticating two different sessions.
      options.ticket.replace(answer.nonce);
      return { kind: 'SESSION', session: answer.nonce };
    }
    return { kind: 'UNAVAILABLE', retryAfterSeconds: answer.retryAfterSeconds };
  };
}
