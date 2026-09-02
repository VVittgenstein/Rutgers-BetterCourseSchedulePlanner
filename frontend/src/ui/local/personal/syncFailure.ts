import { ProductClientError } from '../../shared/product';
import type { LocalMessageKey } from '../i18n/catalog';

/**
 * How a failed local personal-state request should be treated.
 *
 * - REVISION_CONFLICT: a sub-revision (settings / current filters / saved view)
 *   moved because another RBCSP window wrote the same data. The mutation can be
 *   replayed once against a fresh snapshot.
 * - STATE_RESET: the user-state revision moved, which only happens when another
 *   window ran a full local reset. Never replay blindly.
 * - TRANSIENT: the service was busy, rebuilding, or unreachable; the request had
 *   no effect and can be retried with backoff.
 * - ABORTED: the caller cancelled the request; stay silent.
 * - REJECTED: the request itself was refused (validation, name conflict,
 *   storage full, missing view); surface it without retrying.
 */
export type LocalSyncFailureKind =
  | 'REVISION_CONFLICT'
  | 'STATE_RESET'
  | 'TRANSIENT'
  | 'ABORTED'
  | 'REJECTED';

const SUB_REVISION_CONFLICT_CODES: ReadonlySet<string> = new Set([
  'SETTINGS_REVISION_CONFLICT',
  'CURRENT_FILTERS_REVISION_CONFLICT',
  'SAVED_VIEW_REVISION_CONFLICT',
]);

const USER_STATE_REVISION_CONFLICT_CODE = 'USER_STATE_REVISION_CONFLICT';
const SAVED_VIEW_NAME_CONFLICT_CODE = 'SAVED_VIEW_NAME_CONFLICT';
const STORAGE_FULL_STATUS = 507;
const RATE_LIMITED_STATUS = 429;
const CONFLICT_STATUS = 409;

function isAbortError(error: unknown): boolean {
  return typeof error === 'object'
    && error !== null
    && 'name' in error
    && (error as { readonly name?: unknown }).name === 'AbortError';
}

/** The `code` string the local runtime sent, without trusting the shared code union. */
export function localApiErrorCode(error: unknown): string | null {
  if (!(error instanceof ProductClientError)) return null;
  const code: unknown = error.apiError?.error.code;
  return typeof code === 'string' ? code : null;
}

export function classifyLocalSyncFailure(error: unknown): LocalSyncFailureKind {
  if (isAbortError(error)) return 'ABORTED';
  if (error instanceof ProductClientError) {
    const code = localApiErrorCode(error);
    if (error.status === CONFLICT_STATUS) {
      if (code !== null && SUB_REVISION_CONFLICT_CODES.has(code)) return 'REVISION_CONFLICT';
      if (code === USER_STATE_REVISION_CONFLICT_CODE) return 'STATE_RESET';
      return 'REJECTED';
    }
    if (error.status === RATE_LIMITED_STATUS) return 'TRANSIENT';
    if (error.status === STORAGE_FULL_STATUS) return 'REJECTED';
    if (error.status >= 500) return 'TRANSIENT';
    return 'REJECTED';
  }
  // fetch() reports network failures as a TypeError.
  if (error instanceof TypeError) return 'TRANSIENT';
  return 'REJECTED';
}

export function isSavedViewNameConflict(error: unknown): boolean {
  return error instanceof ProductClientError
    && error.status === CONFLICT_STATUS
    && localApiErrorCode(error) === SAVED_VIEW_NAME_CONFLICT_CODE;
}

/** Diagnostic only: the CURRENT_REVISION detail a 409 carries, if any. */
export function currentRevisionDetail(error: unknown): number | null {
  if (!(error instanceof ProductClientError)) return null;
  const details = error.apiError?.error.details ?? [];
  for (const detail of details) {
    if (detail.kind === 'CURRENT_REVISION' && Number.isFinite(detail.revision)) return detail.revision;
  }
  return null;
}

export type LocalSyncStatus =
  | { readonly phase: 'IDLE' }
  | { readonly phase: 'SAVING' }
  | { readonly phase: 'RETRYING'; readonly reason: 'BUSY' | 'CONFLICT'; readonly attempt: number }
  | { readonly phase: 'RECOVERED'; readonly reason: 'CONFLICT' | 'REFRESH' }
  | { readonly phase: 'STALE'; readonly error: Error }
  | {
    readonly phase: 'FAILED';
    readonly reason: 'UNAVAILABLE' | 'CONFLICT' | 'STATE_RESET' | 'REJECTED';
    readonly error: Error;
  };

export const LOCAL_SYNC_IDLE: LocalSyncStatus = { phase: 'IDLE' };

export function localSyncFailureReason(kind: LocalSyncFailureKind): Extract<
  LocalSyncStatus,
  { phase: 'FAILED' }
>['reason'] {
  switch (kind) {
    case 'TRANSIENT':
      return 'UNAVAILABLE';
    case 'REVISION_CONFLICT':
      return 'CONFLICT';
    case 'STATE_RESET':
      return 'STATE_RESET';
    default:
      return 'REJECTED';
  }
}

export function localSyncMessageKey(status: LocalSyncStatus): LocalMessageKey | null {
  switch (status.phase) {
    case 'IDLE':
      return null;
    case 'SAVING':
      return 'local.status.busy';
    case 'RETRYING':
      return status.reason === 'BUSY' ? 'local.sync.retrying_busy' : 'local.sync.retrying_conflict';
    case 'RECOVERED':
      return 'local.sync.recovered';
    case 'STALE':
      return 'local.sync.stale';
    case 'FAILED':
      switch (status.reason) {
        case 'UNAVAILABLE':
          return 'local.sync.failed_unavailable';
        case 'CONFLICT':
          return 'local.sync.failed_conflict';
        case 'STATE_RESET':
          return 'local.sync.failed_reset';
        default:
          return 'local.status.error';
      }
    default:
      return null;
  }
}

/** Where the snapshot currently held by the provider came from. */
export type LocalSnapshotOrigin = 'INITIAL' | 'SELF' | 'PEER' | 'VISIBILITY' | 'RELOAD';

/**
 * Whether a snapshot from another window (or a visibility catch-up) must NOT
 * rewrite this tab's search filter panel: while a debounced filter persist is
 * still pending, or once the user has edited the search draft in this tab, the
 * panel belongs to the user and only this tab's own writes or an explicit
 * reload may replace it.
 */
export function holdsInitialFiltersFor(origin: LocalSnapshotOrigin, editing: boolean): boolean {
  return editing && (origin === 'PEER' || origin === 'VISIBILITY');
}
