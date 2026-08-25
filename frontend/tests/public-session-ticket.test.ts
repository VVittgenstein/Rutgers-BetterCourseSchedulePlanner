import { describe, expect, it, vi } from 'vitest';

import {
  createPublicSessionGate,
  createPublicSessionTicket,
  PUBLIC_SESSION_VALIDATE_PATH,
} from '../src/ui/public/product/sessionTicket';

const FIRST = '00000000-0000-4000-8000-000000000001';
const RENEWED = '00000000-0000-4000-8000-000000000002';

function envelope(data: unknown, init?: ResponseInit): Response {
  return new Response(JSON.stringify({ protocolVersion: 1, data }), {
    status: 200,
    headers: { 'content-type': 'application/json' },
    ...init,
  });
}

function rateLimited(retryAfterSeconds: number | null): Response {
  const body = JSON.stringify({
    protocolVersion: 1,
    error: {
      code: 'RATE_LIMITED',
      messageKey: 'error.rate_limited',
      traceId: FIRST,
      details: retryAfterSeconds === null
        ? []
        : [{ kind: 'RETRY_AFTER_SECONDS', seconds: retryAfterSeconds }],
    },
  });
  return new Response(body, {
    status: 429,
    headers: retryAfterSeconds === null
      ? { 'content-type': 'application/json' }
      : { 'content-type': 'application/json', 'retry-after': String(retryAfterSeconds) },
  });
}

function gateOver(responses: readonly (() => Response | Promise<Response>)[]) {
  const ticket = createPublicSessionTicket(FIRST);
  const calls: Array<{ url: string; body: unknown; method: string | undefined }> = [];
  let index = 0;
  const fetchImplementation = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
    calls.push({
      url: String(url),
      method: init?.method,
      body: typeof init?.body === 'string' ? JSON.parse(init.body) : null,
    });
    const next = responses[Math.min(index, responses.length - 1)];
    index += 1;
    if (next === undefined) throw new Error('no response prepared');
    return await next();
  }) as unknown as typeof fetch;
  const gate = createPublicSessionGate({
    ticket,
    baseUrl: 'https://planner.invalid',
    fetch: fetchImplementation,
    locale: () => 'zh-CN',
  });
  return { calls, gate, ticket };
}

