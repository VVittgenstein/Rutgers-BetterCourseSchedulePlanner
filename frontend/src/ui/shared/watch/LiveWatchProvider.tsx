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
import { ProductClientError } from '../product';
import {
  WatchAudioController,
  type WatchAudioUnlockResult,
} from './audio';

import {
  findIntentEntry,
  watchIntentState,
  type WatchIntentPort,
  type WatchIntentSnapshot,
  type WatchIntentState,
  type WatchIntentStatus,
} from './intent';

export type WatchAudioState = WatchAudioUnlockResult | 'MUTED' | 'UNLOCKING';

/** One submission inside a batch gesture. `policy === null` asks it to stop. */
export interface WatchIntentBatchItem {
  readonly sectionKey: SectionKey;
  readonly policy: WatchPolicyV1 | null;
}

export const MAX_SELECTED_SECTIONS = 9;
export const DEFAULT_WATCH_POLICY: WatchPolicyV1 = {
  notificationMode: 'ONE_SHOT',
  maxAudible: 3,
  continuousDuration: { kind: 'FINITE', seconds: 600 },
};

export interface ActiveWatchView {
  readonly activeWatchId: ActiveWatchId;
  readonly sectionKey: SectionKey;
  /**
   * `null` when this view was derived from the server's standing intent
   * rather than from the START frame that created the watch.
   *
   * A page that joins after a watch has started never receives that frame,
   * and the authority read does not carry a start time -- so the honest
   * answer is "this is running and here is how to address it", not a
   * timestamp invented to fill the field.
   */
  readonly startedAt: string | null;
  readonly policy: WatchPolicyV1;
}

export type WatchTelemetryResourceKind = 'SECTION' | 'BATCH';
export type WatchTelemetryResourceAvailability = 'CURRENT' | 'LKG' | 'ERROR_NO_DATA';

export interface WatchTelemetryResourceError {
  readonly httpStatus: number | null;
  readonly apiCode: string | null;
  readonly traceId: string | null;
}

/** Per-resource REST evidence. WebSocket and audio state are intentionally independent. */
export interface WatchTelemetryResourceState {
  readonly key: string;
  readonly kind: WatchTelemetryResourceKind;
  readonly sectionKey: SectionKey | null;
  readonly batch: { readonly term: string; readonly campus: string } | null;
  readonly availability: WatchTelemetryResourceAvailability;
  readonly loading: boolean;
  readonly lastSuccessAt: string | null;
  readonly error: WatchTelemetryResourceError | null;
}

export type WatchNoticeCode =
  | 'SELECTION_LIMIT'
  | 'TERM_OUT_OF_RANGE'
  | 'START_REJECTED'
  | 'COMMAND_FAILED'
  | 'CONNECTION_LOST'
  | 'WATCH_STOPPED'
  | 'AUDIO_BLOCKED'
  | 'AUDIO_FAILED'
  | 'AUDIO_CAP_REACHED'
  | 'AUDIO_CUE_QUEUED'
  | 'WATCH_ALERT_OPEN'
  /** A selection could not be removed, because doing so would hide a watch. */
  | 'SELECTION_BLOCKED';

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
  readonly telemetryResources: readonly WatchTelemetryResourceState[];
  readonly telemetryLoading: boolean;
  readonly starting: boolean;
  /**
   * The server's standing watch intent, when this target keeps any.
   *
   * `DISABLED` says the target has no durable intent at all, which is a
   * different statement from "there is none right now" -- the Public build
   * genuinely cannot have any. `FAILED` says the page could not read it, and
   * the workspace must then show that rather than the last good answer: a
   * green light left over from a successful read minutes ago is exactly the
   * comfortable lie this surface exists to avoid.
   */
  readonly intentStatus: WatchIntentStatus;
  readonly intent: WatchIntentSnapshot | null;
  /**
   * The Sections the last TRUSTED read said the server holds intent for.
   *
   * Identity only, and deliberately kept when a read fails. It is what stops
   * a failed read from taking a Section the user never selected off the desk
   * entirely -- which would remove the only STOP control for a watch that may
   * still be running. Nothing about it is a claim that anything is running:
   * while `intentStatus` is not `READY` the row shows as unavailable and
   * nothing on it may be submitted.
   */
  readonly intentSaved: readonly SectionKey[];
  intentStateFor(sectionKey: SectionKey): WatchIntentState | null;
  /**
   * Asks the server to watch a section with `policy`, or to stop watching it
   * with `null`.
   *
   * Addressed by SECTION rather than by a running watch id, because the
   * intent exists whether or not anything is running for it -- a section the
   * runtime cannot arm today must still be stoppable.
   *
   * A no-op returning `false` on a target without durable intent.
   */
  setSectionIntent(sectionKey: SectionKey, policy: WatchPolicyV1 | null): Promise<boolean>;
  /**
   * Runs one BATCH gesture -- one press of Start selected, one press of Apply
   * policy -- against a single immutable basis.
   *
   * `plan` is handed the snapshot captured when the user pressed, and every
   * submission the batch makes is compared against that one: the generation
   * it was showing, and each Section's own revision from it. Capturing per
   * item instead is what turns a batch into a silent overwrite -- the second
   * item picks up whatever the first item's answer returned, so a change
   * another tab made to it, or a rotation the first item triggered, is
   * applied over rather than refused. A refusal ends the batch; nothing is
   * ever resubmitted.
   */
  setSectionIntentBatch(
    plan: (basis: WatchIntentSnapshot) => readonly WatchIntentBatchItem[],
  ): Promise<void>;
  refreshIntent(): Promise<void>;
  isSelected(sectionKey: SectionKey): boolean;
  isActive(sectionKey: SectionKey): boolean;
  /**
   * Whether this section may be taken off the managed list right now.
   *
   * `false` while the server still wants it, while its teardown is still
   * running, and -- fail closed -- whenever the standing intent could not be
   * read at all. This list is the only place a watch can be stopped from, so
   * removing a row from it while something is still watching leaves a watch
   * that keeps polling and keeps ringing with nothing left to press.
   */
  isRemovable(sectionKey: SectionKey): boolean;
  isWatchable(sectionKey: SectionKey): boolean;
  updateWatchableTerms(terms: readonly string[]): void;
  select(sectionKey: SectionKey): void;
  remove(sectionKey: SectionKey): void;
  startSelected(policy: WatchPolicyV1): Promise<void>;
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
  retryTelemetryResource(key: string): Promise<void>;
}

/**
 * What kind of question an authority answer is the answer to.
 *
 * `READ` is this page asking the server what is running, right now, over the
 * connection it holds. `MUTATION` is the server reporting what one write
 * left behind. The page applies both -- they carry the same snapshot shape,
 * and either can settle a Section whose outcome was unknown -- but only a
 * `READ` is evidence about the CONNECTION, and the socket-wide cutoff is
 * lifted by nothing else.
 */
