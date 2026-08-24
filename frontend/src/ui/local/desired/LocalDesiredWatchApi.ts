import type { SectionKey, WatchPolicyV1 } from '../../shared/product';
import type {
  WatchIntentEntry,
  WatchIntentPort,
  WatchIntentProblem,
  WatchIntentResult,
  WatchIntentRunning,
  WatchIntentSnapshot,
  WatchIntentSubmission,
} from '../../shared/watch';

/**
 * The local desired-watch authority, over ordinary local HTTP.
 *
 * `desired_watches` is the local build's record of which sections the user
 * wants watched. It survives restarts, every tab edits the same rows, and the
 * server -- not any page -- decides what is actually running from it.
 *
 * There is no socket here on purpose. An earlier design gave this a
 * WebSocket with a projection stream so every tab saw every edit as it
 * happened; the product ruling that replaced it is that a change made in one
 * tab becomes visible in another when that tab reloads. A page reads on load,
 * reads again after its own submissions, and that is the whole protocol.
 */

export const LOCAL_DESIRED_WATCH_PATH = '/api/v1/local/desired-watch';
export const LOCAL_DESIRED_WATCH_CONTRACT_VERSION = 1;

export interface LocalDesiredWatchApiOptions {
  readonly baseUrl?: string;
  readonly fetch?: typeof fetch;
  readonly mutationId?: () => string;
  readonly session: () => string | null;
}

export class LocalDesiredWatchError extends Error {
  readonly status: number;

  constructor(status: number) {
    super(`LOCAL_DESIRED_WATCH_${status}`);
    this.name = 'LocalDesiredWatchError';
    this.status = status;
  }
}

export class LocalDesiredWatchApi implements WatchIntentPort {
  readonly #baseUrl: string;
  readonly #fetch: typeof fetch;
  readonly #mutationId: () => string;
  readonly #session: () => string | null;

  constructor(options: LocalDesiredWatchApiOptions) {
    this.#baseUrl = (options.baseUrl ?? '').replace(/\/$/u, '');
    this.#fetch = options.fetch ?? globalThis.fetch.bind(globalThis);
    this.#mutationId = options.mutationId ?? (() => crypto.randomUUID());
    this.#session = options.session;
  }

  async read(signal?: AbortSignal): Promise<WatchIntentSnapshot> {
    const response = await this.#fetch(`${this.#baseUrl}${LOCAL_DESIRED_WATCH_PATH}`, {
      cache: 'no-store',
      credentials: 'same-origin',
      headers: { accept: 'application/json' },
      method: 'GET',
      ...(signal === undefined ? {} : { signal }),
    });
    const body = await decodeJson(response);
    if (!response.ok) throw new LocalDesiredWatchError(response.status);
    return parseSnapshot(body);
  }

  /**
   * Submits one compare-and-swap.
   *
   * The generation and the section's revision come from the snapshot the
   * PAGE was showing, and the mutation id is fresh per gesture. Together
   * those are what make a lost response safe to retry and a stale gesture
   * impossible to apply: the server replays the original answer for a
   * repeated id, and refuses a revision that has moved on.
   *
   * A refusal is never resubmitted from here. The revision the page held is
   * stale precisely because the state changed, so replaying the user's
   * gesture would apply a decision they made about a state that no longer
   * exists. The caller re-reads instead.
   */
  async submit(
    submission: WatchIntentSubmission,
    snapshot: WatchIntentSnapshot,
    signal?: AbortSignal,
  ): Promise<WatchIntentResult> {
    const session = this.#session();
    const headers = new Headers({
      accept: 'application/json',
      'content-type': 'application/json',
    });
    if (session !== null) headers.set('x-bcsp-session', session);
    const entry = snapshot.entries.find((candidate) =>
      sameSection(candidate.section, submission.section));
    const response = await this.#fetch(`${this.#baseUrl}${LOCAL_DESIRED_WATCH_PATH}`, {
      body: JSON.stringify({
        protocolVersion: 1,
        payload: {
          contractVersion: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
          section: submission.section,
          policy: submission.policy,
          basedOnRevision: entry?.revision ?? 0,
          authorityGeneration: snapshot.generation,
          mutationId: this.#mutationId(),
        },
      }),
      cache: 'no-store',
      credentials: 'same-origin',
      headers,
      method: 'PUT',
      ...(signal === undefined ? {} : { signal }),
    });
    const body = await decodeJson(response);
    // Every outcome the authority itself produced -- accepted, refused,
    // deferred -- carries this body, whatever status it earned. A body
    // without it is a protocol or transport failure (a malformed request, a
    // missing session), and those are not answers about the user's intent.
    //
    // The Section goes in because half of what makes a commit believable is
    // whether it agrees with the state it returned ABOUT THAT SECTION, and
    // the body alone does not say which one was written.
    return parseMutationResult(body, response.status, submission.section);
  }
}

