import { apiGet, apiPost } from './client';
import type { FetchStatusResponse } from './types';
import type { TermSeason } from '../utils/term';

export interface StartFetchPayload {
  year?: number;
  season?: TermSeason;
  term?: string;
  campus: string;
  mode?: 'full-init' | 'incremental';
}

export function fetchJobStatus() {
  return apiGet<FetchStatusResponse>('/fetch');
}

export function startFetchJob(payload: StartFetchPayload) {
  return apiPost<FetchStatusResponse>('/fetch', payload);
}
