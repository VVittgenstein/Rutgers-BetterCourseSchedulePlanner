import { WATCH_CONTACT_STALE_MILLISECONDS } from '../product/WatchClient';
import type { WatchRecoveryPhase } from '../product/WatchClient';
import type { WatchNotificationPermission } from './notification';

/**
 * The five-ring truth chain behind "this page will ring".
 *
 * Each ring is a separate claim, and the page is only green when every one of
 * them is true AT THE SAME TIME. They are evaluated in the order below, so the
 * reason the user is shown is the FIRST thing that is actually wrong rather
 * than whichever check happened to run last.
 */
export type WatchReadinessRing =
  /** ① There is a live connection AND the server has proved it recently. */
  | 'CONNECTION'
  /** ② The server is watching exactly what this page says it is watching. */
  | 'ARMED'
  /** ③ The browser's audio output can actually play. */
  | 'AUDIO'
  /** ④ A cue would be noticed: the page is visible, or a fallback exists. */
  | 'DELIVERY'
  /** ⑤ The user has not silenced it. */
  | 'SOUND';

export const WATCH_READINESS_RINGS: readonly WatchReadinessRing[] = [
  'CONNECTION',
  'ARMED',
  'AUDIO',
  'DELIVERY',
  'SOUND',
];

export type WatchReadinessLevel =
  /** Every ring holds. */
  | 'READY'
  /** Something is watched, and something in the chain is broken. */
  | 'DEGRADED'
  /** Nothing is being watched, so there is nothing to promise. */
  | 'STOPPED';

export type WatchReadinessReason =
  | 'NOT_WATCHING'
  | 'DISCONNECTED'
  | 'RECONNECTING'
  | 'CONTACT_STALE'
  | 'INTENT_UNREADABLE'
  | 'PREPARING'
  | 'AUDIO_BLOCKED'
  | 'AUDIO_FAILED'
  | 'AUDIO_LOCKED'
  | 'NO_FALLBACK_WHILE_HIDDEN'
  | 'MUTED';

/** The one thing the user can press to fix this ring. */
export type WatchReadinessAction =
  | 'NONE'
  | 'RECONNECT'
  | 'ENABLE_SOUND'
  | 'ALLOW_NOTIFICATIONS'
  | 'UNMUTE'
  | 'WAIT';

export type WatchAudioReadinessInput = 'READY' | 'BLOCKED' | 'FAILED' | 'UNLOCKING' | 'MUTED' | null;

export interface WatchReadinessInput {
  /** Sections the user has asked to be watched right now. */
  readonly wanted: number;
  /**
   * How many of those the server is watching under the CURRENT stamp.
   *
   * "Under the current stamp" is the whole content of ring ②: a watch armed
   * for a policy the user has since changed, or under an authority generation
   * that has been replaced, is a running watch that will not do what the page
   * says it does.
   */
  readonly armed: number;
  readonly connectionOpen: boolean;
  readonly recovery: WatchRecoveryPhase;
  /** True while this page cannot assert that anything at all is running. */
  readonly connectionCutoff: boolean;
  /** Whether the server has ever proved it was there on this connection. */
  readonly hasContact: boolean;
  /**
   * Whether that proof is still inside the staleness bound.
   *
   * Passed in as an answer rather than computed here from a clock, and the
   * caller is expected to compute it AT READ TIME with
   * {@link isWatchContactFresh}. That is the difference between a page that
   * expires its own claim and one that waits to be told: a scheduled timer is
   * throttled in a hidden tab, so the only reliable moment to compare the
   * clock is whenever the answer is actually being used.
   */
  readonly contactFresh: boolean;
  /** True when the page could not read what the server is watching. */
  readonly intentUnreadable: boolean;
  readonly audio: WatchAudioReadinessInput;
  readonly muted: boolean;
  readonly volume: number;
  readonly pageVisible: boolean;
  readonly notificationPermission: WatchNotificationPermission;
  /** The in-app setting. A user who turned notifications off keeps them off. */
  readonly notificationsEnabled: boolean;
}

export interface WatchReadinessState {
  readonly level: WatchReadinessLevel;
  /** The first broken ring, or `null` when nothing is broken. */
  readonly reason: WatchReadinessReason | null;
  readonly brokenRing: WatchReadinessRing | null;
  readonly action: WatchReadinessAction;
  /** Every ring's own answer. `null` where the ring does not apply. */
  readonly rings: Readonly<Record<WatchReadinessRing, boolean | null>>;
  /** True while a cue would only be noticed through a notification. */
  readonly fallbackInUse: boolean;
}

const NOTHING_WATCHED: Readonly<Record<WatchReadinessRing, boolean | null>> = {
  CONNECTION: null,
  ARMED: null,
  AUDIO: null,
  DELIVERY: null,
  SOUND: null,
};

