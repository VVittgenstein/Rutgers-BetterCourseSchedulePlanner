import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

import type {
  ActiveWatchId,
  OpenEpisodeV1,
  OpenFreshnessV1,
  OpenObservationV1,
  OpenRefreshStatusV1,
  OpenSectionStatusV1,
  ProductRuntimePort,
  SectionKey,
  WatchAlertV1,
  WatchClientCommandV1,
  WatchConnectionState,
  WatchPolicyV1,
  WatchServerEventV1,
  WatchStartItemV1,
  WsServerEnvelope,
} from '../product';
import {
  WatchAudioController,
  type WatchAudioUnlockResult,
} from './audio';

export type WatchAudioState = WatchAudioUnlockResult | 'MUTED' | 'UNLOCKING';

export const MAX_SELECTED_SECTIONS = 9;
export const DEFAULT_WATCH_POLICY: WatchPolicyV1 = {
  notificationMode: 'ONE_SHOT',
  maxAudible: 3,
  continuousDuration: { kind: 'FINITE', seconds: 600 },
};

export interface ActiveWatchView {
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
  readonly startedAt: string;
  readonly policy: WatchPolicyV1;
}

export type WatchNoticeCode =
  | 'SELECTION_LIMIT'
  | 'START_REJECTED'
  | 'COMMAND_FAILED'
  | 'CONNECTION_LOST'
  | 'WATCH_STOPPED'
  | 'AUDIO_BLOCKED'
  | 'AUDIO_FAILED'
  | 'AUDIO_CAP_REACHED'
  | 'AUDIO_CUE_QUEUED'
  | 'WATCH_ALERT_OPEN';

export interface WatchNotice {
  readonly id: number;
  readonly code: WatchNoticeCode;
  readonly tone: 'STATUS' | 'ALERT';
  readonly sectionKey?: SectionKey | undefined;
  readonly detail?: string | undefined;
}

export interface LiveWatchValue {
  readonly selected: readonly SectionKey[];
  readonly pending: readonly SectionKey[];
  readonly active: readonly ActiveWatchView[];
  readonly observations: readonly OpenObservationV1[];
  readonly episodes: readonly OpenEpisodeV1[];
  readonly alerts: readonly WatchAlertV1[];
  readonly notices: readonly WatchNotice[];
  readonly connection: WatchConnectionState;
  readonly audioState: WatchAudioState;
  readonly muted: boolean;
  readonly volume: number;
  readonly continuousEpisodeIds: readonly string[];
  readonly batchStatuses: readonly OpenRefreshStatusV1[];
  readonly sectionStatuses: readonly OpenSectionStatusV1[];
  readonly telemetryLoading: boolean;
  readonly telemetryError: boolean;
  isSelected(sectionKey: SectionKey): boolean;
  isActive(sectionKey: SectionKey): boolean;
  select(sectionKey: SectionKey): void;
  remove(sectionKey: SectionKey): void;
  startSelected(policy: WatchPolicyV1): void;
  stop(watch: ActiveWatchView): void;
  updatePolicy(watch: ActiveWatchView, policy: WatchPolicyV1): void;
  acknowledge(episode: OpenEpisodeV1): void;
  acknowledgeAll(): void;
  resume(episode: OpenEpisodeV1): void;
  resetAudibleCount(watch: ActiveWatchView): void;
  dismissAlert(alert: WatchAlertV1): void;
  dismissNotice(id: number): void;
  disconnect(): void;
  enableSound(): Promise<WatchAudioUnlockResult>;
  testSound(): Promise<WatchAudioUnlockResult>;
  setMuted(muted: boolean): void;
  setVolume(volume: number): void;
  refreshTelemetry(): Promise<void>;
}

const LiveWatchContext = createContext<LiveWatchValue | null>(null);

function sectionIdentity(sectionKey: SectionKey): string {
  return `${sectionKey.term}\u0000${sectionKey.campus}\u0000${sectionKey.index}`;
}

function sameSection(left: SectionKey, right: SectionKey): boolean {
  return sectionIdentity(left) === sectionIdentity(right);
}

function sameBatch(
  left: { readonly term: string; readonly campus: string },
  right: { readonly term: string; readonly campus: string },
): boolean {
  return left.term === right.term && left.campus === right.campus;
}

function batchIdentity(batch: { readonly term: string; readonly campus: string }): string {
  return `${batch.term}\u0000${batch.campus}`;
}

function sectionTelemetryFailureKey(sectionKey: SectionKey): string {
  return `section:${sectionIdentity(sectionKey)}`;
}

function batchTelemetryFailureKey(
  batch: { readonly term: string; readonly campus: string },
): string {
  return `batch:${batchIdentity(batch)}`;
}

