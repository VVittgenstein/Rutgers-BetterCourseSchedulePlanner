import { useEffect, useRef, useState } from 'react';

import {
  ProductClientError,
  type CourseQueryRequestV1,
  type FilterFieldId,
  type FilterStateV1,
  type FilterValuesV1,
  type ProductRuntimePort,
} from '../product';

export type RelaxableField =
  | 'subjects'
  | 'keywords'
  | 'courseNumberBands'
  | 'levels'
  | 'credits'
  | 'core'
  | 'prerequisite'
  | 'sectionIndexes'
  | 'openStatuses'
  | 'modalities'
  | 'synchronicities'
  | 'instructors'
  | 'availability'
  | 'meetingLocations'
  | 'examCodes'
  | 'permission';

export type IncompleteToggleKey = 'prerequisite' | 'modality' | 'synchronicity';

export type Relaxation =
  | { readonly kind: 'CLEAR_FIELD'; readonly field: RelaxableField; readonly stableId: FilterFieldId }
  | { readonly kind: 'INCLUDE_INCOMPLETE'; readonly toggle: IncompleteToggleKey; readonly stableId: FilterFieldId };

export interface DiagnosisRow {
  readonly relaxation: Relaxation;
  readonly total: number;
}

export type DiagnosisState =
  | { readonly status: 'IDLE' }
  | { readonly status: 'PROBING' }
  | { readonly status: 'READY'; readonly rows: readonly DiagnosisRow[]; readonly allZero: boolean }
  | { readonly status: 'UNAVAILABLE' };

/** Upper bound on concurrent pageSize-1 probes issued for one empty result. */
export const MAX_DIAGNOSIS_PROBES = 8;

/** One delayed retry per probe covers the Local runtime's snapshot-rebuild window. */
export const PROBE_RETRY_DELAY_MS = 1500;

/** The subset of filter values a relaxation can read or rewrite; both
 * `FilterValuesV1` (request) and `FilterStateV1` (draft) satisfy it. */
type RelaxableValues = Pick<FilterValuesV1, RelaxableField | 'includeIncomplete'>;

/** Fixed priority: data-quality-sensitive Section fields first, broad course
 * fields last. Term and campuses are never relaxed (scope changes require Apply). */
const RELAXATION_PRIORITY: readonly Relaxation[] = [
  { kind: 'INCLUDE_INCOMPLETE', toggle: 'synchronicity', stableId: 'FLT-S04b' },
  { kind: 'INCLUDE_INCOMPLETE', toggle: 'modality', stableId: 'FLT-S04a' },
  { kind: 'INCLUDE_INCOMPLETE', toggle: 'prerequisite', stableId: 'FLT-C09' },
  { kind: 'CLEAR_FIELD', field: 'synchronicities', stableId: 'FLT-S04b' },
  { kind: 'CLEAR_FIELD', field: 'modalities', stableId: 'FLT-S04a' },
  { kind: 'CLEAR_FIELD', field: 'openStatuses', stableId: 'FLT-S03' },
  { kind: 'CLEAR_FIELD', field: 'availability', stableId: 'FLT-S06' },
  { kind: 'CLEAR_FIELD', field: 'meetingLocations', stableId: 'FLT-S07' },
  { kind: 'CLEAR_FIELD', field: 'instructors', stableId: 'FLT-S05' },
  { kind: 'CLEAR_FIELD', field: 'examCodes', stableId: 'FLT-S09' },
  { kind: 'CLEAR_FIELD', field: 'permission', stableId: 'FLT-S10' },
  { kind: 'CLEAR_FIELD', field: 'sectionIndexes', stableId: 'FLT-S01' },
  { kind: 'CLEAR_FIELD', field: 'prerequisite', stableId: 'FLT-C09' },
  { kind: 'CLEAR_FIELD', field: 'core', stableId: 'FLT-C08' },
  { kind: 'CLEAR_FIELD', field: 'credits', stableId: 'FLT-C07' },
  { kind: 'CLEAR_FIELD', field: 'levels', stableId: 'FLT-C06' },
  { kind: 'CLEAR_FIELD', field: 'courseNumberBands', stableId: 'FLT-C05' },
  { kind: 'CLEAR_FIELD', field: 'keywords', stableId: 'FLT-C04' },
  { kind: 'CLEAR_FIELD', field: 'subjects', stableId: 'FLT-C03' },
];

/** Mirrors the "active" predicate of FilterPanel.fieldSummary for one field. */
function fieldIsActive(values: RelaxableValues, field: RelaxableField): boolean {
  switch (field) {
    case 'credits': return values.credits !== null;
    case 'core': return values.core.codes.length > 0;
    case 'prerequisite': return values.prerequisite !== 'ANY';
    case 'meetingLocations': return values.meetingLocations.locations.length > 0;
    case 'permission': return values.permission !== 'ANY';
    default: return values[field].length > 0;
  }
}

function relaxationIsActive(values: RelaxableValues, relaxation: Relaxation): boolean {
  if (relaxation.kind === 'CLEAR_FIELD') return fieldIsActive(values, relaxation.field);
  switch (relaxation.toggle) {
    case 'prerequisite':
      return values.prerequisite !== 'ANY' && !values.includeIncomplete.prerequisite;
    case 'modality':
      return values.modalities.length > 0 && !values.includeIncomplete.modality;
    case 'synchronicity':
      return values.synchronicities.length > 0 && !values.includeIncomplete.synchronicity;
  }
}

/** Every single-step relaxation that would change the given values, in
 * priority order and capped at MAX_DIAGNOSIS_PROBES. */
