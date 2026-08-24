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
    if (!isOutcomeBody(body)) throw new LocalDesiredWatchError(response.status);
    const data = body.data;
    return {
      outcome: mapOutcome(String(data.outcome)),
      snapshot: data.state === undefined || data.state === null
        ? null
        : parseSnapshotData(data.state),
      maximum: typeof data.maximum === 'number' ? data.maximum : null,
    };
  }
}

export function createLocalDesiredWatchApi(
  options: LocalDesiredWatchApiOptions,
): LocalDesiredWatchApi {
  return new LocalDesiredWatchApi(options);
}

function mapOutcome(outcome: string): WatchIntentResult['outcome'] {
  switch (outcome) {
    case 'COMMITTED':
      return 'COMMITTED';
    case 'LIMIT_EXCEEDED':
      return 'AT_CAPACITY';
    case 'AUTHORITY_FULL':
      return 'UNAVAILABLE';
    // STALE_GENERATION, STALE_REVISION and MUTATION_ID_CONFLICT all mean the
    // same thing to a page: what you read is not what is there.
    default:
      return 'CONFLICT';
  }
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

function isOutcomeBody(value: unknown): value is { readonly data: Record<string, unknown> } {
  return isRecord(value)
    && value.protocolVersion === 1
    && isRecord(value.data)
    && value.data.contractVersion === LOCAL_DESIRED_WATCH_CONTRACT_VERSION
    && typeof value.data.outcome === 'string';
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
  if (!isRecord(value) || value.protocolVersion !== 1 || !isRecord(value.data)) {
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