describe('the public session ticket', () => {
  it('confirms the ticket it holds with a bare body the server accepts', async () => {
    const { calls, gate, ticket } = gateOver([() => envelope({ valid: true })]);

    expect(await gate()).toEqual({ kind: 'SESSION', session: FIRST });
    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe(`https://planner.invalid${PUBLIC_SESSION_VALIDATE_PATH}`);
    expect(calls[0]?.method).toBe('POST');
    // The frozen request contract refuses unknown fields, so the envelope the
    // rest of this client wraps everything in would be a 400 every time.
    expect(calls[0]?.body).toEqual({ nonce: FIRST, locale: 'zh-CN' });
    expect(ticket.read()).toBe(FIRST);
  });

  it('adopts a renewed ticket before the attempt that will use it', async () => {
    const { gate, ticket } = gateOver([() => envelope({ renewed: RENEWED })]);

    expect(await gate()).toEqual({ kind: 'SESSION', session: RENEWED });
    // Saved, not merely returned: the HTTP client reads the same holder, and
    // a page holding two tickets would be authenticating two sessions.
    expect(ticket.read()).toBe(RENEWED);
  });

  it('carries a stated wait, and only from a rate limit', async () => {
    const limited = gateOver([() => rateLimited(45)]);
    expect(await limited.gate()).toEqual({ kind: 'UNAVAILABLE', retryAfterSeconds: 45 });

    // The header is the primary source; the typed detail is the fallback for a
    // proxy that strips it.
    const detailOnly = gateOver([() => new Response(JSON.stringify({
      protocolVersion: 1,
      error: {
        code: 'RATE_LIMITED',
        messageKey: 'error.rate_limited',
        traceId: FIRST,
        details: [{ kind: 'RETRY_AFTER_SECONDS', seconds: 12 }],
      },
    }), { status: 429, headers: { 'content-type': 'application/json' } })]);
    expect(await detailOnly.gate()).toEqual({ kind: 'UNAVAILABLE', retryAfterSeconds: 12 });

    // Every other refusal is just "not now". The approved backoff decides
    // when to ask again; a wait invented here would be this page guessing.
    for (const status of [400, 403, 421, 503]) {
      const refused = gateOver([() => new Response('{}', { status })]);
      expect(await refused.gate()).toEqual({ kind: 'UNAVAILABLE', retryAfterSeconds: null });
    }
  });

  it('refuses to connect on an answer that is not one of the two frozen shapes', async () => {
    const shapes: readonly unknown[] = [
      { valid: false },
      { valid: true, renewed: RENEWED },
      { renewed: 'not-a-uuid' },
      { renewed: '' },
      {},
      { valid: null },
      'valid',
    ];
    for (const data of shapes) {
      const { gate, ticket } = gateOver([() => envelope(data)]);
      expect(await gate()).toEqual({ kind: 'UNAVAILABLE', retryAfterSeconds: null });
      expect(ticket.read()).toBe(FIRST);
    }

    // An un-enveloped or wrong-version body is not an answer either.
    const bare = gateOver([() => new Response(JSON.stringify({ valid: true }), { status: 200 })]);
    expect(await bare.gate()).toEqual({ kind: 'UNAVAILABLE', retryAfterSeconds: null });
  });

  it('treats a network failure as not-now rather than as an answer', async () => {
    const { gate, ticket } = gateOver([() => Promise.reject(new TypeError('offline'))]);
    expect(await gate()).toEqual({ kind: 'UNAVAILABLE', retryAfterSeconds: null });
    expect(ticket.read()).toBe(FIRST);
  });

  it('asks once when two callers ask at the same moment', async () => {
    let release: ((response: Response) => void) | null = null;
    const { calls, gate } = gateOver([
      () => new Promise<Response>((resolve) => { release = resolve; }),
    ]);

    const first = gate();
    const second = gate();
    release?.(envelope({ renewed: RENEWED }));
    const answers = await Promise.all([first, second]);

    // Renewal is anonymous issuance drawn from the same per-client budget as
    // the index page. Two questions for one connection attempt would spend it
    // twice as fast for no extra information.
    expect(calls).toHaveLength(1);
    expect(answers).toEqual([
      { kind: 'SESSION', session: RENEWED },
      { kind: 'SESSION', session: RENEWED },
    ]);
  });

  it('asks again for the next attempt rather than reusing the last answer', async () => {
    const { calls, gate } = gateOver([
      () => envelope({ valid: true }),
      () => envelope({ renewed: RENEWED }),
    ]);

    expect(await gate()).toEqual({ kind: 'SESSION', session: FIRST });
    expect(await gate()).toEqual({ kind: 'SESSION', session: RENEWED });
    expect(calls).toHaveLength(2);
    expect(calls[1]?.body).toEqual({ nonce: FIRST, locale: 'zh-CN' });
  });

  it('omits the locale entirely when the page has none to state', async () => {
    const ticket = createPublicSessionTicket(FIRST);
    const calls: unknown[] = [];
    const gate = createPublicSessionGate({
      ticket,
      fetch: (async (_url: string, init?: RequestInit) => {
        calls.push(typeof init?.body === 'string' ? JSON.parse(init.body) : null);
        return envelope({ valid: true });
      }) as unknown as typeof fetch,
      locale: () => null,
    });
    await gate();
    // Omitted, not null: the server negotiates from Accept-Language exactly as
    // it does for the index page, and `deny_unknown_fields` refuses a null.
    expect(calls[0]).toEqual({ nonce: FIRST });
  });

  it('never writes the ticket anywhere a later page could read it', async () => {
    const forbidden = ['localStorage', 'sessionStorage', 'indexedDB', 'document'] as const;
    const trip = vi.fn();
    const restore: Array<() => void> = [];
    for (const name of forbidden) {
      const original = Object.getOwnPropertyDescriptor(globalThis, name);
      Object.defineProperty(globalThis, name, {
        configurable: true,
        get() {
          trip(name);
          throw new Error(`${name} must never be touched`);
        },
      });
      restore.push(() => {
        if (original === undefined) delete (globalThis as Record<string, unknown>)[name];
        else Object.defineProperty(globalThis, name, original);
      });
    }
    try {
      const { gate, ticket } = gateOver([() => envelope({ renewed: RENEWED })]);
      await gate();
      expect(ticket.read()).toBe(RENEWED);
      // A renewed ticket is a live credential. Anything that outlives the
      // page holding it -- storage, a cookie, an attribute -- is a copy
      // nobody is going to remember to clear.
      expect(trip).not.toHaveBeenCalled();
    } finally {
      for (const undo of restore) undo();
    }
  });
});
