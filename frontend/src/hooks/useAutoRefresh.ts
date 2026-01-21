import { useCallback, useEffect, useRef, useState } from 'react';

const ENABLED_STORAGE_KEY = 'bcsp:autoRefreshEnabled';
const INTERVAL_STORAGE_KEY = 'bcsp:autoRefreshInterval';
const DEFAULT_INTERVAL_SECONDS = 30;
const MIN_INTERVAL_SECONDS = 15;
const MAX_INTERVAL_SECONDS = 120;

export type AutoRefreshStatus = 'idle' | 'waiting' | 'refreshing';

export interface AutoRefreshControls {
  enabled: boolean;
  intervalSeconds: number;
  status: AutoRefreshStatus;
  secondsUntilNext: number | null;
  enable: () => void;
  disable: () => void;
  toggle: () => void;
  setIntervalSeconds: (seconds: number) => void;
  triggerNow: () => void;
}

const readStoredEnabled = (): boolean => {
  if (typeof window === 'undefined') return false;
  try {
    const raw = window.localStorage.getItem(ENABLED_STORAGE_KEY);
    return raw === '1' || raw === 'true';
  } catch {
    return false;
  }
};

const readStoredInterval = (): number => {
  if (typeof window === 'undefined') return DEFAULT_INTERVAL_SECONDS;
  try {
    const raw = window.localStorage.getItem(INTERVAL_STORAGE_KEY);
    if (raw) {
      const parsed = parseInt(raw, 10);
      if (!isNaN(parsed) && parsed >= MIN_INTERVAL_SECONDS && parsed <= MAX_INTERVAL_SECONDS) {
        return parsed;
      }
    }
  } catch {
    // ignore
  }
  return DEFAULT_INTERVAL_SECONDS;
};

export interface UseAutoRefreshOptions {
  onRefresh: () => void;
  intervalSeconds?: number;
}

export function useAutoRefresh(options: UseAutoRefreshOptions): AutoRefreshControls {
  const { onRefresh } = options;

  const [enabled, setEnabled] = useState<boolean>(() => readStoredEnabled());
  const [intervalSeconds, setIntervalSecondsState] = useState<number>(() =>
    options.intervalSeconds ?? readStoredInterval()
  );
  const [status, setStatus] = useState<AutoRefreshStatus>('idle');
  const [secondsUntilNext, setSecondsUntilNext] = useState<number | null>(null);

  const timerRef = useRef<number | null>(null);
  const countdownRef = useRef<number | null>(null);
  const nextRefreshAtRef = useRef<number | null>(null);
  const onRefreshRef = useRef(onRefresh);

  useEffect(() => {
    onRefreshRef.current = onRefresh;
  }, [onRefresh]);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(ENABLED_STORAGE_KEY, enabled ? '1' : '0');
    } catch {
      // best effort
    }
  }, [enabled]);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(INTERVAL_STORAGE_KEY, String(intervalSeconds));
    } catch {
      // best effort
    }
  }, [intervalSeconds]);

  const clearTimers = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    if (countdownRef.current !== null) {
      window.clearInterval(countdownRef.current);
      countdownRef.current = null;
    }
    nextRefreshAtRef.current = null;
    setSecondsUntilNext(null);
  }, []);

  const scheduleRefresh = useCallback(
    (delayMs: number) => {
      clearTimers();
      if (!enabled) return;

      const targetTime = Date.now() + delayMs;
      nextRefreshAtRef.current = targetTime;
      setStatus('waiting');

      const updateCountdown = () => {
        const now = Date.now();
        const remaining = Math.max(0, Math.ceil((targetTime - now) / 1000));
        setSecondsUntilNext(remaining);
      };
      updateCountdown();
      countdownRef.current = window.setInterval(updateCountdown, 1000);

      timerRef.current = window.setTimeout(() => {
        if (!enabled) return;
        setStatus('refreshing');
        setSecondsUntilNext(null);

        try {
          onRefreshRef.current();
        } catch {
          // ignore errors
        }

        setStatus('waiting');
        scheduleRefresh(intervalSeconds * 1000);
      }, delayMs);
    },
    [clearTimers, enabled, intervalSeconds]
  );

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible' && enabled) {
        scheduleRefresh(intervalSeconds * 1000);
      } else if (document.visibilityState === 'hidden') {
        clearTimers();
        setStatus('idle');
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, [clearTimers, enabled, intervalSeconds, scheduleRefresh]);

  useEffect(() => {
    if (enabled && document.visibilityState === 'visible') {
      scheduleRefresh(intervalSeconds * 1000);
    } else {
      clearTimers();
      setStatus('idle');
    }
    return () => clearTimers();
  }, [clearTimers, enabled, intervalSeconds, scheduleRefresh]);

  const enable = useCallback(() => {
    setEnabled(true);
  }, []);

  const disable = useCallback(() => {
    setEnabled(false);
    clearTimers();
    setStatus('idle');
  }, [clearTimers]);

  const toggle = useCallback(() => {
    if (enabled) {
      disable();
    } else {
      enable();
    }
  }, [disable, enable, enabled]);

  const setIntervalSeconds = useCallback((seconds: number) => {
    const clamped = Math.max(MIN_INTERVAL_SECONDS, Math.min(MAX_INTERVAL_SECONDS, seconds));
    setIntervalSecondsState(clamped);
  }, []);

  const triggerNow = useCallback(() => {
    if (!enabled) return;
    clearTimers();
    setStatus('refreshing');

    try {
      onRefreshRef.current();
    } catch {
      // ignore errors
    }

    setStatus('waiting');
    scheduleRefresh(intervalSeconds * 1000);
  }, [clearTimers, enabled, intervalSeconds, scheduleRefresh]);

  return {
    enabled,
    intervalSeconds,
    status,
    secondsUntilNext,
    enable,
    disable,
    toggle,
    setIntervalSeconds,
    triggerNow,
  };
}