/**
 * True when a notification could carry a cue the page itself cannot.
 *
 * `default` is not a fallback. An un-answered permission prompt posts nothing,
 * and counting it would be the exact substitution this surface forbids:
 * a maybe standing in for a yes.
 */
export function notificationFallbackAvailable(
  permission: WatchNotificationPermission,
  enabled: boolean,
): boolean {
  return enabled && permission === 'granted';
}

/**
 * Whether the last proof the server was there is still inside the bound.
 *
 * Called at the moment the answer is used, not on a schedule. A page whose
 * heartbeat has expired must read as degraded even if nothing woke it up --
 * that is the single property no event announces, and the one a frozen server
 * on a live socket depends on nobody checking.
 */
export function isWatchContactFresh(
  lastContactAt: number | null,
  now: number,
  staleAfterMilliseconds: number = WATCH_CONTACT_STALE_MILLISECONDS,
): boolean {
  return lastContactAt !== null && now - lastContactAt <= staleAfterMilliseconds;
}

/** Evaluates the chain. Pure: every input is an answer, not a source. */
export function evaluateWatchReadiness(input: WatchReadinessInput): WatchReadinessState {
  const fallback = notificationFallbackAvailable(
    input.notificationPermission,
    input.notificationsEnabled,
  );
  if (input.wanted === 0) {
    return {
      level: 'STOPPED',
      reason: 'NOT_WATCHING',
      brokenRing: null,
      action: 'NONE',
      rings: NOTHING_WATCHED,
      fallbackInUse: false,
    };
  }

  const connection = input.connectionOpen && !input.connectionCutoff && input.contactFresh;
  const armed = !input.intentUnreadable && input.armed >= input.wanted;
  const audio = input.audio === 'READY';
  const delivery = input.pageVisible || fallback;
  const sound = !input.muted && input.volume > 0;
  const rings: Readonly<Record<WatchReadinessRing, boolean | null>> = {
    CONNECTION: connection,
    ARMED: armed,
    AUDIO: audio,
    DELIVERY: delivery,
    SOUND: sound,
  };

  const degraded = (
    reason: WatchReadinessReason,
    brokenRing: WatchReadinessRing,
    action: WatchReadinessAction,
  ): WatchReadinessState => ({
    level: 'DEGRADED',
    reason,
    brokenRing,
    action,
    rings,
    fallbackInUse: false,
  });

  if (!connection) {
    if (input.recovery === 'STOPPED_BY_USER') {
      return degraded('DISCONNECTED', 'CONNECTION', 'RECONNECT');
    }
    if (!input.connectionOpen || input.connectionCutoff) {
      return degraded('RECONNECTING', 'CONNECTION', 'WAIT');
    }
    // Open, not cut off, and still no recent proof the server is there. This
    // is the case no user action and no network event announces, and it is
    // exactly why READY is a bounded-staleness claim rather than a socket
    // state.
    return degraded('CONTACT_STALE', 'CONNECTION', 'WAIT');
  }
  if (!armed) {
    return input.intentUnreadable
      ? degraded('INTENT_UNREADABLE', 'ARMED', 'WAIT')
      : degraded('PREPARING', 'ARMED', 'WAIT');
  }
  if (!audio) {
    if (input.audio === 'FAILED') return degraded('AUDIO_FAILED', 'AUDIO', 'ENABLE_SOUND');
    if (input.audio === 'UNLOCKING') return degraded('AUDIO_LOCKED', 'AUDIO', 'WAIT');
    return degraded('AUDIO_BLOCKED', 'AUDIO', 'ENABLE_SOUND');
  }
  if (!delivery) {
    return degraded('NO_FALLBACK_WHILE_HIDDEN', 'DELIVERY', 'ALLOW_NOTIFICATIONS');
  }
  if (!sound) {
    return degraded('MUTED', 'SOUND', 'UNMUTE');
  }
  return {
    level: 'READY',
    reason: null,
    brokenRing: null,
    action: 'NONE',
    rings,
    // Green through a notification rather than through the page itself is
    // still green -- but the page says which, because "you will hear it" and
    // "you will be told" are different promises.
    fallbackInUse: !input.pageVisible,
  };
}

/**
 * When the chain must be re-evaluated even if nothing else happens.
 *
 * `null` when contact is already stale or absent: there is nothing left to
 * expire. Otherwise the moment the current evidence stops being fresh.
 */
export function nextReadinessExpiry(
  lastContactAt: number | null,
  staleAfterMilliseconds: number = WATCH_CONTACT_STALE_MILLISECONDS,
): number | null {
  if (lastContactAt === null) return null;
  return lastContactAt + staleAfterMilliseconds;
}
