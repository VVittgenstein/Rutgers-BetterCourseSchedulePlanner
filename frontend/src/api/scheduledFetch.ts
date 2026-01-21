import { apiGet, apiPost, apiDelete, apiPatch } from './client';

export type FetchMode = 'full-init' | 'incremental';

export interface ScheduledFetchConfig {
  enabled: boolean;
  intervalMinutes: number;
  term: string;
  campus: string;
  mode: FetchMode;
}

export interface ScheduledFetchState {
  config: ScheduledFetchConfig;
  status: 'idle' | 'scheduled' | 'running';
  lastRunAt: string | null;
  lastRunStatus: 'success' | 'error' | null;
  lastRunMessage: string | null;
  nextRunAt: string | null;
}

export interface ScheduledFetchResponse {
  meta: {
    generatedAt: string;
    version: string;
  };
  data: ScheduledFetchState;
}

export interface StartScheduledFetchParams {
  term: string;
  campus: string;
  intervalMinutes?: number;
  mode?: FetchMode;
}

export async function getScheduledFetchState(signal?: AbortSignal): Promise<ScheduledFetchResponse> {
  return apiGet<ScheduledFetchResponse>('/scheduled-fetch', {}, signal);
}

export async function startScheduledFetch(
  params: StartScheduledFetchParams,
  signal?: AbortSignal
): Promise<ScheduledFetchResponse> {
  return apiPost<ScheduledFetchResponse>('/scheduled-fetch', params, signal);
}

export async function stopScheduledFetch(signal?: AbortSignal): Promise<ScheduledFetchResponse> {
  return apiDelete<ScheduledFetchResponse>('/scheduled-fetch', signal);
}

export async function updateScheduledFetch(
  params: Partial<StartScheduledFetchParams>,
  signal?: AbortSignal
): Promise<ScheduledFetchResponse> {
  return apiPatch<ScheduledFetchResponse>('/scheduled-fetch', params, signal);
}
