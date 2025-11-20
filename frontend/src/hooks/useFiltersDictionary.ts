import { useCallback, useEffect, useRef, useState } from 'react';

import type { FiltersDictionary } from '../components/FilterPanel';
import { fetchFiltersDictionary } from '../api/filters';
import type { ApiError } from '../api/client';
import { fallbackFiltersDictionary } from '../data/fallbackDictionary';

export type FiltersDictionaryStatus = 'idle' | 'loading' | 'success' | 'error';

interface UseFiltersDictionaryResult {
  dictionary: FiltersDictionary | null;
  status: FiltersDictionaryStatus;
  error: ApiError | null;
  usingFallback: boolean;
  refetch: () => void;
}

export function useFiltersDictionary(): UseFiltersDictionaryResult {
  const [dictionary, setDictionary] = useState<FiltersDictionary | null>(null);
  const [status, setStatus] = useState<FiltersDictionaryStatus>('idle');
  const [error, setError] = useState<ApiError | null>(null);
  const [usingFallback, setUsingFallback] = useState(false);
  const [requestKey, setRequestKey] = useState(0);
  const abortRef = useRef<AbortController | null>(null);

  const load = useCallback(
    (signal: AbortSignal) => {
      setStatus('loading');
      setError(null);
      setUsingFallback(false);
      fetchFiltersDictionary(signal)
        .then((payload) => {
          setDictionary(payload);
          setStatus('success');
        })
        .catch((err) => {
          if (signal.aborted) return;
          setDictionary(fallbackFiltersDictionary);
          setError(err as ApiError);
          setUsingFallback(true);
          setStatus('error');
        });
    },
    [],
  );

  useEffect(() => {
    const controller = new AbortController();
    abortRef.current?.abort();
    abortRef.current = controller;
    load(controller.signal);
    return () => controller.abort();
  }, [load, requestKey]);

  useEffect(() => {
    return () => abortRef.current?.abort();
  }, []);

  const refetch = useCallback(() => {
    setRequestKey((value) => value + 1);
  }, []);

  return {
    dictionary,
    status,
    error,
    usingFallback,
    refetch,
  };
}