function expireFreshness(freshness: OpenFreshnessV1, now: number): OpenFreshnessV1 {
  if (freshness.state !== 'FRESH' || freshness.freshUntil === null) return freshness;
  const freshUntil = Date.parse(freshness.freshUntil);
  if (!Number.isFinite(freshUntil) || now <= freshUntil) return freshness;
  const observedAt = freshness.observedAt === null ? Number.NaN : Date.parse(freshness.observedAt);
  const elapsedSeconds = Number.isFinite(observedAt)
    ? Math.max(0, Math.floor((now - observedAt) / 1_000))
    : freshness.lastKnownGoodAgeSeconds;
  return {
    ...freshness,
    state: 'STALE',
    lastKnownGoodAgeSeconds: elapsedSeconds,
    uncertainty: 'STALE_LAST_KNOWN_GOOD',
  };
}

function normalizeSectionStatus(status: OpenSectionStatusV1, now = Date.now()): OpenSectionStatusV1 {
  return { ...status, freshness: expireFreshness(status.freshness, now) };
}

function normalizeBatchStatus(status: OpenRefreshStatusV1, now = Date.now()): OpenRefreshStatusV1 {
  return { ...status, freshness: expireFreshness(status.freshness, now) };
}

function futureFreshnessExpiry(freshness: OpenFreshnessV1, now: number): number | null {
  if (freshness.state !== 'FRESH' || freshness.freshUntil === null) return null;
  const expiry = Date.parse(freshness.freshUntil);
  return Number.isFinite(expiry) && expiry > now ? expiry : null;
}

const MAX_BROWSER_TIMEOUT_MILLISECONDS = 2_147_000_000;

function replaceByIdentity<T>(
  values: readonly T[],
  next: T,
  identity: (value: T) => string,
): readonly T[] {
  const key = identity(next);
  const index = values.findIndex((value) => identity(value) === key);
  if (index < 0) return [...values, next];
  return values.map((value, valueIndex) => valueIndex === index ? next : value);
}

export interface LiveWatchProviderProps {
  readonly children: ReactNode;
  readonly runtime: ProductRuntimePort;
  readonly audio?: WatchAudioController | undefined;
  readonly initialSelected?: readonly SectionKey[] | undefined;
  readonly initialVolume?: number | undefined;
  readonly onSelectedChange?: ((selected: readonly SectionKey[]) => void) | undefined;
  readonly onVolumeChange?: ((volume: number) => void) | undefined;
}

