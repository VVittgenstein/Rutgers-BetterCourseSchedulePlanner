import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import type { CourseFilterState } from '../state/courseFilters';
import { buildCourseQueryParams } from '../state/courseFilters';
import { apiGet, type ApiError, type ApiQueryParamValue } from '../api/client';
import type { CourseSearchResponse, CourseSearchRow } from '../api/types';

export interface CourseResultItem {
  id: number;
  code: string;
  title: string;
  subtitle?: string | null;
  campusCode: string;
  subjectCode: string;
  courseNumber: string;
  level?: string | null;
  termId: string;
  credits: {
    min?: number | null;
    max?: number | null;
    display?: string | null;
  };
  hasOpenSections: boolean;
  sections: {
    total: number;
    open: number;
    deliveryMethods: string[];
  };
  updatedAt?: string | null;
  prerequisites?: string | null;
  subjectDescription?: string | null;
  schoolDescription?: string | null;
}

export interface CourseQueryMeta {
  page: number;
  pageSize: number;
  total: number;
  hasNext: boolean;
  generatedAt: string;
  version: string;
}

export interface UseCourseQueryResult {
  status: 'idle' | 'loading' | 'success' | 'error';
  items: CourseResultItem[];
  meta: CourseQueryMeta | null;
  isLoading: boolean;
  isFetching: boolean;
  error: ApiError | null;
  queryKey: string | null;
  refetch: () => void;
}

export interface UseCourseQueryOptions {
  enabled?: boolean;
  staleTimeMs?: number;
  debounceMs?: number;
}

interface CacheEntry {
  data: CourseResultItem[];
  meta: CourseQueryMeta;
  timestamp: number;
}

const DEFAULT_STALE_TIME = 45_000;
const DEFAULT_DEBOUNCE = 200;

