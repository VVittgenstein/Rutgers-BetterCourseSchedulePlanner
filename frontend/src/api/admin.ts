import { apiGet, apiPut } from './client';
import type { MailConfigResponse, MailConfigUpdatePayload } from './types';

export const fetchMailConfig = (signal?: AbortSignal) =>
  apiGet<MailConfigResponse>('/admin/mail-config', undefined, signal);

export const updateMailConfig = (payload: MailConfigUpdatePayload, signal?: AbortSignal) =>
  apiPut<MailConfigResponse>('/admin/mail-config', payload, signal);
