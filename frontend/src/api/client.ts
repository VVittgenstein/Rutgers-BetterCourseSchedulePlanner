type PrimitiveParam = string | number | boolean;
export type ApiQueryParamValue = PrimitiveParam | PrimitiveParam[];

export interface ApiErrorPayload {
  error?: {
    code?: string;
    message?: string;
    details?: string[];
    traceId?: string;
  };
}

export interface ApiError extends Error {
  status: number;
  code?: string;
  traceId?: string;
  details?: string[];
}

const DEFAULT_BASE_URL = '/api';

export const API_BASE_URL =
  (typeof import.meta !== 'undefined' && import.meta.env && import.meta.env.VITE_API_BASE_URL) || DEFAULT_BASE_URL;

export async function apiGet<TResponse>(
  path: string,
  params?: Record<string, ApiQueryParamValue>,
  signal?: AbortSignal,
): Promise<TResponse> {
  const url = new URL(trimSlash(API_BASE_URL) + ensureLeadingSlash(path), window.location.origin);
  if (params) {
    const query = new URLSearchParams();
    for (const [key, rawValue] of Object.entries(params)) {
      appendQueryValue(query, key, rawValue);
    }
    url.search = query.toString();
  }

  const response = await fetch(url.toString(), {
    method: 'GET',
    signal,
    headers: {
      Accept: 'application/json',
    },
    credentials: 'same-origin',
  });

  if (!response.ok) {
    const payload = (await parseErrorPayload(response)) as ApiErrorPayload | undefined;
    const error: ApiError = Object.assign(new Error(payload?.error?.message ?? `Request failed: ${response.status}`), {
      status: response.status,
      code: payload?.error?.code,
      details: payload?.error?.details,
      traceId: payload?.error?.traceId,
    });
    throw error;
  }

  return (await response.json()) as TResponse;
}

async function parseErrorPayload(response: Response) {
  try {
    return await response.json();
  } catch {
    return undefined;
  }
}

function appendQueryValue(params: URLSearchParams, key: string, value: ApiQueryParamValue | undefined) {
  if (value === undefined || value === null) return;
  if (Array.isArray(value)) {
    value.forEach((entry) => {
      if (entry === undefined || entry === null) return;
      params.append(key, String(entry));
    });
    return;
  }
  params.append(key, String(value));
}

function ensureLeadingSlash(path: string) {
  if (!path.startsWith('/')) {
    return `/${path}`;
  }
  return path;
}

function trimSlash(input: string) {
  if (!input) return '';
  return input.endsWith('/') ? input.slice(0, -1) : input;
}