export function useCourseQuery(state: CourseFilterState, options?: UseCourseQueryOptions): UseCourseQueryResult {
  const canQuery = Boolean(state.term && (state.campus || state.subjects.length));
  const enabled = options?.enabled ?? true;
  const staleTime = options?.staleTimeMs ?? DEFAULT_STALE_TIME;
  const debounceMs = options?.debounceMs ?? DEFAULT_DEBOUNCE;

  const params = useMemo(() => {
    if (!enabled || !canQuery) {
      return null;
    }
    try {
      return buildCourseQueryParams(state);
    } catch {
      return null;
    }
  }, [state, enabled, canQuery]);

  const queryKey = useMemo(() => (params ? createStableKey(params) : null), [params]);
  const [debouncedKey, setDebouncedKey] = useState<string | null>(queryKey);
  const cacheRef = useRef(new Map<string, CacheEntry>());
  const paramsRef = useRef(new Map<string, Record<string, ApiQueryParamValue>>());
  const abortControllerRef = useRef<AbortController | null>(null);
  const inFlightKeyRef = useRef<string | null>(null);
  const lastRefreshRef = useRef(0);
  const [refreshKey, setRefreshKey] = useState(0);

  const [resultState, setResultState] = useState<{
    status: UseCourseQueryResult['status'];
    items: CourseResultItem[];
    meta: CourseQueryMeta | null;
    error: ApiError | null;
  }>({
    status: 'idle',
    items: [],
    meta: null,
    error: null,
  });
  const [isFetching, setIsFetching] = useState(false);

  useEffect(() => {
    if (queryKey && params) {
      paramsRef.current.set(queryKey, params);
    }
  }, [params, queryKey]);

  useEffect(() => {
    if (!queryKey) {
      setDebouncedKey(null);
      return;
    }
    const handle = window.setTimeout(() => {
      setDebouncedKey(queryKey);
    }, debounceMs);
    return () => window.clearTimeout(handle);
  }, [queryKey, debounceMs]);

  useEffect(() => {
    const forceRefresh = refreshKey !== lastRefreshRef.current;
    lastRefreshRef.current = refreshKey;

    if (!enabled || !canQuery || !debouncedKey) {
      setResultState((prev) => ({
        status: enabled && canQuery ? 'loading' : 'idle',
        items: enabled && canQuery ? prev.items : [],
        meta: enabled && canQuery ? prev.meta : null,
        error: null,
      }));
      setIsFetching(false);
      abortControllerRef.current?.abort();
      return;
    }

    const existingParams = paramsRef.current.get(debouncedKey);
    if (!existingParams) {
      return;
    }

    const cacheEntry = cacheRef.current.get(debouncedKey);
    const now = Date.now();
    const isCacheValid = cacheEntry && now - cacheEntry.timestamp < staleTime;

    if (cacheEntry && isCacheValid && !forceRefresh) {
      setResultState({
        status: 'success',
        items: cacheEntry.data,
        meta: cacheEntry.meta,
        error: null,
      });
      setIsFetching(false);
      return;
    }

    const controller = new AbortController();
    abortControllerRef.current?.abort();
    abortControllerRef.current = controller;
    inFlightKeyRef.current = debouncedKey;
    setIsFetching(true);

    setResultState((prev) => {
      if (cacheEntry && !forceRefresh) {
        return {
          status: prev.status === 'idle' ? 'loading' : prev.status,
          items: prev.items,
          meta: prev.meta,
          error: null,
        };
      }
      return {
        status: 'loading',
        items: cacheEntry?.data ?? [],
        meta: cacheEntry?.meta ?? null,
        error: null,
      };
    });

    apiGet<CourseSearchResponse>('/courses', existingParams, controller.signal)
      .then((payload) => {
        if (inFlightKeyRef.current !== debouncedKey) {
          return;
        }
        const mapped = payload.data.map(transformCourseRow);
        const meta = payload.meta;
        cacheRef.current.set(debouncedKey, { data: mapped, meta, timestamp: Date.now() });
        setResultState({
          status: 'success',
          items: mapped,
          meta,
          error: null,
        });
      })
      .catch((error) => {
        if (controller.signal.aborted || inFlightKeyRef.current !== debouncedKey) {
          return;
        }
        setResultState((prev) => ({
          status: 'error',
          items: prev.items,
          meta: prev.meta,
          error: error as ApiError,
        }));
      })
      .finally(() => {
        if (inFlightKeyRef.current === debouncedKey) {
          inFlightKeyRef.current = null;
          setIsFetching(false);
        }
      });

    return () => {
      if (inFlightKeyRef.current === debouncedKey) {
        controller.abort();
      }
    };
  }, [enabled, canQuery, debouncedKey, staleTime, refreshKey]);

  useEffect(() => {
    return () => {
      abortControllerRef.current?.abort();
    };
  }, []);

  const refetch = useCallback(() => {
    setRefreshKey((value) => value + 1);
  }, []);

  return {
    status: resultState.status,
    items: resultState.items,
    meta: resultState.meta,
    isLoading: resultState.status === 'loading',
    isFetching,
    error: resultState.error,
    queryKey: debouncedKey,
    refetch,
  };
}

function transformCourseRow(row: CourseSearchRow): CourseResultItem {
  return {
    id: row.courseId,
    code: row.courseString ?? `${row.subjectCode}:${row.courseNumber}`,
    title: row.title,
    subtitle: row.expandedTitle,
    campusCode: row.campusCode,
    subjectCode: row.subjectCode,
    courseNumber: row.courseNumber,
    level: row.level,
    termId: row.termId,
    credits: {
      min: row.creditsMin,
      max: row.creditsMax,
      display: row.creditsDisplay,
    },
    hasOpenSections: row.hasOpenSections,
    sections: {
      total: row.sectionsSummary?.total ?? row.sectionsOpen ?? 0,
      open: row.sectionsSummary?.open ?? row.sectionsOpen ?? 0,
      deliveryMethods: row.sectionsSummary?.deliveryMethods ?? [],
    },
    updatedAt: row.updatedAt,
    prerequisites: row.prerequisites,
    subjectDescription: row.subject?.description ?? null,
    schoolDescription: row.subject?.schoolDescription ?? null,
  };
}

function createStableKey(params: Record<string, ApiQueryParamValue>) {
  const parts: string[] = [];
  const keys = Object.keys(params).sort();
  for (const key of keys) {
    const value = params[key];
    if (value === undefined || value === null) continue;
    if (Array.isArray(value)) {
      const normalized = [...value].map((entry) => String(entry)).sort();
      normalized.forEach((entry) => parts.push(`${key}=${entry}`));
    } else {
      parts.push(`${key}=${String(value)}`);
    }
  }
  return parts.join('&');
}
