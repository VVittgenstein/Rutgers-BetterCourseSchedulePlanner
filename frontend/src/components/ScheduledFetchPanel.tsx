import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { ScheduledFetchControls } from '../hooks/useScheduledFetch';
import { classNames } from '../utils/classNames';
import './ScheduledFetchPanel.css';

interface ScheduledFetchPanelProps {
  controls: ScheduledFetchControls;
  currentTerm?: string;
  currentCampus?: string;
}

const INTERVAL_OPTIONS = [1, 2, 5, 10, 15, 30];

export function ScheduledFetchPanel({ controls, currentTerm, currentCampus }: ScheduledFetchPanelProps) {
  const { t } = useTranslation();
  const { state, isLoading, error, start, stop } = controls;
  const [selectedInterval, setSelectedInterval] = useState(5);

  const isEnabled = state?.config.enabled ?? false;
  const status = state?.status ?? 'idle';

  const handleToggle = async () => {
    if (isEnabled) {
      await stop();
    } else {
      if (!currentTerm || !currentCampus) {
        return;
      }
      await start({
        term: currentTerm,
        campus: currentCampus,
        intervalMinutes: selectedInterval,
        mode: 'incremental',
      });
    }
  };

  const formatTime = (isoString: string | null) => {
    if (!isoString) return null;
    try {
      return new Date(isoString).toLocaleTimeString();
    } catch {
      return isoString;
    }
  };

  const statusText = (() => {
    if (!isEnabled) {
      return t('scheduledFetch.status.disabled');
    }
    if (status === 'running') {
      return t('scheduledFetch.status.running');
    }
    if (state?.nextRunAt) {
      return t('scheduledFetch.status.scheduled', { time: formatTime(state.nextRunAt) });
    }
    return t('scheduledFetch.status.idle');
  })();

  const lastRunText = (() => {
    if (!state?.lastRunAt) {
      return t('scheduledFetch.lastRun.never');
    }
    if (state.lastRunStatus === 'success') {
      return t('scheduledFetch.lastRun.success', { time: formatTime(state.lastRunAt) });
    }
    if (state.lastRunStatus === 'error') {
      return t('scheduledFetch.lastRun.error', { message: state.lastRunMessage ?? 'Unknown' });
    }
    return formatTime(state.lastRunAt);
  })();

  const canEnable = Boolean(currentTerm && currentCampus);

  return (
    <div className={classNames('scheduled-fetch', isEnabled && 'scheduled-fetch--enabled')}>
      <div className="scheduled-fetch__header">
        <div className="scheduled-fetch__title-row">
          <span className="scheduled-fetch__title">{t('scheduledFetch.title')}</span>
          <button
            type="button"
            className={classNames(
              'scheduled-fetch__toggle',
              isEnabled && 'scheduled-fetch__toggle--on'
            )}
            onClick={handleToggle}
            disabled={isLoading || (!isEnabled && !canEnable)}
            aria-pressed={isEnabled}
            aria-label={isEnabled ? t('scheduledFetch.actions.disable') : t('scheduledFetch.actions.enable')}
          >
            <span className="scheduled-fetch__toggle-track">
              <span className="scheduled-fetch__toggle-thumb" />
            </span>
          </button>
        </div>
        <p className="scheduled-fetch__subtitle">{t('scheduledFetch.subtitle')}</p>
      </div>

      {!isEnabled && (
        <div className="scheduled-fetch__controls">
          <label className="scheduled-fetch__interval-label">
            <span>{t('scheduledFetch.interval.label')}</span>
            <select
              className="scheduled-fetch__interval-select"
              value={selectedInterval}
              onChange={(e) => setSelectedInterval(Number(e.target.value))}
            >
              {INTERVAL_OPTIONS.map((minutes) => (
                <option key={minutes} value={minutes}>
                  {t('scheduledFetch.interval.minutes', { value: minutes })}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}

      {isEnabled && (
        <div className="scheduled-fetch__info">
          <div className="scheduled-fetch__info-row">
            <span className="scheduled-fetch__info-label">{t('scheduledFetch.interval.label')}:</span>
            <span className="scheduled-fetch__info-value">
              {t('scheduledFetch.interval.minutes', { value: state?.config.intervalMinutes ?? selectedInterval })}
            </span>
          </div>
          <div className="scheduled-fetch__info-row">
            <span className="scheduled-fetch__info-label">Term:</span>
            <span className="scheduled-fetch__info-value">{state?.config.term}</span>
          </div>
          <div className="scheduled-fetch__info-row">
            <span className="scheduled-fetch__info-label">Campus:</span>
            <span className="scheduled-fetch__info-value">{state?.config.campus}</span>
          </div>
        </div>
      )}

      <div
        className={classNames(
          'scheduled-fetch__status',
          status === 'running' && 'scheduled-fetch__status--running',
          status === 'scheduled' && 'scheduled-fetch__status--scheduled'
        )}
      >
        {isEnabled && <span className="scheduled-fetch__indicator" />}
        <span className="scheduled-fetch__status-text">{statusText}</span>
      </div>

      {state?.lastRunAt && (
        <div className="scheduled-fetch__last-run">
          <span className="scheduled-fetch__last-run-label">{t('scheduledFetch.lastRun.label')}:</span>
          <span
            className={classNames(
              'scheduled-fetch__last-run-value',
              state.lastRunStatus === 'error' && 'scheduled-fetch__last-run-value--error'
            )}
          >
            {lastRunText}
          </span>
        </div>
      )}

      {error && <div className="scheduled-fetch__error">{error}</div>}

      <p className="scheduled-fetch__warning">{t('scheduledFetch.warning')}</p>
    </div>
  );
}