export function LiveWatchProvider({
  audio,
  children,
  initialSelected = [],
  initialVolume = 70,
  onSelectedChange,
  onVolumeChange,
  runtime,
}: LiveWatchProviderProps) {
  const [audioController] = useState(() => audio ?? new WatchAudioController());
  const [selected, setSelected] = useState<readonly SectionKey[]>(() =>
    initialSelected.slice(0, MAX_SELECTED_SECTIONS));
  const [pending, setPending] = useState<readonly SectionKey[]>([]);
  const [active, setActive] = useState<readonly ActiveWatchView[]>([]);
  const [observations, setObservations] = useState<readonly OpenObservationV1[]>([]);
  const [episodes, setEpisodes] = useState<readonly OpenEpisodeV1[]>([]);
  const [alerts, setAlerts] = useState<readonly WatchAlertV1[]>([]);
  const [notices, setNotices] = useState<readonly WatchNotice[]>([]);
  const [connection, setConnection] = useState<WatchConnectionState>(runtime.watch.state);
  const [audioState, setAudioState] = useState<WatchAudioState>('MUTED');
  const [muted, setMutedState] = useState(true);
  const [volume, setVolumeState] = useState(() =>
    Math.min(100, Math.max(0, Number.isFinite(initialVolume) ? initialVolume : 70)));
  const [continuousEpisodeIds, setContinuousEpisodeIds] = useState<readonly string[]>([]);
  const [batchStatuses, setBatchStatuses] = useState<readonly OpenRefreshStatusV1[]>([]);
  const [sectionStatuses, setSectionStatuses] = useState<readonly OpenSectionStatusV1[]>([]);
  const [telemetryLoading, setTelemetryLoading] = useState(false);
  const [telemetryError, setTelemetryError] = useState(false);
  const selectedRef = useRef(selected);
  const activeRef = useRef(active);
  const mutedRef = useRef(muted);
  const volumeRef = useRef(volume);
  const continuousEpisodeIdsRef = useRef(continuousEpisodeIds);
  const pendingPolicies = useRef(new Map<string, WatchPolicyV1>());
  const queuedStart = useRef<readonly WatchStartItemV1[] | null>(null);
  const noticeId = useRef(0);
  const telemetryAbort = useRef<AbortController | null>(null);
  const telemetryEpoch = useRef(0);
  const batchRequestRevisions = useRef(new Map<string, number>());
  const sectionRequestRevisions = useRef(new Map<string, number>());
  const hadConnection = useRef(runtime.watch.state === 'OPEN');
  const announcedAlerts = useRef(new Set<string>());
  const announcedAudioCaps = useRef(new Set<ActiveWatchId>());
  const telemetryFailures = useRef(new Set<string>());
  const audioProviderMounted = useRef(false);

  useEffect(() => { selectedRef.current = selected; }, [selected]);
  useEffect(() => { activeRef.current = active; }, [active]);
  useEffect(() => { mutedRef.current = muted; }, [muted]);
  useEffect(() => { volumeRef.current = volume; }, [volume]);
  useEffect(() => { continuousEpisodeIdsRef.current = continuousEpisodeIds; }, [continuousEpisodeIds]);

  const addNotice = useCallback((
    code: WatchNoticeCode,
    tone: WatchNotice['tone'],
    sectionKey?: SectionKey,
    detail?: string,
  ) => {
    noticeId.current += 1;
    const notice: WatchNotice = {
      id: noticeId.current,
      code,
      tone,
      ...(sectionKey === undefined ? {} : { sectionKey }),
      ...(detail === undefined ? {} : { detail }),
    };
    setNotices((current) => [...current.slice(-5), notice]);
  }, []);

  const setTelemetryFailure = useCallback((key: string, failed: boolean) => {
    if (failed) telemetryFailures.current.add(key);
    else telemetryFailures.current.delete(key);
    setTelemetryError(telemetryFailures.current.size > 0);
  }, []);

  const send = useCallback((command: WatchClientCommandV1): boolean => {
    try {
      runtime.watch.send(command);
      return true;
    } catch (error) {
      addNotice('COMMAND_FAILED', 'ALERT', undefined, error instanceof Error ? error.message : undefined);
      return false;
    }
  }, [addNotice, runtime]);

  const sendQueuedStart = useCallback(() => {
    const items = queuedStart.current;
    if (items === null || items.length === 0 || runtime.watch.state !== 'OPEN') return;
    if (send({ type: 'START_WATCH', items })) queuedStart.current = null;
  }, [runtime, send]);

  const loadBatchStatus = useCallback(async (
    batch: { readonly term: string; readonly campus: string },
  ) => {
    const identity = batchIdentity(batch);
    const epoch = telemetryEpoch.current;
    const revision = (batchRequestRevisions.current.get(identity) ?? 0) + 1;
    batchRequestRevisions.current.set(identity, revision);
    try {
      const response = await runtime.product.openStatus({ contractVersion: 1, batch });
      const isCurrent = telemetryEpoch.current === epoch
        && batchRequestRevisions.current.get(identity) === revision;
      const isRelevant = selectedRef.current.some((section) => sameBatch(section, batch))
        || activeRef.current.some((watch) => sameBatch(watch.sectionKey, batch));
      if (!isCurrent || !isRelevant) return;
      const status = normalizeBatchStatus(response);
      setBatchStatuses((current) => replaceByIdentity(
        current,
        status,
        (value) => batchIdentity(value.batch),
      ));
      setTelemetryFailure(batchTelemetryFailureKey(batch), false);
    } catch {
      const isCurrent = telemetryEpoch.current === epoch
        && batchRequestRevisions.current.get(identity) === revision;
      const isRelevant = selectedRef.current.some((section) => sameBatch(section, batch))
        || activeRef.current.some((watch) => sameBatch(watch.sectionKey, batch));
      if (isCurrent && isRelevant) setTelemetryFailure(batchTelemetryFailureKey(batch), true);
    }
  }, [runtime, setTelemetryFailure]);

  const startContinuousAudio = useCallback((nextVolume: number, nextMuted: boolean) => {
    const outcome = audioController.startContinuous(nextVolume, nextMuted);
    if (outcome === 'AUTOPLAY_BLOCKED') {
      setAudioState('BLOCKED');
      addNotice('AUDIO_BLOCKED', 'ALERT');
    } else if (outcome === 'FAILED') {
      setAudioState('FAILED');
      addNotice('AUDIO_FAILED', 'ALERT');
    }
    return outcome;
  }, [addNotice, audioController]);

  useEffect(() => {
    const normalized = Math.min(
      100,
      Math.max(0, Number.isFinite(initialVolume) ? initialVolume : 70),
    );
    if (normalized === volumeRef.current) return;
    volumeRef.current = normalized;
    setVolumeState(normalized);
    if (continuousEpisodeIdsRef.current.length > 0) {
      startContinuousAudio(normalized, mutedRef.current);
    }
  }, [initialVolume, startContinuousAudio]);

  const handleServerEvent = useCallback((
    envelope: WsServerEnvelope<WatchServerEventV1>,
  ) => {
    const event = envelope.payload;
    if (event.type === 'START_RESULT') {
      const completedKeys = event.result.items.map((item) => sectionIdentity(item.sectionKey));
      setPending((current) => current.filter((key) => !completedKeys.includes(sectionIdentity(key))));
      for (const item of event.result.items) {
        const identity = sectionIdentity(item.sectionKey);
        if (item.status === 'ACTIVE') {
          const policy = pendingPolicies.current.get(identity) ?? DEFAULT_WATCH_POLICY;
          setActive((current) => replaceByIdentity(current, {
            activeWatchId: item.activeWatchId,
            sectionKey: item.sectionKey,
            startedAt: item.startedAt,
            policy,
          }, (value) => sectionIdentity(value.sectionKey)));
        } else {
          addNotice('START_REJECTED', 'ALERT', item.sectionKey, item.reason);
        }
        pendingPolicies.current.delete(identity);
      }
      return;
    }
    if (event.type === 'WATCH_STOPPED') {
      announcedAudioCaps.current.delete(event.stopped.activeWatchId);
      setActive((current) => current.filter((watch) => watch.activeWatchId !== event.stopped.activeWatchId));
      setObservations((current) => current.filter((observation) =>
        !sameSection(observation.sectionKey, event.stopped.sectionKey)));
      setEpisodes((current) => current.filter((episode) => episode.activeWatchId !== event.stopped.activeWatchId));
      setAlerts((current) => current.filter((alert) => alert.episode.activeWatchId !== event.stopped.activeWatchId));
      addNotice('WATCH_STOPPED', 'STATUS', event.stopped.sectionKey, event.stopped.reason);
      return;
    }
    if (event.type === 'OPEN_OBSERVATION') {
      const observation = event.fanout.observation;
      setObservations((current) => replaceByIdentity(
        current,
        observation,
        (value) => sectionIdentity(value.sectionKey),
      ));
      const projected = normalizeSectionStatus({
        contractVersion: 1,
        sectionKey: observation.sectionKey,
        state: observation.state,
        lastObservationId: observation.observationId,
        catalogContentVersion: observation.catalogContentVersion,
        freshness: {
          state: 'FRESH',
          observedAt: observation.observedAt,
          freshUntil: observation.freshUntil,
          lastKnownGoodAgeSeconds: 0,
          uncertainty: null,
        },
        schedulerLagMilliseconds: observation.schedulerLagMilliseconds,
        counterSnapshot: observation.counterSnapshot,
      });
      const projectedIdentity = sectionIdentity(projected.sectionKey);
      sectionRequestRevisions.current.set(
        projectedIdentity,
        (sectionRequestRevisions.current.get(projectedIdentity) ?? 0) + 1,
      );
      setSectionStatuses((current) => replaceByIdentity(
        current,
        projected,
        (value) => sectionIdentity(value.sectionKey),
      ));
      setTelemetryFailure(sectionTelemetryFailureKey(observation.sectionKey), false);
      void loadBatchStatus(observation.batch);
      return;
    }
    if (event.type === 'EPISODE_UPDATED') {
      setEpisodes((current) => replaceByIdentity(current, event.episode, (value) => value.episodeId));
      return;
    }
    if (event.type === 'ALERT_UPDATED') {
      if (!event.alert.visible || event.alert.disposition === 'CLOSED' || event.alert.disposition === 'DISMISSED') {
        setAlerts((current) => current.filter((alert) => alert.alertId !== event.alert.alertId));
      } else {
        setAlerts((current) => replaceByIdentity(current, event.alert, (value) => value.alertId));
      }
      if (!announcedAlerts.current.has(event.alert.alertId) && event.alert.visible) {
        announcedAlerts.current.add(event.alert.alertId);
        const announcer: WatchNotice = {
          id: ++noticeId.current,
          code: 'WATCH_ALERT_OPEN',
          tone: 'ALERT',
          sectionKey: event.alert.episode.sectionKey,
          detail: 'OPEN_ALERT',
        };
        setNotices((current) => [...current.slice(-5), announcer]);
      }
      return;
    }
    if (event.type === 'AUDIO_DISPOSITION') {
      const disposition = event.audio;
      if (disposition.disposition === 'CUE_REQUESTED') {
        announcedAudioCaps.current.delete(disposition.cue.activeWatchId);
        const outcome = audioController.play(disposition.cue, volumeRef.current, mutedRef.current);
        if (outcome === 'AUTOPLAY_BLOCKED') {
          setAudioState('BLOCKED');
          addNotice('AUDIO_BLOCKED', 'ALERT', disposition.cue.sectionKey);
        } else if (outcome === 'FAILED') {
          setAudioState('FAILED');
          addNotice('AUDIO_FAILED', 'ALERT', disposition.cue.sectionKey);
        }
        send({
          type: 'REPORT_CUE_OUTCOME',
          report: {
            cueId: disposition.cue.cueId,
            activeWatchId: disposition.cue.activeWatchId,
            sectionKey: disposition.cue.sectionKey,
            outcome,
            reportedAt: new Date().toISOString(),
          },
        });
      } else if (disposition.disposition === 'CUE_QUEUED') {
        addNotice('AUDIO_CUE_QUEUED', 'STATUS', disposition.sectionKey);
      } else if (disposition.disposition === 'SILENT_MAX_AUDIBLE') {
        if (!announcedAudioCaps.current.has(disposition.activeWatchId)) {
          announcedAudioCaps.current.add(disposition.activeWatchId);
          addNotice('AUDIO_CAP_REACHED', 'ALERT', disposition.sectionKey, `${disposition.audibleCount}/${disposition.maxAudible}`);
        }
      } else if (disposition.disposition === 'CONTINUOUS_MIXER_ACTIVE') {
        continuousEpisodeIdsRef.current = disposition.episodeIds;
        setContinuousEpisodeIds(disposition.episodeIds);
        startContinuousAudio(volumeRef.current, mutedRef.current);
      } else if (disposition.disposition === 'CONTINUOUS_MIXER_STOPPED') {
        continuousEpisodeIdsRef.current = [];
        setContinuousEpisodeIds([]);
        audioController.stopContinuous();
      }
    }
  }, [addNotice, audioController, loadBatchStatus, send, setTelemetryFailure, startContinuousAudio]);

  useEffect(() => {
    const unsubscribeEvents = runtime.watch.subscribe(handleServerEvent);
    const unsubscribeState = runtime.watch.subscribeState((next) => {
      setConnection(next);
      if (next === 'OPEN') {
        hadConnection.current = true;
        sendQueuedStart();
      } else if (next === 'CLOSED' || next === 'ERROR') {
        const connectionOwnedState = hadConnection.current
          || queuedStart.current !== null
          || activeRef.current.length > 0;
        hadConnection.current = false;
        telemetryAbort.current?.abort();
        telemetryEpoch.current += 1;
        batchRequestRevisions.current.clear();
        sectionRequestRevisions.current.clear();
        telemetryFailures.current.clear();
        setTelemetryError(false);
        announcedAudioCaps.current.clear();
        queuedStart.current = null;
        pendingPolicies.current.clear();
        setPending([]);
        setActive([]);
        setObservations([]);
        setEpisodes([]);
        setAlerts([]);
        continuousEpisodeIdsRef.current = [];
        setContinuousEpisodeIds([]);
        audioController.stopContinuous();
        if (connectionOwnedState) addNotice('CONNECTION_LOST', 'ALERT');
      }
    });
    return () => {
      unsubscribeEvents();
      unsubscribeState();
    };
  }, [addNotice, audioController, handleServerEvent, runtime, sendQueuedStart]);

  useEffect(() => {
    audioProviderMounted.current = true;
    return () => {
      telemetryAbort.current?.abort();
      telemetryEpoch.current += 1;
      batchRequestRevisions.current.clear();
      sectionRequestRevisions.current.clear();
      telemetryFailures.current.clear();
      announcedAudioCaps.current.clear();
      audioProviderMounted.current = false;
      queueMicrotask(() => {
        if (!audioProviderMounted.current) audioController.dispose();
      });
    };
  }, [audioController]);

  const select = useCallback((sectionKey: SectionKey) => {
    if (selectedRef.current.some((value) => sameSection(value, sectionKey))) return;
    if (selectedRef.current.length >= MAX_SELECTED_SECTIONS) {
      addNotice('SELECTION_LIMIT', 'ALERT', sectionKey, String(MAX_SELECTED_SECTIONS));
      return;
    }
    const next = [...selectedRef.current, sectionKey];
    selectedRef.current = next;
    setSelected(next);
    onSelectedChange?.(next);
  }, [addNotice, onSelectedChange]);

  const remove = useCallback((sectionKey: SectionKey) => {
    if (activeRef.current.some((watch) => sameSection(watch.sectionKey, sectionKey))) return;
    const next = selectedRef.current.filter((value) => !sameSection(value, sectionKey));
    selectedRef.current = next;
    telemetryEpoch.current += 1;
    batchRequestRevisions.current.clear();
    sectionRequestRevisions.current.clear();
    telemetryFailures.current.clear();
    telemetryAbort.current?.abort();
    setTelemetryError(false);
    setSelected(next);
    onSelectedChange?.(next);
  }, [onSelectedChange]);

  const enableSound = useCallback(async () => {
    setAudioState('UNLOCKING');
    const state = await audioController.unlock();
    setAudioState(state);
    if (state === 'READY') {
      mutedRef.current = false;
      setMutedState(false);
      if (continuousEpisodeIdsRef.current.length > 0) {
        startContinuousAudio(volumeRef.current, false);
      }
    }
    if (state === 'BLOCKED') addNotice('AUDIO_BLOCKED', 'ALERT');
    if (state === 'FAILED') addNotice('AUDIO_FAILED', 'ALERT');
    return state;
  }, [addNotice, audioController, startContinuousAudio]);

  const testSound = useCallback(async () => {
    const unlocked = await enableSound();
    if (unlocked !== 'READY') return unlocked;
    const outcome = audioController.preview(volumeRef.current);
    if (outcome === 'AUTOPLAY_BLOCKED') {
      setAudioState('BLOCKED');
      addNotice('AUDIO_BLOCKED', 'ALERT');
      return 'BLOCKED';
    }
    if (outcome === 'FAILED') {
      setAudioState('FAILED');
      addNotice('AUDIO_FAILED', 'ALERT');
      return 'FAILED';
    }
    return 'READY';
  }, [addNotice, audioController, enableSound]);

  const startSelected = useCallback((policy: WatchPolicyV1) => {
    const inactive = selectedRef.current.filter((sectionKey) =>
      !activeRef.current.some((watch) => sameSection(watch.sectionKey, sectionKey)));
    if (inactive.length === 0) return;
    const items = inactive.map((sectionKey) => ({ sectionKey, policy }));
    for (const item of items) pendingPolicies.current.set(sectionIdentity(item.sectionKey), policy);
    queuedStart.current = items;
    setPending(inactive);
    if (audioState !== 'READY') void enableSound();
    try {
      if (runtime.watch.state === 'OPEN') sendQueuedStart();
      else runtime.watch.connect();
    } catch (error) {
      queuedStart.current = null;
      setPending([]);
      addNotice('COMMAND_FAILED', 'ALERT', undefined, error instanceof Error ? error.message : undefined);
    }
  }, [addNotice, audioState, enableSound, runtime, sendQueuedStart]);

  const stop = useCallback((watch: ActiveWatchView) => {
    send({ type: 'STOP_WATCH', watch: { activeWatchId: watch.activeWatchId, sectionKey: watch.sectionKey } });
  }, [send]);

  const updatePolicy = useCallback((watch: ActiveWatchView, policy: WatchPolicyV1) => {
    if (send({
      type: 'UPDATE_POLICY',
      watch: { activeWatchId: watch.activeWatchId, sectionKey: watch.sectionKey },
      policy,
    })) {
      announcedAudioCaps.current.delete(watch.activeWatchId);
      setActive((current) => current.map((value) =>
        value.activeWatchId === watch.activeWatchId ? { ...value, policy } : value));
    }
  }, [send]);

  const acknowledge = useCallback((episode: OpenEpisodeV1) => {
    send({
      type: 'ACKNOWLEDGE_EPISODE',
      episode: {
        activeWatchId: episode.activeWatchId,
        episodeId: episode.episodeId,
        sectionKey: episode.sectionKey,
      },
    });
  }, [send]);

  const acknowledgeAll = useCallback(() => {
    send({ type: 'ACKNOWLEDGE_ALL_EPISODES' });
  }, [send]);

  const resume = useCallback((episode: OpenEpisodeV1) => {
    send({
      type: 'RESUME_TIMED_OUT_EPISODE',
      episode: {
        activeWatchId: episode.activeWatchId,
        episodeId: episode.episodeId,
        sectionKey: episode.sectionKey,
      },
    });
  }, [send]);

  const resetAudibleCount = useCallback((watch: ActiveWatchView) => {
    if (send({
      type: 'RESET_AUDIBLE_COUNT',
      watch: { activeWatchId: watch.activeWatchId, sectionKey: watch.sectionKey },
    })) announcedAudioCaps.current.delete(watch.activeWatchId);
  }, [send]);

  const dismissAlert = useCallback((alert: WatchAlertV1) => {
    send({
      type: 'DISMISS_ALERT',
      alert: {
        activeWatchId: alert.episode.activeWatchId,
        alertId: alert.alertId,
        episodeId: alert.episode.episodeId,
        sectionKey: alert.episode.sectionKey,
      },
    });
    setAlerts((current) => current.filter((value) => value.alertId !== alert.alertId));
  }, [send]);

  const disconnect = useCallback(() => runtime.watch.disconnect(), [runtime]);
  const setMuted = useCallback((next: boolean) => {
    mutedRef.current = next;
    setMutedState(next);
    if (next) {
      audioController.stopContinuous();
    } else if (continuousEpisodeIdsRef.current.length > 0) {
      startContinuousAudio(volumeRef.current, false);
    }
  }, [audioController, startContinuousAudio]);
  const setVolume = useCallback((next: number) => {
    const normalized = Math.min(100, Math.max(0, next));
    volumeRef.current = normalized;
    setVolumeState(normalized);
    onVolumeChange?.(normalized);
    if (continuousEpisodeIdsRef.current.length > 0) {
      startContinuousAudio(normalized, mutedRef.current);
    }
  }, [onVolumeChange, startContinuousAudio]);

  const refreshTelemetry = useCallback(async () => {
    telemetryAbort.current?.abort();
    const epoch = telemetryEpoch.current + 1;
    telemetryEpoch.current = epoch;
    batchRequestRevisions.current.clear();
    sectionRequestRevisions.current.clear();
    telemetryFailures.current.clear();
    const keys = [...selectedRef.current];
    for (const watch of activeRef.current) {
      if (!keys.some((key) => sameSection(key, watch.sectionKey))) keys.push(watch.sectionKey);
    }
    if (keys.length === 0) {
      setBatchStatuses([]);
      setSectionStatuses([]);
      setTelemetryError(false);
      setTelemetryLoading(false);
      return;
    }
    const abort = new AbortController();
    telemetryAbort.current = abort;
    setTelemetryLoading(true);
    setTelemetryError(false);
    const batches = keys.filter((key, index) =>
      keys.findIndex((candidate) => sameBatch(candidate, key)) === index);
    const sectionTickets = keys.map((sectionKey) => {
      const identity = sectionIdentity(sectionKey);
      const revision = (sectionRequestRevisions.current.get(identity) ?? 0) + 1;
      sectionRequestRevisions.current.set(identity, revision);
      return { identity, revision, sectionKey };
    });
    const batchTickets = batches.map((batch) => {
      const identity = batchIdentity(batch);
      const revision = (batchRequestRevisions.current.get(identity) ?? 0) + 1;
      batchRequestRevisions.current.set(identity, revision);
      return { batch, identity, revision };
    });
    const [sectionResults, batchResults] = await Promise.all([
      Promise.allSettled(sectionTickets.map(({ sectionKey }) =>
        runtime.product.openSectionStatus({ contractVersion: 1, sectionKey }, abort.signal))),
      Promise.allSettled(batchTickets.map(({ batch }) =>
        runtime.product.openStatus({ contractVersion: 1, batch }, abort.signal))),
    ]);
    if (abort.signal.aborted || telemetryEpoch.current !== epoch) return;
    const relevantSections = new Set(keys.map(sectionIdentity));
    const relevantBatches = new Set(batches.map(batchIdentity));
    sectionResults.forEach((result, index) => {
      const ticket = sectionTickets[index]!;
      if (sectionRequestRevisions.current.get(ticket.identity) !== ticket.revision) return;
      const failureKey = sectionTelemetryFailureKey(ticket.sectionKey);
      if (result.status === 'rejected') telemetryFailures.current.add(failureKey);
      else telemetryFailures.current.delete(failureKey);
    });
    batchResults.forEach((result, index) => {
      const ticket = batchTickets[index]!;
      if (batchRequestRevisions.current.get(ticket.identity) !== ticket.revision) return;
      const failureKey = batchTelemetryFailureKey(ticket.batch);
      if (result.status === 'rejected') telemetryFailures.current.add(failureKey);
      else telemetryFailures.current.delete(failureKey);
    });
    setSectionStatuses((current) => {
      let next: readonly OpenSectionStatusV1[] = current.filter((status) =>
        relevantSections.has(sectionIdentity(status.sectionKey)));
      sectionResults.forEach((result, index) => {
        const ticket = sectionTickets[index]!;
        if (sectionRequestRevisions.current.get(ticket.identity) !== ticket.revision) return;
        if (result.status === 'rejected') return;
        next = replaceByIdentity(next, normalizeSectionStatus(result.value), (status) =>
          sectionIdentity(status.sectionKey));
      });
      return next;
    });
    setBatchStatuses((current) => {
      let next: readonly OpenRefreshStatusV1[] = current.filter((status) =>
        relevantBatches.has(batchIdentity(status.batch)));
      batchResults.forEach((result, index) => {
        const ticket = batchTickets[index]!;
        if (batchRequestRevisions.current.get(ticket.identity) !== ticket.revision) return;
        if (result.status === 'rejected') return;
        next = replaceByIdentity(next, normalizeBatchStatus(result.value), (status) =>
          batchIdentity(status.batch));
      });
      return next;
    });
    setTelemetryError(telemetryFailures.current.size > 0);
    setTelemetryLoading(false);
  }, [runtime]);

  const nextFreshnessExpiry = useMemo(() => {
    const now = Date.now();
    const expiries = [...sectionStatuses, ...batchStatuses]
      .map((status) => futureFreshnessExpiry(status.freshness, now))
      .filter((expiry): expiry is number => expiry !== null);
    return expiries.length === 0 ? null : Math.min(...expiries);
  }, [batchStatuses, sectionStatuses]);

  useEffect(() => {
    if (nextFreshnessExpiry === null) return undefined;
    let cancelled = false;
    let timer: ReturnType<typeof globalThis.setTimeout> | null = null;
    const schedule = () => {
      const delay = Math.min(
        MAX_BROWSER_TIMEOUT_MILLISECONDS,
        Math.max(0, nextFreshnessExpiry - Date.now() + 1),
      );
      timer = globalThis.setTimeout(() => {
        if (cancelled) return;
        const now = Date.now();
        if (now <= nextFreshnessExpiry) {
          schedule();
          return;
        }
        setSectionStatuses((current) => current.map((status) => normalizeSectionStatus(status, now)));
        setBatchStatuses((current) => current.map((status) => normalizeBatchStatus(status, now)));
        void refreshTelemetry();
      }, delay);
    };
    schedule();
    return () => {
      cancelled = true;
      if (timer !== null) globalThis.clearTimeout(timer);
    };
  }, [nextFreshnessExpiry, refreshTelemetry]);

  const telemetryKey = useMemo(() => selected.map(sectionIdentity).sort().join('|'), [selected]);
  useEffect(() => {
    if (telemetryKey.length === 0) {
      telemetryAbort.current?.abort();
      telemetryEpoch.current += 1;
      batchRequestRevisions.current.clear();
      sectionRequestRevisions.current.clear();
      telemetryFailures.current.clear();
      setBatchStatuses([]);
      setSectionStatuses([]);
      setTelemetryLoading(false);
      setTelemetryError(false);
      return;
    }
    void refreshTelemetry();
  }, [refreshTelemetry, telemetryKey]);

  useEffect(() => {
    if ((connection === 'CLOSED' || connection === 'ERROR') && selectedRef.current.length > 0) {
      void refreshTelemetry();
    }
  }, [connection, refreshTelemetry]);

  const value = useMemo<LiveWatchValue>(() => ({
    selected,
    pending,
    active,
    observations,
    episodes,
    alerts,
    notices,
    connection,
    audioState,
    muted,
    volume,
    continuousEpisodeIds,
    batchStatuses,
    sectionStatuses,
    telemetryLoading,
    telemetryError,
    isSelected: (sectionKey) => selected.some((value) => sameSection(value, sectionKey)),
    isActive: (sectionKey) => active.some((watch) => sameSection(watch.sectionKey, sectionKey)),
    select,
    remove,
    startSelected,
    stop,
    updatePolicy,
    acknowledge,
    acknowledgeAll,
    resume,
    resetAudibleCount,
    dismissAlert,
    dismissNotice: (id) => setNotices((current) => current.filter((notice) => notice.id !== id)),
    disconnect,
    enableSound,
    testSound,
    setMuted,
    setVolume,
    refreshTelemetry,
  }), [
    acknowledge,
    acknowledgeAll,
    active,
    alerts,
    audioState,
    batchStatuses,
    connection,
    continuousEpisodeIds,
    disconnect,
    dismissAlert,
    enableSound,
    episodes,
    muted,
    notices,
    observations,
    pending,
    refreshTelemetry,
    remove,
    resetAudibleCount,
    resume,
    sectionStatuses,
    select,
    selected,
    setMuted,
    setVolume,
    startSelected,
    stop,
    telemetryError,
    telemetryLoading,
    testSound,
    updatePolicy,
    volume,
  ]);

  return <LiveWatchContext.Provider value={value}>{children}</LiveWatchContext.Provider>;
}

export function useLiveWatch(): LiveWatchValue {
  const value = useContext(LiveWatchContext);
  if (value === null) throw new Error('useLiveWatch must be used within LiveWatchProvider');
  return value;
}

export function useLiveWatchOptional(): LiveWatchValue | null {
  return useContext(LiveWatchContext);
}