type WatchAuthorityEvidence = 'READ' | 'MUTATION';

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

function sectionTelemetryTarget(sectionKey: SectionKey): WatchTelemetryResourceState {
  return {
    key: sectionTelemetryFailureKey(sectionKey),
    kind: 'SECTION',
    sectionKey,
    batch: null,
    availability: 'ERROR_NO_DATA',
    loading: false,
    lastSuccessAt: null,
    error: null,
  };
}

function batchTelemetryTarget(
  batch: { readonly term: string; readonly campus: string },
): WatchTelemetryResourceState {
  return {
    key: batchTelemetryFailureKey(batch),
    kind: 'BATCH',
    sectionKey: null,
    batch,
    availability: 'ERROR_NO_DATA',
    loading: false,
    lastSuccessAt: null,
    error: null,
  };
}

function telemetryError(error: unknown): WatchTelemetryResourceError {
  if (error instanceof ProductClientError) {
    return {
      httpStatus: error.status,
      apiCode: error.apiError?.error.code ?? null,
      traceId: error.apiError?.error.traceId ?? null,
    };
  }
  return { httpStatus: null, apiCode: null, traceId: null };
}

function sectionLastSuccessAt(status: OpenSectionStatusV1): string | null {
  return status.freshness.observedAt;
}

function batchLastSuccessAt(status: OpenRefreshStatusV1): string | null {
  return status.lastValidObservation?.observedAt
    ?? status.freshness.observedAt;
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
  /**
   * Supplied by a target that keeps standing watch intent on the server.
   *
   * When it is present, starting and stopping a watch is a change to that
   * intent rather than a command on the socket, and the socket stops being
   * able to start or stop anything. Two sources of truth for "is this
   * watched" would disagree the first time one of them was used.
   */
  readonly intent?: WatchIntentPort | undefined;
  readonly initialSelected?: readonly SectionKey[] | undefined;
  readonly initialWatchableTerms?: readonly string[] | undefined;
  readonly initialVolume?: number | undefined;
  readonly onSelectedChange?: ((selected: readonly SectionKey[]) => void) | undefined;
  readonly onVolumeChange?: ((volume: number) => void) | undefined;
}

