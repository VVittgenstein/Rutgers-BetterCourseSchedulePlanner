import { useCallback, useEffect, useRef, useState } from 'react';

import type { ApiError } from '../api/client';
import { claimLocalNotifications } from '../api/notifications';
import type { LocalNotification } from '../api/types';

const DEVICE_STORAGE_KEY = 'bcsp:localSoundDeviceId';
const ENABLED_STORAGE_KEY = 'bcsp:localSoundEnabled';
const DEFAULT_POLL_INTERVAL_MS = 7000;
const ERROR_BACKOFF_MS = 15000;
const MAX_TOASTS = 5;

export type LocalSoundStatus = 'idle' | 'polling' | 'error';

export interface LocalSoundToast {
  id: string;
  notification: LocalNotification;
  receivedAt: number;
}

export interface LocalSoundControls {
  deviceId: string;
  enabled: boolean;
  status: LocalSoundStatus;
  lastError: string | null;
  lastPolledAt: number | null;
  audioBlocked: boolean;
  toasts: LocalSoundToast[];
  enable: () => Promise<void>;
  disable: () => void;
  toggle: () => Promise<void>;
  regenerateDeviceId: () => string;
  resumeAudio: () => Promise<void>;
  dismissToast: (id: string) => void;
}

const randomChunk = () => Math.random().toString(36).slice(2, 8);

const buildDeviceId = () => {
  const uuid = typeof crypto !== 'undefined' && 'randomUUID' in crypto ? crypto.randomUUID() : randomChunk();
  const compact = uuid.replace(/[^a-zA-Z0-9]/g, '').slice(0, 14);
  return `lsnd-${compact || randomChunk()}`;
};

const readStoredDeviceId = () => {
  if (typeof window === 'undefined') return buildDeviceId();
  try {
    const stored = window.localStorage.getItem(DEVICE_STORAGE_KEY);
    if (stored && stored.trim().length >= 6) {
      return stored.trim();
    }
  } catch {
    // ignore
  }
  return buildDeviceId();
};

const readStoredEnabled = () => {
  if (typeof window === 'undefined') return false;
  try {
    const raw = window.localStorage.getItem(ENABLED_STORAGE_KEY);
    return raw === '1' || raw === 'true';
  } catch {
    return false;
  }
};

