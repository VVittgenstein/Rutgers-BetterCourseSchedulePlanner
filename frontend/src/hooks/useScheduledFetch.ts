import { useCallback, useEffect, useRef, useState } from 'react';

import type { ApiError } from '../api/client';
import {
  getScheduledFetchState,
  startScheduledFetch,
  stopScheduledFetch,
  type ScheduledFetchState,
  type StartScheduledFetchParams,
} from '../api/scheduledFetch';

const DEFAULT_POLL_INTERVAL_MS = 10000; // 10 seconds

export interface ScheduledFetchControls {
  state: ScheduledFetchState | null;
  isLoading: boolean;
  error: string | null;
  start: (params: StartScheduledFetchParams) => Promise<void>;
  stop: () => Promise<void>;
  refresh: () => void;
}

export function useScheduledFetch(): ScheduledFetchControls {
  const [state, setState] = useState<ScheduledFetchState | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timerRef = useRef<number | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const fetchState = useCallback(async () => {
    abortControllerRef.current?.abort();
    const controller = new AbortController();
    abortControllerRef.current = controller;

    try {
      const response = await getScheduledFetchState(controller.signal);
      setState(response.data);
      setError(null);
      return response.data;
    } catch (err) {
      if ((err as Error).name === 'AbortError') {
        return null;
      }
      const apiError = err as ApiError;
      setError(apiError.message);
      return null;
    }
  }, []);

  const schedulePoll = useCallback(() => {
    clearTimer();
    timerRef.current = window.setTimeout(async () => {
      const newState = await fetchState();
      // Continue polling if enabled or running
      if (newState?.config.enabled || newState?.status === 'running') {
        schedulePoll();
      }
    }, DEFAULT_POLL_INTERVAL_MS);
  }, [clearTimer, fetchState]);

  // Initial fetch on mount
  useEffect(() => {
    setIsLoading(true);
    fetchState().finally(() => setIsLoading(false));

    return () => {
      clearTimer();
      abortControllerRef.current?.abort();
    };
  }, [clearTimer, fetchState]);

  // Start polling when enabled
  useEffect(() => {
    if (state?.config.enabled || state?.status === 'running') {
      schedulePoll();
    } else {
      clearTimer();
    }
  }, [state?.config.enabled, state?.status, clearTimer, schedulePoll]);

  const start = useCallback(
    async (params: StartScheduledFetchParams) => {
      setIsLoading(true);
      setError(null);
      try {
        const response = await startScheduledFetch(params);
        setState(response.data);
      } catch (err) {
        const apiError = err as ApiError;
        setError(apiError.message);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const stop = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const response = await stopScheduledFetch();
      setState(response.data);
    } catch (err) {
      const apiError = err as ApiError;
      setError(apiError.message);
      throw err;
    } finally {
      setIsLoading(false);
    }
  }, []);

  const refresh = useCallback(() => {
    void fetchState();
  }, [fetchState]);

  return {
    state,
    isLoading,
    error,
    start,
    stop,
    refresh,
  };
}