export function LiveWatchProvider({
  audio,
  children,
  intent,
  initialSelected = [],
  initialWatchableTerms,
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
  const [telemetryResources, setTelemetryResources] = useState<
    readonly WatchTelemetryResourceState[]
  >([]);
  const [telemetryLoading, setTelemetryLoading] = useState(false);
  const [starting, setStarting] = useState(false);
  const [intentSnapshot, setIntentSnapshot] = useState<WatchIntentSnapshot | null>(null);
  const [intentStatus, setIntentStatus] = useState<WatchIntentStatus>(
    intent === undefined ? 'DISABLED' : 'LOADING',
  );
  /**
   * Sections this page has direct physical evidence are no longer running.
   *
   * Kept apart from the snapshot rather than folded into it, because they are
   * different KINDS of statement and one has to be able to overrule the
   * other. The snapshot is the server's answer; this is what this page saw
   * happen since. A later answer clears it -- see `applyAuthority`.
   */
  const [disproved, setDisproved] = useState<ReadonlySet<string>>(() => new Set());
  /**
   * The Sections the last TRUSTED read said the server was managing.
   *
   * Kept across a failed read on purpose. A failed read means "this page can
   * no longer say what is running", not "the user never asked for anything":
   * dropping these takes the desk row -- and with it the only STOP control --
   * off the screen for exactly the sections the user did not put in their
   * selection. The row comes back as unavailable rather than green, and
   * nothing on it may be submitted until a read succeeds again.
   *
   * "Managing" is deliberately wider than "wants watched". A tombstone whose
   * teardown is still running, and a row the server still reports something
   * materialized for, both name a physical watch that may still be alive --
   * so a STOP fault followed by a failed read must not be able to hide one.
   */
  const [trustedIdentities, setTrustedIdentities] = useState<readonly SectionKey[]>([]);
  /**
   * Sections with a mutation whose outcome this page does not know.
   *
   * A submission is sent and then anything can happen to the answer: the
   * response is lost, the body does not decode, the authority refuses and the
   * re-read hangs. In every one of those the server may or may not have acted
   * -- and the one thing the page must not do is go on offering the state it
   * held before as if nothing had been asked. So the Section loses its green
   * light, its Remove button and its right to submit again until an authority
   * answer issued no earlier than the mutation itself comes back.
   *
   * It is also an IDENTITY. A START whose response was lost may have armed a
   * watch the previous snapshot knows nothing about, and a row that is not on
   * the desk is a watch with no control at all.
   */
  const [uncertain, setUncertain] = useState<readonly SectionKey[]>([]);
  /**
   * True while this page cannot assert that ANY watch is running.
   *
   * The socket it was watching through closed. Section-by-section evidence
   * cannot express that: a read issued before the close can land after it,
   * carrying rows the server believed were running at the time, and if the
   * page had nothing running when the socket went -- a snapshot that was
   * still null, or every row still preparing -- there is no per-Section
   * evidence for that answer to lose against. It would simply light up. So
   * the close is recorded as a cutoff over ALL authority answers, and only a
   * successful read issued after the socket is open again can lift it.
   */
  const [socketCutoff, setSocketCutoff] = useState(false);
  const [watchableTerms, setWatchableTerms] = useState<ReadonlySet<string> | null>(() =>
    initialWatchableTerms === undefined ? null : new Set(initialWatchableTerms));
  /**
   * What is running, as the SERVER reports it.
   *
   * With durable intent this replaces the list built from START frames,
   * because that list is only ever as complete as this page's own history: a
   * page that joins after the watches were armed receives no START frame and
   * would show zero active watches -- an active count of 0, a connection line
   * that says nothing is being watched, no audio warning when sound is
   * blocked, and no way to reach the controls for an episode it can see. The
   * authority read is the same answer for every page, whenever it arrived.
   */
  /**
   * The authority read, with this page's own physical evidence applied.
   *
   * An authority snapshot is the server's answer as of when it was taken, and
   * a `WATCH_STOPPED` frame or a socket that closed is newer, first-hand
   * evidence that the answer is no longer true. Waiting for the next GET to
   * agree is exactly how a page keeps a green light lit for a watch it has
   * just been told ended -- and a GET can hang.
   *
   * Only the RUNNING half is disproved. Which sections the user wants watched
   * is not something a stopped watch says anything about, and the rows are
   * what carry the STOP control.
   */
  const uncertainKeys = useMemo(
    () => new Set(uncertain.map(sectionIdentity)),
    [uncertain],
  );
  const disprovedSnapshot = useMemo<WatchIntentSnapshot | null>(() => {
    if (intentSnapshot === null) return null;
    if (!socketCutoff && disproved.size === 0 && uncertainKeys.size === 0) return intentSnapshot;
    return {
      ...intentSnapshot,
      entries: intentSnapshot.entries.map((entry) => {
        if (entry.running === null) return entry;
        if (socketCutoff) return { ...entry, running: null };
        const identity = sectionIdentity(entry.section);
        return disproved.has(identity) || uncertainKeys.has(identity)
          ? { ...entry, running: null }
          : entry;
      }),
    };
  }, [disproved, intentSnapshot, socketCutoff, uncertainKeys]);
  /**
   * Every Section this page must keep a control for, whatever it can read.
   *
   * The union of what the last trusted read was managing and what this page
   * has an unsettled mutation for. The second half is not caution: a START
   * whose response was lost may already be watching, and the snapshot that
   * predates it says nothing about the Section at all.
   */
  const intentSaved = useMemo<readonly SectionKey[]>(() => {
    if (uncertain.length === 0) return trustedIdentities;
    const rows = [...trustedIdentities];
    const known = new Set(rows.map(sectionIdentity));
    for (const sectionKey of uncertain) {
      const identity = sectionIdentity(sectionKey);
      if (known.has(identity)) continue;
      known.add(identity);
      rows.push(sectionKey);
    }
    return rows;
  }, [trustedIdentities, uncertain]);
  const intentActive = useMemo<readonly ActiveWatchView[]>(() => {
    if (intent === undefined || disprovedSnapshot === null) return [];
    return disprovedSnapshot.entries.flatMap((entry) => entry.running === null ? [] : [{
      activeWatchId: entry.running.activeWatchId,
      sectionKey: entry.section,
      startedAt: null,
      policy: entry.running.policy,
    }]);
  }, [disprovedSnapshot, intent]);
  const effectiveActive = intent === undefined ? active : intentActive;
  const selectedRef = useRef(selected);
  const activeRef = useRef(active);
  const pendingRef = useRef(pending);
  const audioStateRef = useRef<WatchAudioState>(audioState);
  const mutedRef = useRef(muted);
  const volumeRef = useRef(volume);
  const telemetryResourcesRef = useRef(telemetryResources);
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
  const audioProviderMounted = useRef(false);
  const startAttempt = useRef(0);
  const startingRef = useRef(false);
  const watchableTermsRef = useRef(watchableTerms);
  const intentSnapshotRef = useRef(intentSnapshot);
  const intentStatusRef = useRef(intentStatus);
  const intentConnection = useRef<WatchConnectionState | null>(null);
  // ONE ordered domain for every authority answer.
  //
  // A GET issued before a STOP can return after it. Applied as it arrived,
  // its full snapshot puts the section back to WATCHING -- the page shows a
  // green light for intent the user has already cancelled, and the tombstone
  // that proves they cancelled it is the thing that got overwritten. Two
  // writes to different sections have the same problem in the other
  // direction. So authority operations run one at a time, in the order they
  // were asked for, and an answer from an earlier operation can never be
  // applied over a later one's.
  const authorityQueue = useRef<Promise<unknown>>(Promise.resolve());
  const authorityIssued = useRef(0);
  const authorityApplied = useRef(0);
  /**
   * Each disproof, against the operation counter at the moment it arrived.
   *
   * A read that was already in flight when the watch stopped is not newer
   * information about it, however late it lands, so only an answer to an
   * operation issued AFTER the evidence may clear it.
   */
  const disprovedAt = useRef(new Map<string, number>());
  /**
   * Each unsettled mutation, against the operation counter it was sent under.
   *
   * Cleared by an authority answer from that operation or a later one, and by
   * nothing else: an answer to a read that was already in flight when the
   * mutation went out describes the world before it, so treating it as the
   * outcome would restore exactly the state the mutation may have replaced.
   */
  const uncertainAt = useRef(new Map<string, { readonly at: number; readonly section: SectionKey }>());
  /**
   * The operation counter at the moment the socket last reached OPEN, while a
   * cutoff stands.
   *
   * `null` means the cutoff cannot be lifted yet -- the socket is not open, so
   * no read that could lift it has been issued. Once it is a number, only an
   * answer to an operation issued AFTER it counts: a read that was in flight
   * across the close is not evidence about the connection that replaced it.
   */
  const cutoffReleaseAt = useRef<number | null>(null);
  const socketCutoffRef = useRef(false);

  useEffect(() => { intentSnapshotRef.current = intentSnapshot; }, [intentSnapshot]);
  useEffect(() => { intentStatusRef.current = intentStatus; }, [intentStatus]);
  useEffect(() => { selectedRef.current = selected; }, [selected]);
  useEffect(() => { activeRef.current = effectiveActive; }, [effectiveActive]);
  useEffect(() => { pendingRef.current = pending; }, [pending]);
  useEffect(() => { audioStateRef.current = audioState; }, [audioState]);
  useEffect(() => { mutedRef.current = muted; }, [muted]);
  useEffect(() => { volumeRef.current = volume; }, [volume]);
  useEffect(() => { telemetryResourcesRef.current = telemetryResources; }, [telemetryResources]);
  useEffect(() => { continuousEpisodeIdsRef.current = continuousEpisodeIds; }, [continuousEpisodeIds]);
  useEffect(() => { watchableTermsRef.current = watchableTerms; }, [watchableTerms]);

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

  const updateTelemetryResource = useCallback((
    target: WatchTelemetryResourceState,
    update: (current: WatchTelemetryResourceState) => WatchTelemetryResourceState,
  ) => {
    setTelemetryResources((current) => {
      const existing = current.find((resource) => resource.key === target.key) ?? target;
      const nextResource = update(existing);
      const next = replaceByIdentity(current, nextResource, (resource) => resource.key);
      telemetryResourcesRef.current = next;
      return next;
    });
  }, []);

  const markTelemetryLoading = useCallback((target: WatchTelemetryResourceState) => {
    updateTelemetryResource(target, (current) => ({ ...current, loading: true }));
  }, [updateTelemetryResource]);

  const markTelemetrySuccess = useCallback((
    target: WatchTelemetryResourceState,
    freshness: OpenFreshnessV1,
    lastSuccessAt: string | null,
  ) => {
    updateTelemetryResource(target, (current) => ({
      ...current,
      availability: lastSuccessAt === null || freshness.state === 'UNKNOWN'
        ? 'ERROR_NO_DATA'
        : freshness.state === 'STALE'
          ? 'LKG'
          : 'CURRENT',
      loading: false,
      lastSuccessAt,
      error: null,
    }));
  }, [updateTelemetryResource]);

  const markTelemetryFailure = useCallback((
    target: WatchTelemetryResourceState,
    error: unknown,
  ) => {
    updateTelemetryResource(target, (current) => ({
      ...current,
      availability: current.lastSuccessAt === null ? 'ERROR_NO_DATA' : 'LKG',
      loading: false,
      error: telemetryError(error),
    }));
  }, [updateTelemetryResource]);

  /**
   * Runs one authority operation in the single ordered domain.
   *
   * The domain orders ANSWERS. It does not re-base gestures: what a
   * submission compares against is captured when the user clicks, not when
   * the operation reaches the front of the queue. Reading the latest snapshot
   * here instead would quietly apply each gesture to whatever the previous
   * one produced -- so a STOP still in flight, followed by a policy change on
   * the same section, would submit the second against the tombstone the first
   * had just written and start the watch the user had just stopped.
   */
  const runAuthority = useCallback(<T,>(
    operation: (sequence: number) => Promise<T>,
  ): Promise<T> => {
    authorityIssued.current += 1;
    const sequence = authorityIssued.current;
    const run = authorityQueue.current.then(
      () => operation(sequence),
      () => operation(sequence),
    );
    authorityQueue.current = run.then(() => undefined, () => undefined);
    return run;
  }, []);

  /**
   * Applies one authority answer, unless a later one has already landed.
   *
   * `evidence` is what the answer IS, not merely where it came from. A read
   * is this page asking the server what is running now; a mutation answer is
   * the server reporting what one write left behind, and it is issued
   * whether or not this page has a connection to hear an alert over. Both
   * carry an authority snapshot and both may settle uncertainty, but only
   * the first is evidence about the connection -- see the cutoff below.
   */
  const applyAuthority = useCallback((
    sequence: number,
    snapshot: WatchIntentSnapshot | null,
    status: WatchIntentStatus,
    evidence: WatchAuthorityEvidence,
  ) => {
    if (sequence < authorityApplied.current) return;
    authorityApplied.current = sequence;
    intentSnapshotRef.current = snapshot;
    intentStatusRef.current = status;
    setIntentSnapshot(snapshot);
    setIntentStatus(status);
    if (status !== 'READY' || snapshot === null) return;
    // A successful read is the newer answer for everything this page saw stop
    // BEFORE it was asked for. Evidence from after it stands.
    let cleared = false;
    for (const [identity, at] of [...disprovedAt.current]) {
      if (sequence <= at) continue;
      disprovedAt.current.delete(identity);
      cleared = true;
    }
    if (cleared) setDisproved(new Set(disprovedAt.current.keys()));
    // A mutation's own answer settles it, and so does any later answer.
    // Anything issued EARLIER describes the world before the submission and
    // says nothing about whether it took effect.
    let settled = false;
    for (const [identity, pending] of [...uncertainAt.current]) {
      if (sequence < pending.at) continue;
      uncertainAt.current.delete(identity);
      settled = true;
    }
    if (settled) {
      setUncertain([...uncertainAt.current.values()].map((pending) => pending.section));
    }
    // The socket-wide cutoff is lifted only by a READ this page asked for
    // after the socket was open again. Two things it is not. A read that
    // crossed the close, however late it lands, is an answer about a
    // connection that no longer exists. And a mutation's answer is not this
    // evidence at all, whenever it was issued: a PUT says what the authority
    // now holds, which is a statement about the server's intent and not about
    // whether THIS page can hear the watch it describes. Letting a write
    // relight the desk would mean a user who pressed a button over a dead
    // socket got a green light for it.
    if (
      evidence === 'READ'
      && socketCutoffRef.current
      && cutoffReleaseAt.current !== null
      && sequence > cutoffReleaseAt.current
    ) {
      socketCutoffRef.current = false;
      cutoffReleaseAt.current = null;
      setSocketCutoff(false);
    }
    // Every row that could still name a physical watch, not only the ones the
    // user currently wants: a teardown in progress and a materialization the
    // server still reports both outlive `policy`.
    setTrustedIdentities(snapshot.entries.flatMap((entry) =>
      entry.policy === null && !entry.stopping && entry.running === null
        ? []
        : [entry.section]));
  }, []);

  /**
   * Whether this Section has a submission whose outcome is not known.
   *
   * The ONE authority every gate asks, and it is the map `markUncertain`
   * writes rather than anything derived from it by a render. `uncertain` --
   * the state, and everything memoised from it -- is how the answer is
   * DISPLAYED, and it necessarily lags by a render; a gate that read the
   * displayed copy would be answering about the page as it was drawn before
   * the request went out, which is exactly the moment a second gesture is
   * made. Two sources here means two answers, and the one the gates got was
   * the stale one.
   */
  const isUncertain = useCallback(
    (section: SectionKey): boolean => uncertainAt.current.has(sectionIdentity(section)),
    [],
  );

  /**
   * Records that a submission for this Section is in flight and its outcome
   * is not known.
   *
   * Called as the operation reaches the front of the queue and immediately
   * before the request goes out, so the page stops asserting the pre-gesture
   * state from the moment the gesture is actually asked. The map is written
   * SYNCHRONOUSLY here, and that is what the gates read: from this line on,
   * within the same turn, the Section cannot be submitted again or removed.
   */
  const markUncertain = useCallback((section: SectionKey, sequence: number) => {
    uncertainAt.current.set(sectionIdentity(section), { at: sequence, section });
    setUncertain([...uncertainAt.current.values()].map((pending) => pending.section));
  }, []);

  /**
   * Records first-hand evidence that a named Section is no longer being
   * watched.
   *
   * Per-Section, and only per-Section. Losing the socket entirely is not a
   * longer list of Sections -- there may be none to list -- so that is
   * recorded as a cutoff over every answer instead. See `socketCutoff`.
   */
  const disprove = useCallback((sections: readonly SectionKey[]) => {
    const identities = sections.map(sectionIdentity);
    if (identities.length === 0) return;
    for (const identity of identities) disprovedAt.current.set(identity, authorityIssued.current);
    setDisproved(new Set(disprovedAt.current.keys()));
  }, []);

  /**
   * Re-reads the server's intent.
   *
   * A failure clears the snapshot rather than keeping the last good one. The
   * whole value of this projection is that it is the server's current answer;
   * a stale copy would still render a green light, and the user would have no
   * way to tell it apart from a live one.
   */
  const refreshIntent = useCallback(async () => {
    if (intent === undefined) return;
    setIntentStatus((current) => (current === 'READY' ? current : 'LOADING'));
    await runAuthority(async (sequence) => {
      try {
        applyAuthority(sequence, await intent.read(), 'READY', 'READ');
      } catch {
        applyAuthority(sequence, null, 'FAILED', 'READ');
        addNotice('COMMAND_FAILED', 'ALERT');
      }
    });
  }, [addNotice, applyAuthority, intent, runAuthority]);

  /**
   * Submits one intent change against the snapshot the page was showing when
   * the user made the gesture.
   *
   * The snapshot is captured HERE, synchronously, and not re-read when the
   * operation runs. That is the whole contract of `WatchIntentPort.submit`: a
   * compare-and-swap is only a safeguard if it compares against the state the
   * user was actually looking at. Re-basing onto whatever the queue has since
   * produced turns "refuse this, the world moved" into "apply it anyway",
   * which is how a STOP and a policy change clicked in that order end with
   * the section watched again.
   *
   * A conflict re-reads and stops. It never resubmits: the revision the page
   * held is stale precisely because someone changed the same section, and
   * replaying the user's gesture against the new state would apply a decision
   * they made about a state that no longer exists.
   */
  const submitAgainst = useCallback(async (
    section: SectionKey,
    policy: WatchPolicyV1 | null,
    basis: WatchIntentSnapshot,
  ): Promise<boolean> => {
    if (intent === undefined) return false;
    // A Section whose last submission has not been answered may not be
    // submitted again. The page does not know what the server did with the
    // first one, so the second would be a decision made about a state nobody
    // can vouch for -- and re-sending the first is exactly the automatic
    // replay this surface must never do.
    //
    // Asked of `uncertainAt`, which `markUncertain` writes as the request
    // goes out, and not of anything a render produces from it. The whole
    // window this gate exists for is the one BEFORE React has re-rendered:
    // two gestures inside one turn -- a second press, a Remove -- reach a
    // mirror updated by an effect while it still says the section is free,
    // and the second PUT goes out against a state the first may already have
    // replaced.
    if (isUncertain(section)) {
      addNotice('COMMAND_FAILED', 'ALERT', section);
      return false;
    }
    const settled = await runAuthority(async (sequence) => {
      markUncertain(section, sequence);
      try {
        const result = await intent.submit({ section, policy }, basis);
        // The answer updates the authority the page renders and settles this
        // Section, and does not lift the connection cutoff.
        if (result.snapshot !== null) {
          applyAuthority(sequence, result.snapshot, 'READY', 'MUTATION');
        }
        if (result.outcome === 'COMMITTED') return 'COMMITTED' as const;
        addNotice(
          result.outcome === 'AT_CAPACITY'
            ? 'SELECTION_LIMIT'
            : result.outcome === 'CONFLICT'
              ? 'START_REJECTED'
              : 'COMMAND_FAILED',
          'ALERT',
          section,
        );
        return result.snapshot === null ? 'REREAD' as const : 'REFUSED' as const;
      } catch {
        addNotice('COMMAND_FAILED', 'ALERT', section);
        return 'REREAD' as const;
      }
    });
    // The re-read is its own operation in the same domain, so it can never
    // land before the answer that asked for it -- and it is never the user's
    // gesture sent again. The revision they held is stale precisely because
    // the state changed. It is also what settles the Section: until it comes
    // back, the row stays unusable rather than falling back to what the page
    // held before the gesture.
    if (settled === 'REREAD') await refreshIntent();
    return settled === 'COMMITTED';
  }, [addNotice, applyAuthority, intent, isUncertain, markUncertain, refreshIntent, runAuthority]);

  /**
   * The snapshot a gesture made right now compares against, or `null` when
   * there is nothing trustworthy to compare against at all.
   */
  const gestureBasis = useCallback((): WatchIntentSnapshot | null => {
    if (intentStatusRef.current !== 'READY') return null;
    return intentSnapshotRef.current;
  }, []);

  const submitIntent = useCallback(async (
    section: SectionKey,
    policy: WatchPolicyV1 | null,
  ): Promise<boolean> => {
    if (intent === undefined) return false;
    const basis = gestureBasis();
    // Nothing trustworthy to compare against, so there is nothing to submit:
    // re-read and let the user decide again against a state they can see.
    if (basis === null) {
      await refreshIntent();
      return false;
    }
    return await submitAgainst(section, policy, basis);
  }, [gestureBasis, intent, refreshIntent, submitAgainst]);

  /**
   * Runs several submissions against ONE basis, in order, stopping at the
   * first that is not committed.
   *
   * Sequential because the alternative races the batch against itself -- two
   * writes to the same authority in flight at once, one of which must lose.
   * One basis because the whole point of a batch is that the user pressed
   * once, about one screen: an item that compared against what the item
   * before it produced would be applying the user's decision to a state they
   * never saw, and would overwrite a change another tab made to it instead of
   * being refused.
   */
  const runIntentBatch = useCallback(async (
    basis: WatchIntentSnapshot,
    items: readonly WatchIntentBatchItem[],
  ) => {
    for (const item of items) {
      if (!await submitAgainst(item.sectionKey, item.policy, basis)) break;
    }
  }, [submitAgainst]);

  const submitIntentBatch = useCallback(async (
    plan: (basis: WatchIntentSnapshot) => readonly WatchIntentBatchItem[],
  ) => {
    if (intent === undefined) return;
    const basis = gestureBasis();
    if (basis === null) {
      await refreshIntent();
      return;
    }
    await runIntentBatch(basis, plan(basis));
  }, [gestureBasis, intent, refreshIntent, runIntentBatch]);

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
    const sent = send({ type: 'START_WATCH', items });
    queuedStart.current = null;
    if (sent) return;
    const failedKeys = new Set(items.map((item) => sectionIdentity(item.sectionKey)));
    for (const key of failedKeys) pendingPolicies.current.delete(key);
    setPending((current) => {
      const next = current.filter((key) => !failedKeys.has(sectionIdentity(key)));
      pendingRef.current = next;
      return next;
    });
  }, [runtime, send]);

  const loadBatchStatus = useCallback(async (
    batch: { readonly term: string; readonly campus: string },
  ) => {
    const target = batchTelemetryTarget(batch);
    const identity = batchIdentity(batch);
    const epoch = telemetryEpoch.current;
    const revision = (batchRequestRevisions.current.get(identity) ?? 0) + 1;
    batchRequestRevisions.current.set(identity, revision);
    markTelemetryLoading(target);
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
      markTelemetrySuccess(target, status.freshness, batchLastSuccessAt(status));
    } catch (error) {
      const isCurrent = telemetryEpoch.current === epoch
        && batchRequestRevisions.current.get(identity) === revision;
      const isRelevant = selectedRef.current.some((section) => sameBatch(section, batch))
        || activeRef.current.some((watch) => sameBatch(watch.sectionKey, batch));
      if (isCurrent && isRelevant) markTelemetryFailure(target, error);
    }
  }, [markTelemetryFailure, markTelemetryLoading, markTelemetrySuccess, runtime]);

  const loadSectionStatus = useCallback(async (sectionKey: SectionKey) => {
    const target = sectionTelemetryTarget(sectionKey);
    const identity = sectionIdentity(sectionKey);
    const epoch = telemetryEpoch.current;
    const revision = (sectionRequestRevisions.current.get(identity) ?? 0) + 1;
    sectionRequestRevisions.current.set(identity, revision);
    markTelemetryLoading(target);
    try {
      const response = await runtime.product.openSectionStatus({ contractVersion: 1, sectionKey });
      const isCurrent = telemetryEpoch.current === epoch
        && sectionRequestRevisions.current.get(identity) === revision;
      const isRelevant = selectedRef.current.some((section) => sameSection(section, sectionKey))
        || activeRef.current.some((watch) => sameSection(watch.sectionKey, sectionKey));
      if (!isCurrent || !isRelevant) return;
      const status = normalizeSectionStatus(response);
      setSectionStatuses((current) => replaceByIdentity(
        current,
        status,
        (value) => sectionIdentity(value.sectionKey),
      ));
      markTelemetrySuccess(target, status.freshness, sectionLastSuccessAt(status));
    } catch (error) {
      const isCurrent = telemetryEpoch.current === epoch
        && sectionRequestRevisions.current.get(identity) === revision;
      const isRelevant = selectedRef.current.some((section) => sameSection(section, sectionKey))
        || activeRef.current.some((watch) => sameSection(watch.sectionKey, sectionKey));
      if (isCurrent && isRelevant) markTelemetryFailure(target, error);
    }
  }, [markTelemetryFailure, markTelemetryLoading, markTelemetrySuccess, runtime]);

  const startContinuousAudio = useCallback((nextVolume: number, nextMuted: boolean) => {
    const outcome = audioController.startContinuous(nextVolume, nextMuted);
    if (outcome === 'AUTOPLAY_BLOCKED') {
      audioStateRef.current = 'BLOCKED';
      setAudioState('BLOCKED');
      addNotice('AUDIO_BLOCKED', 'ALERT');
    } else if (outcome === 'FAILED') {
      audioStateRef.current = 'FAILED';
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
      setPending((current) => {
        const next = current.filter((key) => !completedKeys.includes(sectionIdentity(key)));
        pendingRef.current = next;
        return next;
      });
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
      // The server just changed what is RUNNING. With durable intent that is
      // a fact about the authority projection, not about this frame: a
      // section that was preparing is now watching, and only the ordered read
      // can say so. Without it a restored section stays "preparing" forever
      // while a watch runs behind it.
      if (intent !== undefined) void refreshIntent();
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
      // This frame is not a hint that the projection may be stale: it is the
      // server saying the physical watch ended. The green light goes out on
      // the frame, and the re-read that follows is how the page learns what
      // the server intends to do next -- not what it waits for before telling
      // the truth about what is running. A GET can hang.
      if (intent !== undefined) {
        disprove([event.stopped.sectionKey]);
        void refreshIntent();
      }
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
          audioStateRef.current = 'BLOCKED';
          setAudioState('BLOCKED');
          addNotice('AUDIO_BLOCKED', 'ALERT', disposition.cue.sectionKey);
        } else if (outcome === 'FAILED') {
          audioStateRef.current = 'FAILED';
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
  }, [addNotice, audioController, disprove, intent, loadBatchStatus, refreshIntent, send, startContinuousAudio]);

  useEffect(() => {
    const unsubscribeEvents = runtime.watch.subscribe(handleServerEvent);
    const unsubscribeState = runtime.watch.subscribeState((next) => {
      setConnection(next);
      if (next === 'OPEN') {
        hadConnection.current = true;
        // The cutoff is not lifted here. A socket being open again says the
        // page COULD learn what is running; it does not say what is. Only a
        // read issued from this point on can, so this is the line the release
        // is measured from.
        if (socketCutoffRef.current) cutoffReleaseAt.current = authorityIssued.current;
        sendQueuedStart();
      } else if (next === 'CLOSED' || next === 'ERROR') {
        const connectionOwnedState = hadConnection.current
          || queuedStart.current !== null
          || activeRef.current.length > 0;
        hadConnection.current = false;
        startAttempt.current += 1;
        startingRef.current = false;
        setStarting(false);
        announcedAudioCaps.current.clear();
        queuedStart.current = null;
        pendingPolicies.current.clear();
        pendingRef.current = [];
        setPending([]);
        setActive([]);
        setObservations([]);
        setEpisodes([]);
        setAlerts([]);
        // The durable-intent projection needs the same treatment, and does
        // not get it from `setActive([])`: the socket this page watched
        // through is gone, so the last read's `materialized` rows are no
        // longer something this page can assert. Without this the badge, the
        // active count and the connection line all keep saying "watching"
        // over a closed socket until some future GET happens to disagree.
        //
        // Recorded as a cutoff over every answer rather than as a list of
        // Sections, because the list can be empty and the statement still
        // true: a page whose first read had not landed, or whose rows were
        // all still preparing, has nothing to name -- and a read issued
        // before the close, returning RUNNING after it, would light the whole
        // desk up over a socket that is gone.
        socketCutoffRef.current = true;
        cutoffReleaseAt.current = null;
        setSocketCutoff(true);
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
  }, [addNotice, audioController, disprove, handleServerEvent, runtime, sendQueuedStart]);

  // The first read, and the reconnection of intent to what is running.
  //
  // The second half matters as much as the first. The server materializes
  // stored intent when a page ATTACHES, which happens after this component
  // has already read once -- so the first read legitimately shows sections
  // as preparing. Re-reading when the socket reaches OPEN is what turns them
  // green, and it is driven by this page's own connection rather than by
  // anything another tab did, so it introduces no cross-tab live sync.
  useEffect(() => {
    if (intent === undefined) return;
    void refreshIntent();
  }, [intent, refreshIntent]);

  useEffect(() => {
    if (intent === undefined) return;
    const previous = intentConnection.current;
    intentConnection.current = connection;
    // The mount read above covers the state the page started in. Only a
    // TRANSITION into OPEN is new information: the server materializes
    // stored intent when a page attaches, so this is the read that turns a
    // restored section green.
    if (previous !== null && previous !== 'OPEN' && connection === 'OPEN') {
      void refreshIntent();
    }
  }, [connection, intent, refreshIntent]);

  // A page with standing intent is an audience for it: the server tears every
  // physical watch down when the last page leaves, so a restored session only
  // comes back to life because a page attached.
  useEffect(() => {
    if (intent === undefined || intentStatus !== 'READY') return;
    const wanted = intentSnapshot?.entries.some((entry) => entry.policy !== null) === true;
    if (wanted && runtime.watch.state !== 'OPEN' && runtime.watch.state !== 'CONNECTING') {
      runtime.watch.connect();
    }
  }, [intent, intentSnapshot, intentStatus, runtime]);

  useEffect(() => {
    audioProviderMounted.current = true;
    return () => {
      telemetryAbort.current?.abort();
      telemetryEpoch.current += 1;
      batchRequestRevisions.current.clear();
      sectionRequestRevisions.current.clear();
      announcedAudioCaps.current.clear();
      startAttempt.current += 1;
      audioProviderMounted.current = false;
      queueMicrotask(() => {
        if (!audioProviderMounted.current) audioController.dispose();
      });
    };
  }, [audioController]);

  /**
   * Whether a section may be taken off the managed list.
   *
   * Fail closed on every uncertainty. With durable intent the server's rows
   * are the answer -- not this page's `active` list, which is empty on a page
   * that joined after the watches were armed -- and if those rows could not
   * be read at all, the honest answer is "not now" rather than a guess that
   * hides a running watch behind an empty list.
   */
  const isRemovable = useCallback((sectionKey: SectionKey): boolean => {
    if (intent === undefined) {
      return !activeRef.current.some((watch) => sameSection(watch.sectionKey, sectionKey));
    }
    if (intentStatusRef.current !== 'READY') return false;
    // A submission whose outcome is unknown is the same uncertainty in a
    // different place: the server may already have armed a watch this page's
    // snapshot knows nothing about, and removing the row would take away the
    // only thing that could ever stop it. Same authority, same reason: the
    // Remove pressed in the turn the submission went out must see it.
    if (isUncertain(sectionKey)) return false;
    const entry = findIntentEntry(intentSnapshotRef.current, sectionKey);
    if (entry === null) return true;
    // A tombstone whose teardown is still running, or that still names a live
    // watch, is not finished stopping.
    return entry.policy === null && !entry.stopping && entry.running === null;
  }, [intent, isUncertain]);

  const select = useCallback((sectionKey: SectionKey) => {
    if (watchableTermsRef.current?.has(sectionKey.term) === false) {
      addNotice('TERM_OUT_OF_RANGE', 'ALERT', sectionKey);
      return;
    }
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

  const updateWatchableTerms = useCallback((terms: readonly string[]) => {
    const next = new Set(terms);
    watchableTermsRef.current = next;
    setWatchableTerms(next);
  }, []);

  const remove = useCallback((sectionKey: SectionKey) => {
    // With durable intent this list is the ONLY place a watch can be stopped
    // from, and the server's rows -- not this page's view of what is running
    // -- decide whether removing a row would hide one. A page that joined
    // late has never seen a START frame, so its `active` list is empty while
    // the process is watching nine sections.
    if (intent !== undefined && !isRemovable(sectionKey)) {
      addNotice('SELECTION_BLOCKED', 'ALERT', sectionKey);
      return;
    }
    if (intent === undefined
      && activeRef.current.some((watch) => sameSection(watch.sectionKey, sectionKey))) return;
    const next = selectedRef.current.filter((value) => !sameSection(value, sectionKey));
    selectedRef.current = next;
    telemetryEpoch.current += 1;
    batchRequestRevisions.current.clear();
    sectionRequestRevisions.current.clear();
    telemetryAbort.current?.abort();
    setSelected(next);
    onSelectedChange?.(next);
  }, [addNotice, intent, isRemovable, onSelectedChange]);

  const enableSound = useCallback(async () => {
    audioStateRef.current = 'UNLOCKING';
    setAudioState('UNLOCKING');
    let state: WatchAudioUnlockResult;
    try {
      state = await audioController.unlock();
    } catch {
      state = 'FAILED';
    }
    audioStateRef.current = state;
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
      audioStateRef.current = 'BLOCKED';
      setAudioState('BLOCKED');
      addNotice('AUDIO_BLOCKED', 'ALERT');
      return 'BLOCKED';
    }
    if (outcome === 'FAILED') {
      audioStateRef.current = 'FAILED';
      setAudioState('FAILED');
      addNotice('AUDIO_FAILED', 'ALERT');
      return 'FAILED';
    }
    return 'READY';
  }, [addNotice, audioController, enableSound]);

  const startSelected = useCallback(async (policy: WatchPolicyV1) => {
    if (startingRef.current) return;
    // With durable intent, starting is a change to what the user WANTS
    // watched. The socket does not carry it: the server decides what is
    // running from the stored rows, so a page that also sent a START would
    // be asserting a second answer to the same question.
    if (intent !== undefined) {
      // ONE basis for the whole press, captured before anything is awaited.
      // Which Sections still need starting and what each of them is compared
      // against both come from it, so the batch acts on the screen the user
      // was looking at rather than on whatever the previous item produced.
      const basis = gestureBasis();
      if (basis === null) {
        await refreshIntent();
        return;
      }
      const wanted = selectedRef.current.filter((sectionKey) => {
        const entry = findIntentEntry(basis, sectionKey);
        return entry === null || entry.policy === null;
      });
      if (wanted.length === 0) return;
      startingRef.current = true;
      setStarting(true);
      try {
        if (audioStateRef.current !== 'READY') await enableSound();
        await runIntentBatch(
          basis,
          wanted.map((sectionKey) => ({ sectionKey, policy })),
        );
        if (runtime.watch.state !== 'OPEN') runtime.watch.connect();
      } finally {
        startingRef.current = false;
        setStarting(false);
      }
      return;
    }
    const inactive = selectedRef.current.filter((sectionKey) =>
      watchableTermsRef.current?.has(sectionKey.term) !== false
      &&
      !activeRef.current.some((watch) => sameSection(watch.sectionKey, sectionKey))
      && !pendingRef.current.some((pendingKey) => sameSection(pendingKey, sectionKey)));
    if (inactive.length === 0) return;
    const attempt = startAttempt.current + 1;
    startAttempt.current = attempt;
    startingRef.current = true;
    setStarting(true);
    const items = inactive.map((sectionKey) => ({ sectionKey, policy }));
    for (const item of items) pendingPolicies.current.set(sectionIdentity(item.sectionKey), policy);
    pendingRef.current = inactive;
    setPending(inactive);
    try {
      if (audioStateRef.current !== 'READY') await enableSound();
      if (startAttempt.current !== attempt) return;
      queuedStart.current = items;
      if (runtime.watch.state === 'OPEN') sendQueuedStart();
      else runtime.watch.connect();
    } catch (error) {
      queuedStart.current = null;
      for (const item of items) pendingPolicies.current.delete(sectionIdentity(item.sectionKey));
      pendingRef.current = [];
      setPending([]);
      addNotice('COMMAND_FAILED', 'ALERT', undefined, error instanceof Error ? error.message : undefined);
    } finally {
      if (startAttempt.current === attempt) {
        startingRef.current = false;
        setStarting(false);
      }
    }
  }, [
    addNotice,
    enableSound,
    gestureBasis,
    intent,
    refreshIntent,
    runIntentBatch,
    runtime,
    sendQueuedStart,
  ]);

  /** Stops watching one section the page is currently showing as watched. */
  const stopSection = useCallback((sectionKey: SectionKey) => {
    if (intent === undefined) return false;
    void submitIntent(sectionKey, null);
    return true;
  }, [intent, submitIntent]);

  const stop = useCallback((watch: ActiveWatchView) => {
    if (stopSection(watch.sectionKey)) return;
    send({ type: 'STOP_WATCH', watch: { activeWatchId: watch.activeWatchId, sectionKey: watch.sectionKey } });
  }, [send, stopSection]);

  const updatePolicy = useCallback((watch: ActiveWatchView, policy: WatchPolicyV1) => {
    if (intent !== undefined) {
      void submitIntent(watch.sectionKey, policy);
      return;
    }
    if (send({
      type: 'UPDATE_POLICY',
      watch: { activeWatchId: watch.activeWatchId, sectionKey: watch.sectionKey },
      policy,
    })) {
      announcedAudioCaps.current.delete(watch.activeWatchId);
      setActive((current) => current.map((value) =>
        value.activeWatchId === watch.activeWatchId ? { ...value, policy } : value));
    }
  }, [intent, send, submitIntent]);

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
    const keys = [...selectedRef.current];
    for (const watch of activeRef.current) {
      if (!keys.some((key) => sameSection(key, watch.sectionKey))) keys.push(watch.sectionKey);
    }
    if (keys.length === 0) {
      setBatchStatuses([]);
      setSectionStatuses([]);
      telemetryResourcesRef.current = [];
      setTelemetryResources([]);
      setTelemetryLoading(false);
      return;
    }
    const abort = new AbortController();
    telemetryAbort.current = abort;
    setTelemetryLoading(true);
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
    const relevantResourceKeys = new Set([
      ...keys.map((sectionKey) => sectionTelemetryFailureKey(sectionKey)),
      ...batches.map((batch) => batchTelemetryFailureKey(batch)),
    ]);
    setTelemetryResources((current) => {
      const retained = current.filter((resource) => relevantResourceKeys.has(resource.key));
      let next: readonly WatchTelemetryResourceState[] = retained;
      for (const sectionKey of keys) {
        const target = sectionTelemetryTarget(sectionKey);
        const existing = next.find((resource) => resource.key === target.key) ?? target;
        next = replaceByIdentity(next, { ...existing, loading: true }, (resource) => resource.key);
      }
      for (const batch of batches) {
        const target = batchTelemetryTarget(batch);
        const existing = next.find((resource) => resource.key === target.key) ?? target;
        next = replaceByIdentity(next, { ...existing, loading: true }, (resource) => resource.key);
      }
      telemetryResourcesRef.current = next;
      return next;
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
      const target = sectionTelemetryTarget(ticket.sectionKey);
      if (result.status === 'rejected') markTelemetryFailure(target, result.reason);
      else {
        const status = normalizeSectionStatus(result.value);
        markTelemetrySuccess(target, status.freshness, sectionLastSuccessAt(status));
      }
    });
    batchResults.forEach((result, index) => {
      const ticket = batchTickets[index]!;
      if (batchRequestRevisions.current.get(ticket.identity) !== ticket.revision) return;
      const target = batchTelemetryTarget(ticket.batch);
      if (result.status === 'rejected') markTelemetryFailure(target, result.reason);
      else {
        const status = normalizeBatchStatus(result.value);
        markTelemetrySuccess(target, status.freshness, batchLastSuccessAt(status));
      }
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
    setTelemetryLoading(false);
  }, [markTelemetryFailure, markTelemetrySuccess, runtime]);

  const retryTelemetryResource = useCallback(async (key: string) => {
    const resource = telemetryResourcesRef.current.find((candidate) => candidate.key === key);
    if (resource === undefined || resource.loading) return;
    if (resource.kind === 'SECTION' && resource.sectionKey !== null) {
      await loadSectionStatus(resource.sectionKey);
      return;
    }
    if (resource.kind === 'BATCH' && resource.batch !== null) {
      await loadBatchStatus(resource.batch);
    }
  }, [loadBatchStatus, loadSectionStatus]);

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
      setBatchStatuses([]);
      setSectionStatuses([]);
      telemetryResourcesRef.current = [];
      setTelemetryResources([]);
      setTelemetryLoading(false);
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
    active: effectiveActive,
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
    telemetryResources,
    telemetryLoading,
    starting,
    intentStatus,
    intent: intentSnapshot,
    intentSaved,
    // Derived from the DISPROVED view, so a section this page has been told
    // stopped cannot read back as `WATCHING` while a re-read is in flight.
    intentStateFor: (sectionKey) => {
      if (disprovedSnapshot === null) return null;
      const entry = findIntentEntry(disprovedSnapshot, sectionKey);
      return entry === null ? 'NOT_WATCHING' : watchIntentState(disprovedSnapshot, entry);
    },
    setSectionIntent: submitIntent,
    setSectionIntentBatch: submitIntentBatch,
    refreshIntent,
    isSelected: (sectionKey) => selected.some((value) => sameSection(value, sectionKey)),
    isActive: (sectionKey) =>
      effectiveActive.some((watch) => sameSection(watch.sectionKey, sectionKey)),
    isRemovable,
    isWatchable: (sectionKey) => watchableTerms?.has(sectionKey.term) !== false,
    updateWatchableTerms,
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
    retryTelemetryResource,
  }), [
    acknowledge,
    acknowledgeAll,
    effectiveActive,
    isRemovable,
    alerts,
    audioState,
    batchStatuses,
    connection,
    continuousEpisodeIds,
    disconnect,
    dismissAlert,
    enableSound,
    disprovedSnapshot,
    episodes,
    intentSaved,
    intentSnapshot,
    intentStatus,
    muted,
    notices,
    refreshIntent,
    submitIntent,
    submitIntentBatch,
    observations,
    pending,
    refreshTelemetry,
    remove,
    resetAudibleCount,
    retryTelemetryResource,
    resume,
    sectionStatuses,
    select,
    selected,
    setMuted,
    setVolume,
    startSelected,
    stop,
    telemetryLoading,
    telemetryResources,
    testSound,
    updatePolicy,
    updateWatchableTerms,
    volume,
    watchableTerms,
    starting,
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
