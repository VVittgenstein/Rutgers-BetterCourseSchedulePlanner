import { apiPost } from './client';
import type {
  SubscribeRequestPayload,
  SubscribeResponsePayload,
  UnsubscribeRequestPayload,
  UnsubscribeResponsePayload,
} from './types';

export const subscribe = (payload: SubscribeRequestPayload, signal?: AbortSignal) =>
  apiPost<SubscribeResponsePayload>('/subscribe', payload, signal);

export const unsubscribe = (payload: UnsubscribeRequestPayload, signal?: AbortSignal) =>
  apiPost<UnsubscribeResponsePayload>('/unsubscribe', payload, signal);
