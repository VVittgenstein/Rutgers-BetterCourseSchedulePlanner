import { apiPost } from './client';
import type { ClaimLocalNotificationsResponse } from './types';

export interface ClaimLocalNotificationsPayload {
  deviceId: string;
  limit?: number;
}

export const claimLocalNotifications = (payload: ClaimLocalNotificationsPayload, signal?: AbortSignal) =>
  apiPost<ClaimLocalNotificationsResponse>('/notifications/local/claim', payload, signal);
