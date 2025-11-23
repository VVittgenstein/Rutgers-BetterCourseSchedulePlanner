import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { fetchJobStatus, startFetchJob } from '../api/fetchJobs';
import type { FetchJob } from '../api/types';
import type { TermSeason } from '../utils/term';
import { buildTermId, parseTermId, seasonLabelKey } from '../utils/term';
import { classNames } from '../utils/classNames';
import './DataFetchCard.css';

type CampusOption = { label: string; value: string };

interface DataFetchCardProps {
  defaultTerm?: string;
  defaultCampus?: string;
  campusOptions?: CampusOption[];
  onApplySelection?: (term: string, campus: string) => void;
  onSelectionChange?: (term: string, campus: string) => void;
  onDictionaryRefresh?: () => void;
}

const FALLBACK_CAMPUSES = ['NB', 'NK', 'CM', 'CAM', 'NWK'];

export function DataFetchCard({
  defaultTerm,
  defaultCampus,
  campusOptions,
  onApplySelection,
  onSelectionChange,
  onDictionaryRefresh,
}: DataFetchCardProps) {
  const { t } = useTranslation();
  const parsedDefault = useMemo(() => parseTermId(defaultTerm ?? ''), [defaultTerm]);
  const now = new Date();
  const guessedSeason: TermSeason =
    parsedDefault.season ??
    (now.getMonth() >= 7 ? 'fall' : now.getMonth() >= 4 ? 'summer' : 'spring');
  const [year, setYear] = useState(parsedDefault.year ?? now.getFullYear());
  const [season, setSeason] = useState<TermSeason>(guessedSeason);
  const [campus, setCampus] = useState((defaultCampus ?? campusOptions?.[0]?.value ?? 'NB').toUpperCase());
  const [job, setJob] = useState<FetchJob | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const lastCompletedRef = useRef<string | null>(null);
  const lastSelectionRef = useRef<{ term: string; campus: string } | null>(null);

  const suggestedCampuses = useMemo(() => {
    const seen = new Set<string>();
    const entries: CampusOption[] = [];
    for (const option of campusOptions ?? []) {
      const code = option.value.toUpperCase();
      if (seen.has(code)) continue;
      seen.add(code);
      entries.push({ label: option.label ?? code, value: code });
    }
    for (const code of FALLBACK_CAMPUSES) {
      const normalized = code.toUpperCase();
      if (seen.has(normalized)) continue;
      seen.add(normalized);
      entries.push({ label: normalized, value: normalized });
    }
    return entries;
  }, [campusOptions]);

  const termPreview = useMemo(() => buildTermId(year, season), [year, season]);
  const isRunning = job?.status === 'running';

  useEffect(() => {
    if (!defaultCampus) return;
    const next = defaultCampus.toUpperCase();
    setCampus((prev) => (prev === next ? prev : next));
  }, [defaultCampus]);

  useEffect(() => {
    if (!defaultTerm) return;
    const parsed = parseTermId(defaultTerm);
    setYear((prev) => (parsed.year && parsed.year !== prev ? parsed.year : prev));
    setSeason((prev) => (parsed.season && parsed.season !== prev ? parsed.season : prev));
  }, [defaultTerm]);

  const emitSelection = useCallback(
    (nextTerm: string, nextCampus: string) => {
      const normalizedCampus = nextCampus.trim().toUpperCase();
      if (!nextTerm || !normalizedCampus) return;
      const last = lastSelectionRef.current;
      if (last && last.term === nextTerm && last.campus === normalizedCampus) return;
      lastSelectionRef.current = { term: nextTerm, campus: normalizedCampus };
      onSelectionChange?.(nextTerm, normalizedCampus);
    },
    [onSelectionChange],
  );

  const refreshStatus = useCallback(() => {
    fetchJobStatus()
      .then((response) => {
        setJob(response.data.job);
        setError(null);
      })
      .catch((err) => {
        const message = err instanceof Error ? err.message : t('fetcher.status.errorFallback');
        setError(message);
      });
  }, [t]);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (!isRunning) return;
    const handle = window.setInterval(refreshStatus, 3500);
    return () => window.clearInterval(handle);
  }, [isRunning, refreshStatus]);

  useEffect(() => {
    if (!job || job.status === 'running') return;
    if (lastCompletedRef.current === job.id) return;
    lastCompletedRef.current = job.id;
    if (job.status === 'success') {
      onDictionaryRefresh?.();
      onApplySelection?.(job.term, job.campus);
    } else if (job.status === 'error') {
      setError(job.message ?? t('fetcher.status.errorFallback'));
    }
  }, [job, onApplySelection, onDictionaryRefresh, t]);

  const handleStart = async () => {
    setIsSubmitting(true);
    setError(null);
    const campusCode = campus.trim().toUpperCase();
    try {
      const response = await startFetchJob({ year, season, campus: campusCode });
      setJob(response.data.job);
    } catch (err) {
      const message = err instanceof Error ? err.message : t('fetcher.status.errorFallback');
      setError(message);
    } finally {
      setIsSubmitting(false);
    }
  };

  const statusLabel = useMemo(() => {
    if (job?.status === 'running') {
      return t('fetcher.status.running', { term: job.term, campus: job.campus });
    }
    if (job?.status === 'success') {
      return t('fetcher.status.success', { term: job.term, campus: job.campus });
    }
    if (job?.status === 'error') {
      return t('fetcher.status.error', { message: job.message ?? t('fetcher.status.errorFallback') });
    }
    return t('fetcher.status.idle');
  }, [job, t]);

  const canSubmit = Boolean(campus.trim()) && !isSubmitting && !isRunning;

  return (
    <section className="fetch-card" aria-label={t('fetcher.title')}>
      <div className="fetch-card__toolbar">
        <label className="fetch-card__control">
          <span className="fetch-card__label">{t('fetcher.fields.year')}</span>
          <input
            type="number"
            min={2015}
            max={2100}
            value={year}
            onChange={(event) => {
              const nextYear = Number(event.target.value) || now.getFullYear();
              setYear(nextYear);
              emitSelection(buildTermId(nextYear, season), campus);
            }}
          />
        </label>
        <label className="fetch-card__control">
          <span className="fetch-card__label">{t('fetcher.fields.season')}</span>
          <select
            value={season}
            onChange={(event) => {
              const nextSeason = event.target.value as TermSeason;
              setSeason(nextSeason);
              emitSelection(buildTermId(year, nextSeason), campus);
            }}
          >
            <option value="spring">{t(`fetcher.seasons.${seasonLabelKey('spring')}`)}</option>
            <option value="summer">{t(`fetcher.seasons.${seasonLabelKey('summer')}`)}</option>
            <option value="fall">{t(`fetcher.seasons.${seasonLabelKey('fall')}`)}</option>
            <option value="winter">{t(`fetcher.seasons.${seasonLabelKey('winter')}`)}</option>
          </select>
        </label>
        <label className="fetch-card__control">
          <span className="fetch-card__label">{t('fetcher.fields.campus')}</span>
          <input
            list="fetch-campuses"
            value={campus}
            onChange={(event) => {
              const nextCampus = event.target.value.toUpperCase();
              setCampus(nextCampus);
              emitSelection(termPreview, nextCampus);
            }}
            placeholder="NB"
          />
          <datalist id="fetch-campuses">
            {suggestedCampuses.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </datalist>
        </label>
        <div className="fetch-card__term" aria-live="polite">
          {t('fetcher.fields.termPreview', { term: termPreview })}
        </div>
        <div className="fetch-card__actions">
          <button
            type="button"
            className={classNames('fetch-card__btn', (isSubmitting || isRunning) && 'fetch-card__btn--ghost')}
            onClick={handleStart}
            disabled={!canSubmit}
          >
            {isSubmitting || isRunning ? t('fetcher.actions.running') : t('fetcher.actions.start')}
          </button>
          <div
            className={classNames(
              'fetch-card__status',
              job?.status === 'running' && 'fetch-card__status--running',
              job?.status === 'success' && 'fetch-card__status--success',
              job?.status === 'error' && 'fetch-card__status--error',
            )}
            aria-live="polite"
          >
            <span className="fetch-card__status-text">{statusLabel}</span>
            {job?.logFile && (
              <span className="fetch-card__log">{t('fetcher.hints.log', { path: job.logFile })}</span>
            )}
            {error && <span className="fetch-card__status-error">{error}</span>}
          </div>
        </div>
      </div>
    </section>
  );
}
