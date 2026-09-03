import type { SectionKey, WatchPolicyV1 } from '../product';

/**
 * A target-neutral view of standing watch intent held by the server.
 *
 * A target that keeps durable per-user state supplies one of these; a target
 * that does not passes nothing, and the workspace keeps its ordinary
 * connection-scoped behaviour where a watch lives and dies with the socket
 * that started it.
 *
 * The vocabulary here is deliberately about intent and what is running,
 * never about how a particular target stores either. Naming this after one
 * target's storage would put that target's capability in a module the other
 * target's bundle also compiles.
 */

/** What the server is actually running for a section, and under which stamp. */
export interface WatchIntentRunning {
  readonly generation: number;
  readonly revision: number;
  readonly epoch: number;
  readonly policy: WatchPolicyV1;
  /**
   * The identity of the physical watch.
   *
   * Kept rather than parsed and thrown away: it is what every episode
   * control -- acknowledge, resume, reset the audible count, stop -- is
   * addressed by. A page that dropped it could see an alert from a watch it
   * had no way to name, and a page that JOINED after the watch started never
   * receives the frame that would have told it.
   */
  readonly activeWatchId: string;
}

/** Why the server could not act on an intent, and whether it will try again. */
export interface WatchIntentProblem {
  /**
   * True when nothing the server does will change the answer -- the campus
   * is not a product target, the term is outside the window, the catalog does
   * not publish the section.
   *
   * A permanent problem does NOT withdraw the intent. The section can be
   * published again, and a row that vanished on its own would leave the user
   * with a shorter list and no explanation.
   */
  readonly permanent: boolean;
  readonly reason: string;
  readonly retryScheduled: boolean;
}

/**
 * One section's standing intent.
 *
 * `policy === null` is a REMOVAL that the server still remembers, not an
 * absent row: it holds the revision the section was removed at, so a command
 * written before the removal fails its compare-and-swap instead of finding
 * nothing and being admitted.
 */
export interface WatchIntentEntry {
  readonly section: SectionKey;
  readonly policy: WatchPolicyV1 | null;
  readonly revision: number;
  readonly epoch: number;
  readonly running: WatchIntentRunning | null;
  /** This section's own teardown is still in progress; it can still ring. */
  readonly stopping: boolean;
  /** Waiting for a physical slot another section has not released yet. */
  readonly waitingForSlot: boolean;
  readonly problem: WatchIntentProblem | null;
}

export interface WatchIntentSnapshot {
  readonly generation: number;
  readonly entries: readonly WatchIntentEntry[];
}

export interface WatchIntentSubmission {
  readonly section: SectionKey;
  /** `null` asks the server to stop watching the section. */
  readonly policy: WatchPolicyV1 | null;
}

export type WatchIntentOutcome =
  /** Accepted; `snapshot` is the state the submission produced. */
  | 'COMMITTED'
  /** What the page read is not what is there. Re-read; do not resubmit. */
  | 'CONFLICT'
  /** The product cap on watched sections is full. */
  | 'AT_CAPACITY'
  /** Nothing was written and the same submission may be presented again. */
  | 'UNAVAILABLE';

export interface WatchIntentResult {
  readonly outcome: WatchIntentOutcome;
  readonly snapshot: WatchIntentSnapshot | null;
  readonly maximum: number | null;
}

export interface WatchIntentPort {
  read(signal?: AbortSignal): Promise<WatchIntentSnapshot>;
  /**
   * Submits one change against the snapshot the page is showing.
   *
   * The snapshot is passed in rather than remembered by the port so a
   * submission is always compared against what the USER was looking at. A
   * port that quietly used its own latest read could commit a gesture
   * against a state the user never saw.
   */
  submit(
    submission: WatchIntentSubmission,
    snapshot: WatchIntentSnapshot,
    signal?: AbortSignal,
  ): Promise<WatchIntentResult>;
  /**
   * Optional atomic whole-gesture submission. Local implements this so a
   * 255-row action is one authority commit and one reconciliation; targets
   * without it retain the ordered single-item compatibility path.
   */
  submitBatch?(
    submissions: readonly WatchIntentSubmission[],
    snapshot: WatchIntentSnapshot,
    signal?: AbortSignal,
  ): Promise<WatchIntentResult>;
}

export type WatchIntentStatus = 'DISABLED' | 'LOADING' | 'READY' | 'FAILED';

/**
 * What a section's controls should say.
 *
 * Evaluated in order, and the order is the contract: a teardown in progress
 * outranks everything, because that watch is still alive; and `WATCHING` is
 * reachable only from a complete stamp match.
 */
export type WatchIntentState =
  | 'NOT_WATCHING'
  | 'PREPARING'
  | 'WATCHING'
  | 'STOPPING'
  | 'ATTENTION';

/**
  * A key no Section field can forge, written as an ESCAPE rather than as a
  * literal control character.
  *
  * The runtime string is identical either way; the source is not. A raw NUL
  * byte in a text file makes Git classify it as binary, which silently
  * removes the file from diffs, from `grep`, and from every security
  * inventory that walks text sources -- so the one file nobody could read was
  * the one deciding what counts as the same Section.
  */
export function sectionIntentKey(section: SectionKey): string {
  return `${section.term}\u0000${section.campus}\u0000${section.index}`;
}

export function findIntentEntry(
  snapshot: WatchIntentSnapshot | null,
  section: SectionKey,
): WatchIntentEntry | null {
  if (snapshot === null) return null;
  const key = sectionIntentKey(section);
  return snapshot.entries.find((entry) => sectionIntentKey(entry.section) === key) ?? null;
}

/**
 * True only when the server is running a watch for exactly the intent the
 * page is showing.
 *
 * Every part is load-bearing. A stale generation means the whole authority
 * was replaced; a stale revision means the intent moved after the watch
 * started; a stale epoch means the intent was stopped and started again, so
 * the running watch belongs to a cancelled one; and a policy that differs
 * means the watch is running, but not the way the user asked for.
 *
 * This is derived rather than read off the wire on purpose. A boolean sent
 * alongside these fields could contradict them, and a page that believed the
 * boolean would show a green light for a watch that is not running.
 */
export function isIntentRunning(
  snapshot: WatchIntentSnapshot,
  entry: WatchIntentEntry,
): boolean {
  const { policy, running } = entry;
  return policy !== null
    && running !== null
    && running.generation === snapshot.generation
    && running.revision === entry.revision
    && running.epoch === entry.epoch
    && samePolicy(running.policy, policy);
}

export function watchIntentState(
  snapshot: WatchIntentSnapshot,
  entry: WatchIntentEntry,
): WatchIntentState {
  if (entry.stopping) return entry.policy === null ? 'STOPPING' : 'PREPARING';
  if (entry.policy === null) return 'NOT_WATCHING';
  if (isIntentRunning(snapshot, entry)) return 'WATCHING';
  if (entry.problem?.permanent === true) return 'ATTENTION';
  return 'PREPARING';
}

export function samePolicy(left: WatchPolicyV1, right: WatchPolicyV1): boolean {
  return left.notificationMode === right.notificationMode
    && left.maxAudible === right.maxAudible
    && left.continuousDuration.kind === right.continuousDuration.kind
    && (left.continuousDuration.kind !== 'FINITE'
      || right.continuousDuration.kind !== 'FINITE'
      || left.continuousDuration.seconds === right.continuousDuration.seconds);
}