export function createLocalDesiredWatchApi(
  options: LocalDesiredWatchApiOptions,
): LocalDesiredWatchApi {
  return new LocalDesiredWatchApi(options);
}

/**
 * The complete set of answers the authority can give, and the status each one
 * is allowed to arrive with.
 *
 * Both halves are load-bearing. An unknown outcome string must not fall
 * through to some default -- a future refusal read as `CONFLICT` would tell
 * the page to re-read when the truth might be that nothing happened -- and a
 * known outcome carrying the wrong status is not this authority answering.
 * Reading `COMMITTED` off a 500 from a proxy, or `AUTHORITY_FULL` off a 200,
 * would let the page report a state the server never reached.
 */
const OUTCOMES: Readonly<Record<string, {
  readonly outcome: WatchIntentResult['outcome'];
  readonly status: number;
}>> = {
  COMMITTED: { outcome: 'COMMITTED', status: 200 },
  STALE_GENERATION: { outcome: 'CONFLICT', status: 409 },
  STALE_REVISION: { outcome: 'CONFLICT', status: 409 },
  MUTATION_ID_CONFLICT: { outcome: 'CONFLICT', status: 409 },
  LIMIT_EXCEEDED: { outcome: 'AT_CAPACITY', status: 409 },
  AUTHORITY_FULL: { outcome: 'UNAVAILABLE', status: 503 },
};

const ENVELOPE_KEYS = ['protocolVersion', 'data'] as const;

const MUTATION_RESULT_KEYS = [
  'contractVersion',
  'outcome',
  'replayed',
  'authorityGeneration',
  'currentRevision',
  'maximum',
  'committed',
  'state',
] as const;

const COMMITTED_KEYS = ['revision', 'materializationEpoch', 'epochChanged'] as const;

/**
 * Which of the two optional numbers each answer is required to carry, and
 * which it is forbidden to.
 *
 * Both halves are the point. A `COMMITTED` answer arriving with a
 * `currentRevision` is not this authority speaking -- nothing it commits has
 * one -- and a `STALE_REVISION` arriving WITHOUT one leaves the page told to
 * re-read with no number to re-read against, which is indistinguishable from
 * "revision 0" and would re-admit a command against a Section that has rows.
 */
const REFUSAL_SHAPES: Readonly<Record<string, {
  readonly currentRevision: boolean;
  readonly maximum: boolean;
}>> = {
  STALE_GENERATION: { currentRevision: false, maximum: false },
  STALE_REVISION: { currentRevision: true, maximum: false },
  MUTATION_ID_CONFLICT: { currentRevision: false, maximum: false },
  LIMIT_EXCEEDED: { currentRevision: false, maximum: true },
  AUTHORITY_FULL: { currentRevision: false, maximum: true },
};

/**
 * Strict, exhaustive parsing of one mutation answer.
 *
 * The write path decodes as strictly as the read path, for the same reason:
 * this body is the only evidence the page has that its own gesture took
 * effect. A loose decode that accepted a missing `state` on a `COMMITTED`
 * answer, or a negative `maximum`, would let the page render a state nothing
 * on the server ever held.
 *
 * Three kinds of check, and the third is the one a field-by-field decoder
 * still misses: the envelope may carry exactly `protocolVersion` and `data`
 * and nothing else; each outcome's optional fields are required or forbidden
 * rather than merely well-typed; and a commit's own numbers must agree with
 * the state it returned. Fields that are individually plausible and jointly
 * impossible are exactly what a response spliced across two authorities looks
 * like, and that is the shape this decode exists to refuse.
 */