export function useLocalSoundNotifications(options?: {
  pollIntervalMs?: number;
  backoffMs?: number;
}): LocalSoundControls {
  const [deviceId, setDeviceId] = useState<string>(() => readStoredDeviceId());
  const [enabled, setEnabled] = useState<boolean>(() => readStoredEnabled());
  const [status, setStatus] = useState<LocalSoundStatus>('idle');
  const [lastError, setLastError] = useState<string | null>(null);
  const [lastPolledAt, setLastPolledAt] = useState<number | null>(null);
  const [audioBlocked, setAudioBlocked] = useState(false);
  const [toasts, setToasts] = useState<LocalSoundToast[]>([]);
  const timerRef = useRef<number | null>(null);
  const inflightRef = useRef(false);
  const audioContextRef = useRef<AudioContext | null>(null);
  const runPollRef = useRef<() => Promise<void>>();
  const pollInterval = options?.pollIntervalMs ?? DEFAULT_POLL_INTERVAL_MS;
  const backoffInterval = options?.backoffMs ?? ERROR_BACKOFF_MS;

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(DEVICE_STORAGE_KEY, deviceId);
    } catch {
      // best effort
    }
  }, [deviceId]);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(ENABLED_STORAGE_KEY, enabled ? '1' : '0');
    } catch {
      // best effort
    }
  }, [enabled]);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const ensureAudioContext = useCallback(async () => {
    if (typeof window === 'undefined') return null;
    let ctx = audioContextRef.current;
    if (!ctx) {
      try {
        ctx = new AudioContext();
        audioContextRef.current = ctx;
      } catch {
        setAudioBlocked(true);
        return null;
      }
    }
    if (ctx.state === 'suspended') {
      try {
        await ctx.resume();
      } catch {
        setAudioBlocked(true);
        return ctx;
      }
    }
    setAudioBlocked(ctx.state !== 'running');
    return ctx;
  }, []);

  const playTone = useCallback(async () => {
    const ctx = await ensureAudioContext();
    if (!ctx || ctx.state !== 'running') {
      setAudioBlocked(true);
      return;
    }
    const duration = 0.35;
    const now = ctx.currentTime;
    const oscillator = ctx.createOscillator();
    const gain = ctx.createGain();
    oscillator.type = 'sine';
    oscillator.frequency.setValueAtTime(1040, now);
    oscillator.frequency.exponentialRampToValueAtTime(520, now + duration);
    gain.gain.setValueAtTime(0.001, now);
    gain.gain.exponentialRampToValueAtTime(0.3, now + 0.03);
    gain.gain.exponentialRampToValueAtTime(0.001, now + duration);
    oscillator.connect(gain);
    gain.connect(ctx.destination);
    oscillator.start();
    oscillator.stop(now + duration);
  }, [ensureAudioContext]);

  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }, []);

  const handleNotifications = useCallback(
    async (notifications: LocalNotification[]) => {
      if (!notifications.length) return;
      setToasts((prev) => {
        const next = [...notifications.map((entry) => ({
          id: `${entry.notificationId}-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
          notification: entry,
          receivedAt: Date.now(),
        })), ...prev];
        return next.slice(0, MAX_TOASTS);
      });
      await playTone();
    },
    [playTone],
  );

  const scheduleNextPoll = useCallback(
    (delayMs: number) => {
      if (!enabled) return;
      clearTimer();
      timerRef.current = window.setTimeout(() => {
        if (runPollRef.current) {
          void runPollRef.current();
        }
      }, delayMs);
    },
    [clearTimer, enabled],
  );

  const runPoll = useCallback(async () => {
    if (!enabled || inflightRef.current || !deviceId) return;
    inflightRef.current = true;
    setStatus('polling');
    try {
      const response = await claimLocalNotifications({ deviceId });
      setLastPolledAt(Date.now());
      setLastError(null);
      setStatus('idle');
      await handleNotifications(response.notifications);
      scheduleNextPoll(pollInterval);
    } catch (error) {
      const apiError = error as ApiError;
      setLastError(apiError.message);
      setStatus('error');
      scheduleNextPoll(backoffInterval);
    } finally {
      inflightRef.current = false;
    }
  }, [backoffInterval, deviceId, enabled, handleNotifications, pollInterval, scheduleNextPoll]);

  useEffect(() => {
    runPollRef.current = runPoll;
  }, [runPoll]);

  useEffect(() => {
    if (enabled) {
      scheduleNextPoll(0);
    } else {
      clearTimer();
      setStatus('idle');
    }
    return () => clearTimer();
  }, [clearTimer, deviceId, enabled, scheduleNextPoll]);

  const enable = useCallback(async () => {
    await ensureAudioContext();
    setEnabled(true);
  }, [ensureAudioContext]);

  const disable = useCallback(() => {
    setEnabled(false);
    clearTimer();
  }, [clearTimer]);

  const toggle = useCallback(async () => {
    if (enabled) {
      disable();
      return;
    }
    await enable();
  }, [disable, enable, enabled]);

  const regenerateDeviceId = useCallback(() => {
    const next = buildDeviceId();
    setDeviceId(next);
    setToasts([]);
    return next;
  }, []);

  const resumeAudio = useCallback(async () => {
    await ensureAudioContext();
  }, [ensureAudioContext]);

  return {
    deviceId,
    enabled,
    status,
    lastError,
    lastPolledAt,
    audioBlocked,
    toasts,
    enable,
    disable,
    toggle,
    regenerateDeviceId,
    resumeAudio,
    dismissToast,
  };
}
