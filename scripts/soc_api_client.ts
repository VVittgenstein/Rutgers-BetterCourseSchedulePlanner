import { randomUUID } from 'node:crypto';
import { performance } from 'node:perf_hooks';

export type Endpoint = 'courses' | 'openSections';

export const ENDPOINTS: Endpoint[] = ['courses', 'openSections'];

const BASE_URL = 'https://classes.rutgers.edu/soc/api';
const TERM_ALIASES: Record<string, number> = {
  W: 0,
  WINTER: 0,
  WI: 0,
  S: 1,
  SP: 1,
  SPRING: 1,
  SU: 7,
  SUM: 7,
  SUMMER: 7,
  F: 9,
  FA: 9,
  FALL: 9
};

export interface SemesterParts {
  year: number;
  termCode: number;
  normalizedLabel: string;
}

export interface SOCRequestOptions {
  campus: string;
  subject?: string;
  level?: string;
  endpoint: Endpoint;
  timeoutMs?: number;
}

export interface ProbeResult {
  requestId: string;
  url: string;
  statusCode: number;
  statusText: string;
  durationMs: number;
  sizeBytes: number;
  body: unknown;
}

export type SOCErrorKind = 'HTTP' | 'TIMEOUT' | 'NETWORK' | 'JSON_PARSE';

export class SOCRequestError extends Error {
  constructor(
    message: string,
    public readonly kind: SOCErrorKind,
    public readonly requestId: string,
    public readonly endpoint: Endpoint,
    public readonly statusCode?: number,
    public readonly statusText?: string,
    public readonly retryHint?: string,
    public readonly detail?: string
  ) {
    super(message);
  }
}

export function decodeSemester(term: string): SemesterParts {
  const stripped = term.replace(/[-_\s]/g, '').toUpperCase();
  const fiveDigit = stripped.match(/^([0179])(\d{4})$/);
  if (fiveDigit) {
    const [, termCodeRaw, yearRaw] = fiveDigit;
    return {
      year: Number.parseInt(yearRaw, 10),
      termCode: Number.parseInt(termCodeRaw, 10),
      normalizedLabel: `${termCodeRaw}${yearRaw}`
    };
  }

  const swapped = stripped.match(/^(\d{4})([0179])$/);
  if (swapped) {
    const [, yearRaw, termCodeRaw] = swapped;
    return {
      year: Number.parseInt(yearRaw, 10),
      termCode: Number.parseInt(termCodeRaw, 10),
      normalizedLabel: `${termCodeRaw}${yearRaw}`
    };
  }

  const aliasMatch = stripped.match(/^([A-Z]+)(\d{4})$/);
  if (aliasMatch) {
    const [, alias, yearRaw] = aliasMatch;
    const termCode = TERM_ALIASES[alias];
    if (termCode === undefined) {
      throw new Error(`Unrecognized term alias: ${alias}`);
    }
    return {
      year: Number.parseInt(yearRaw, 10),
      termCode,
      normalizedLabel: `${termCode}${yearRaw}`
    };
  }

  throw new Error(`Unable to parse term "${term}". Expected formats like 12024, 20249, or FA2024.`);
}

export async function performProbe(options: SOCRequestOptions, semester: SemesterParts): Promise<ProbeResult> {
  const params = new URLSearchParams({
    year: String(semester.year),
    term: String(semester.termCode),
    campus: options.campus
  });
  if (options.subject) {
    params.set('subject', options.subject);
  }
  if (options.level) {
    params.set('level', options.level);
  }

  const url = `${BASE_URL}/${options.endpoint}.json?${params.toString()}`;
  const requestId = randomUUID();
  const controller = new AbortController();
  const timeoutMs = options.timeoutMs ?? 15000;
  const timeoutHandle = setTimeout(() => controller.abort(), timeoutMs);
  const started = performance.now();

  try {
    const response = await fetch(url, {
      headers: {
        'user-agent': 'BetterCourseScheduleProbe/1.0',
        accept: 'application/json, text/plain'
      },
      signal: controller.signal
    });

    const buffer = Buffer.from(await response.arrayBuffer());
    const durationMs = performance.now() - started;
    const sizeBytes = buffer.byteLength;

    if (!response.ok) {
      const retryHint = deriveRetryHint(response.status);
      emitStructuredError({
        requestId,
        endpoint: options.endpoint,
        url,
        httpStatus: response.status,
        statusText: response.statusText,
        retryHint,
        errorType: 'HTTP',
        detail: buffer.toString('utf-8').slice(0, 400)
      });
      throw new SOCRequestError(
        `Request failed with status ${response.status}`,
        'HTTP',
        requestId,
        options.endpoint,
        response.status,
        response.statusText,
        retryHint
      );
    }

    let body: unknown;
    try {
      body = JSON.parse(buffer.toString('utf-8'));
    } catch (error) {
      const parseError = error as Error;
      emitStructuredError({
        requestId,
        endpoint: options.endpoint,
        url,
        httpStatus: response.status,
        statusText: response.statusText,
        retryHint: 'Inspect response payload, JSON parse failed',
        errorType: 'JSON_PARSE',
        detail: parseError.message
      });
      throw new SOCRequestError(
        'Unable to parse JSON response',
        'JSON_PARSE',
        requestId,
        options.endpoint,
        response.status,
        response.statusText,
        'Inspect response payload, JSON parse failed',
        parseError.message
      );
    }

    return {
      requestId,
      url,
      statusCode: response.status,
      statusText: response.statusText,
      durationMs,
      sizeBytes,
      body
    };
  } catch (error) {
    if (error instanceof SOCRequestError) {
      throw error;
    }
    const err = error as Error;
    if (err.name === 'AbortError') {
      emitStructuredError({
        requestId,
        endpoint: options.endpoint,
        url,
        retryHint: 'Request timed out. Increase timeout or lower concurrency.',
        errorType: 'TIMEOUT',
        detail: 'AbortError triggered by timeout'
      });
      throw new SOCRequestError(
        'Request timed out',
        'TIMEOUT',
        requestId,
        options.endpoint,
        undefined,
        undefined,
        'Request timed out. Increase timeout or lower concurrency.'
      );
    }

    emitStructuredError({
      requestId,
      endpoint: options.endpoint,
      url,
      retryHint: 'Check network connectivity or VPN settings.',
      errorType: 'NETWORK',
      detail: err.message
    });
    throw new SOCRequestError(
      'Network error while contacting SOC API',
      'NETWORK',
      requestId,
      options.endpoint,
      undefined,
      undefined,
      'Check network connectivity or VPN settings.',
      err.message
    );
  } finally {
    clearTimeout(timeoutHandle);
  }
}

export function deriveRetryHint(status: number): string {
  if (status === 429) {
    return 'Hit rate-limit (429). Pause for 60s and retry with fewer parallel calls.';
  }
  if (status === 503 || status === 504) {
    return 'Server overloaded. Back off for 30s and retry.';
  }
  if (status >= 500) {
    return 'Server error. Retry after a short delay.';
  }
  if (status >= 400) {
    return 'Verify query parameters (term/campus) before retrying.';
  }
  return 'Retry details unavailable.';
}

function emitStructuredError(entry: Record<string, unknown>): void {
  const payload = {
    level: 'error',
    timestamp: new Date().toISOString(),
    ...entry
  };
  console.error(JSON.stringify(payload));
}