function parseMutationResult(
  value: unknown,
  status: number,
  section: SectionKey,
): WatchIntentResult {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ENVELOPE_KEYS)
    || value.protocolVersion !== 1
    || !isRecord(value.data)
    || !hasExactKeys(value.data, MUTATION_RESULT_KEYS)
  ) {
    throw new LocalDesiredWatchError(status);
  }
  const data = value.data;
  const known = typeof data.outcome === 'string' ? OUTCOMES[data.outcome] : undefined;
  if (
    known === undefined
    || known.status !== status
    || data.contractVersion !== LOCAL_DESIRED_WATCH_CONTRACT_VERSION
    || typeof data.replayed !== 'boolean'
    || !isSafeCount(data.authorityGeneration)
    || !(data.currentRevision === null || isSafeCount(data.currentRevision))
    || !(data.maximum === null || isSafeCount(data.maximum))
  ) {
    throw new LocalDesiredWatchError(status);
  }
  if (known.outcome === 'COMMITTED') {
    // A commit that does not say what it wrote, does not carry the state it
    // produced, or carries numbers only a refusal has, is not a commit this
    // page can act on.
    if (
      data.currentRevision !== null
      || data.maximum !== null
      || !isRecord(data.committed)
      || !hasExactKeys(data.committed, COMMITTED_KEYS)
      || !isSafeCount(data.committed.revision)
      || !isSafeCount(data.committed.materializationEpoch)
      || typeof data.committed.epochChanged !== 'boolean'
    ) {
      throw new LocalDesiredWatchError(status);
    }
    const snapshot = parseSnapshotData(data.state);
    const entry = snapshot.entries.find((candidate) => sameSection(candidate.section, section));
    // One authority, not two. The generation on the envelope, the revision
    // and epoch the commit claims, and the row the state actually holds for
    // this Section all describe the same write -- so a body assembled from
    // reads taken either side of a rotation contradicts itself here rather
    // than becoming the page's next `basedOnRevision`.
    if (
      data.authorityGeneration !== snapshot.generation
      || entry === undefined
      || entry.revision !== data.committed.revision
      || entry.epoch !== data.committed.materializationEpoch
    ) {
      throw new LocalDesiredWatchError(status);
    }
    return { outcome: 'COMMITTED', snapshot, maximum: null };
  }
  // A refusal carries no state: the page has to re-read, because what it held
  // is not what is there. What it does carry is fixed per outcome.
  const shape = REFUSAL_SHAPES[data.outcome as string];
  if (
    shape === undefined
    || data.committed !== null
    || data.state !== null
    || (data.currentRevision !== null) !== shape.currentRevision
    || (data.maximum !== null) !== shape.maximum
  ) {
    throw new LocalDesiredWatchError(status);
  }
  return { outcome: known.outcome, snapshot: null, maximum: data.maximum };
}

async function decodeJson(response: Response): Promise<unknown> {
  try {
    return await response.json() as unknown;
  } catch {
    throw new LocalDesiredWatchError(response.status);
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function sameSection(left: SectionKey, right: SectionKey): boolean {
  return left.term === right.term && left.campus === right.campus && left.index === right.index;
}

/**
 * Strict, exhaustive parsing.
 *
 * Every field is required and every unexpected key is a failure. A projection
 * that decoded loosely could silently drop the field a green light depends
 * on, and the page would then show a state the server never reported.
 */
function parseSnapshot(value: unknown): WatchIntentSnapshot {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ENVELOPE_KEYS)
    || value.protocolVersion !== 1
    || !isRecord(value.data)
  ) {
    throw new LocalDesiredWatchError(200);
  }
  return parseSnapshotData(value.data);
}

function parseSnapshotData(value: unknown): WatchIntentSnapshot {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ['contractVersion', 'authorityGeneration', 'entries'])
    || value.contractVersion !== LOCAL_DESIRED_WATCH_CONTRACT_VERSION
    || !isSafeCount(value.authorityGeneration)
    || !Array.isArray(value.entries)
  ) {
    throw new LocalDesiredWatchError(200);
  }
  return {
    generation: value.authorityGeneration,
    entries: value.entries.map(parseEntry),
  };
}