export function activeRelaxations(values: RelaxableValues): Relaxation[] {
  return RELAXATION_PRIORITY
    .filter((relaxation) => relaxationIsActive(values, relaxation))
    .slice(0, MAX_DIAGNOSIS_PROBES);
}

/** Neutralises exactly one field (same neutral values as createNeutralFilterState)
 * or turns on exactly one includeIncomplete toggle. */
export function relaxValues<T extends RelaxableValues>(values: T, relaxation: Relaxation): T {
  if (relaxation.kind === 'INCLUDE_INCOMPLETE') {
    return {
      ...values,
      includeIncomplete: { ...values.includeIncomplete, [relaxation.toggle]: true },
    };
  }
  switch (relaxation.field) {
    case 'credits': return { ...values, credits: null };
    case 'core': return { ...values, core: { codes: [], mode: values.core.mode } };
    case 'prerequisite': return { ...values, prerequisite: 'ANY' };
    case 'meetingLocations':
      return { ...values, meetingLocations: { locations: [], mode: values.meetingLocations.mode } };
    case 'permission': return { ...values, permission: 'ANY' };
    default: return { ...values, [relaxation.field]: [] };
  }
}

export function relaxFilters(state: FilterStateV1, relaxation: Relaxation): FilterStateV1 {
  return relaxValues(state, relaxation);
}

export function probeRequest(base: CourseQueryRequestV1, relaxation: Relaxation): CourseQueryRequestV1 {
  return {
    filters: { contractVersion: 3, values: relaxValues(base.filters.values, relaxation) },
    page: { page: 1, pageSize: 1 },
    sort: { ...base.sort },
  };
}

function errorCode(error: unknown): string | null {
  return error instanceof ProductClientError ? error.apiError?.error.code ?? null : null;
}

function isRateLimited(error: unknown): boolean {
  return errorCode(error) === 'RATE_LIMITED';
}

/** Transient server states worth exactly one delayed retry. */
function isRetryable(error: unknown): boolean {
  if (!(error instanceof ProductClientError)) return false;
  const code = errorCode(error);
  return code === 'CATALOG_NOT_READY' || code === 'SEARCH_DATA_NOT_READY' || error.status >= 500;
}

function abortableDelay(milliseconds: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(new Error('Probe retry aborted.'));
      return;
    }
    const onAbort = () => {
      clearTimeout(timer);
      reject(new Error('Probe retry aborted.'));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener('abort', onAbort);
      resolve();
    }, milliseconds);
    signal.addEventListener('abort', onAbort, { once: true });
  });
}

interface ProbeOutcome {
  readonly priority: number;
  readonly relaxation: Relaxation;
  readonly total: number;
}

/**
 * Diagnoses an empty Course result by probing relaxed variants of the last
 * successful request with pageSize 1 and reading `page.total`.
 *
 * The hook is keyed on the request's filters: a new request restarts the
 * diagnosis, a null request (new search, scope change, unmount) aborts every
 * in-flight probe. Probes never touch the search session. Each probe gets one
 * delayed retry on CATALOG_NOT_READY / SEARCH_DATA_NOT_READY / 5xx; a
 * RATE_LIMITED error stops the diagnosis as UNAVAILABLE. Note that the
 * workspace applies a chosen relaxation to the CURRENT draft, which may
 * already differ from the diagnosed request if the user edited it since.
 */
export function useEmptyResultDiagnosis({
  request,
  runtime,
}: {
  readonly request: CourseQueryRequestV1 | null;
  readonly runtime: ProductRuntimePort;
}): DiagnosisState {
  const [state, setState] = useState<DiagnosisState>({ status: 'IDLE' });
  const requestKey = request === null ? null : JSON.stringify(request.filters);
  const latestRequest = useRef<CourseQueryRequestV1 | null>(request);

  useEffect(() => {
    latestRequest.current = request;
  });

  useEffect(() => {
    const current = latestRequest.current;
    if (requestKey === null || current === null) {
      setState({ status: 'IDLE' });
      return undefined;
    }
    const abort = new AbortController();
    const relaxations = activeRelaxations(current.filters.values);
    if (relaxations.length === 0) {
      setState({ status: 'READY', rows: [], allZero: false });
      return () => abort.abort();
    }
    setState({ status: 'PROBING' });
    let rateLimited = false;
    const probe = async (relaxation: Relaxation, priority: number): Promise<ProbeOutcome> => {
      const probeQuery = probeRequest(current, relaxation);
      // Exactly two attempts: the first, and one delayed retry for a
      // transient failure. Any other failure, or a second one, drops the probe.
      let retried = false;
      for (;;) {
        try {
          const response = await runtime.product.searchCourses(probeQuery, abort.signal);
          return { priority, relaxation, total: response.page.total };
        } catch (error) {
          if (abort.signal.aborted) throw error;
          if (isRateLimited(error)) {
            rateLimited = true;
            throw error;
          }
          if (retried || !isRetryable(error)) throw error;
          retried = true;
          await abortableDelay(PROBE_RETRY_DELAY_MS, abort.signal);
        }
      }
    };
    void Promise.allSettled(relaxations.map(probe)).then((results) => {
      if (abort.signal.aborted) return;
      if (rateLimited) {
        setState({ status: 'UNAVAILABLE' });
        return;
      }
      const outcomes = results.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []));
      if (outcomes.length === 0) {
        setState({ status: 'UNAVAILABLE' });
        return;
      }
      outcomes.sort((left, right) => right.total - left.total || left.priority - right.priority);
      setState({
        status: 'READY',
        rows: outcomes.map(({ relaxation, total }) => ({ relaxation, total })),
        allZero: outcomes.every(({ total }) => total === 0),
      });
    });
    return () => abort.abort();
  }, [requestKey, runtime]);

  return state;
}