function parseEntry(value: unknown): WatchIntentEntry {
  if (
    !isRecord(value)
    || !hasExactKeys(value, [
      'section',
      'policy',
      'revision',
      'materializationEpoch',
      'materialized',
      'pendingDisarm',
      'blockedOnSlot',
      'failure',
    ])
    || !isSectionKey(value.section)
    || !isSafeCount(value.revision)
    || !isSafeCount(value.materializationEpoch)
    || typeof value.pendingDisarm !== 'boolean'
    || typeof value.blockedOnSlot !== 'boolean'
  ) {
    throw new LocalDesiredWatchError(200);
  }
  return {
    section: value.section,
    policy: value.policy === null ? null : parsePolicy(value.policy),
    revision: value.revision,
    epoch: value.materializationEpoch,
    running: value.materialized === null ? null : parseRunning(value.materialized),
    stopping: value.pendingDisarm,
    waitingForSlot: value.blockedOnSlot,
    problem: value.failure === null ? null : parseProblem(value.failure),
  };
}

function parseRunning(value: unknown): WatchIntentRunning {
  if (
    !isRecord(value)
    || !hasExactKeys(value, [
      'authorityGeneration',
      'revision',
      'materializationEpoch',
      'policy',
      'activeWatchId',
    ])
    || !isSafeCount(value.authorityGeneration)
    || !isSafeCount(value.revision)
    || !isSafeCount(value.materializationEpoch)
    || typeof value.activeWatchId !== 'string'
  ) {
    throw new LocalDesiredWatchError(200);
  }
  return {
    generation: value.authorityGeneration,
    revision: value.revision,
    epoch: value.materializationEpoch,
    policy: parsePolicy(value.policy),
    // Kept, not discarded. Every episode control the page can offer -- stop,
    // acknowledge, reset the audible count -- is addressed by this id, and a
    // page that joined after the watch started never receives the frame that
    // would otherwise have carried it.
    activeWatchId: value.activeWatchId,
  };
}

function parseProblem(value: unknown): WatchIntentProblem {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ['classification', 'reason', 'retryScheduled'])
    || (value.classification !== 'PERMANENT' && value.classification !== 'TRANSIENT')
    || typeof value.reason !== 'string'
    || typeof value.retryScheduled !== 'boolean'
  ) {
    throw new LocalDesiredWatchError(200);
  }
  return {
    permanent: value.classification === 'PERMANENT',
    reason: value.reason,
    retryScheduled: value.retryScheduled,
  };
}

function parsePolicy(value: unknown): WatchPolicyV1 {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ['notificationMode', 'maxAudible', 'continuousDuration'])
    || (value.notificationMode !== 'ONE_SHOT' && value.notificationMode !== 'CONTINUOUS')
    || !isSafeCount(value.maxAudible)
    || !isRecord(value.continuousDuration)
  ) {
    throw new LocalDesiredWatchError(200);
  }
  const duration = value.continuousDuration;
  if (duration.kind === 'UNLIMITED' && hasExactKeys(duration, ['kind'])) {
    return {
      notificationMode: value.notificationMode,
      maxAudible: value.maxAudible,
      continuousDuration: { kind: 'UNLIMITED' },
    };
  }
  if (
    duration.kind === 'FINITE'
    && hasExactKeys(duration, ['kind', 'seconds'])
    && isSafeCount(duration.seconds)
  ) {
    return {
      notificationMode: value.notificationMode,
      maxAudible: value.maxAudible,
      continuousDuration: { kind: 'FINITE', seconds: duration.seconds },
    };
  }
  throw new LocalDesiredWatchError(200);
}

function isSectionKey(value: unknown): value is SectionKey {
  return isRecord(value)
    && hasExactKeys(value, ['term', 'campus', 'index'])
    && typeof value.term === 'string'
    && typeof value.campus === 'string'
    && typeof value.index === 'string';
}

function isSafeCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}
