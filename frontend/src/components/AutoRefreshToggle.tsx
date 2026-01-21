import { useTranslation } from 'react-i18next';

import type { AutoRefreshControls } from '../hooks/useAutoRefresh';
import { classNames } from '../utils/classNames';
import './AutoRefreshToggle.css';

interface AutoRefreshToggleProps {
  controls: AutoRefreshControls;
}

const INTERVAL_OPTIONS = [15, 30, 45, 60, 90, 120];

export function AutoRefreshToggle({ controls }: AutoRefreshToggleProps) {
  const { t } = useTranslation();
  const { enabled, intervalSeconds, status, secondsUntilNext, toggle, setIntervalSeconds } = controls;

  const statusText = (() => {
    if (!enabled) {
      return t('autoRefresh.status.disabled');
    }
    if (status === 'refreshing') {
      return t('autoRefresh.status.refreshing');
    }
    if (secondsUntilNext !== null) {
      return t('autoRefresh.status.waiting', { seconds: secondsUntilNext });
    }
    return t('autoRefresh.status.enabled');
  })();

  return (
    <div className={classNames('auto-refresh', enabled && 'auto-refresh--enabled')}>
      <div className="auto-refresh__header">
        <div className="auto-refresh__title-row">
          <span className="auto-refresh__title">{t('autoRefresh.title')}</span>
          <button
            type="button"
            className={classNames('auto-refresh__toggle', enabled && 'auto-refresh__toggle--on')}
            onClick={toggle}
            aria-pressed={enabled}
            aria-label={enabled ? t('autoRefresh.actions.disable') : t('autoRefresh.actions.enable')}
          >
            <span className="auto-refresh__toggle-track">
              <span className="auto-refresh__toggle-thumb" />
            </span>
          </button>
        </div>
        <p className="auto-refresh__subtitle">{t('autoRefresh.subtitle')}</p>
      </div>

      {enabled && (
        <div className="auto-refresh__controls">
          <label className="auto-refresh__interval-label">
            <span>{t('autoRefresh.interval.label')}</span>
            <select
              className="auto-refresh__interval-select"
              value={intervalSeconds}
              onChange={(e) => setIntervalSeconds(Number(e.target.value))}
            >
              {INTERVAL_OPTIONS.map((seconds) => (
                <option key={seconds} value={seconds}>
                  {t('autoRefresh.interval.seconds', { value: seconds })}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}

      <div
        className={classNames(
          'auto-refresh__status',
          status === 'refreshing' && 'auto-refresh__status--refreshing',
          status === 'waiting' && 'auto-refresh__status--waiting'
        )}
      >
        {enabled && <span className="auto-refresh__indicator" />}
        <span className="auto-refresh__status-text">{statusText}</span>
      </div>
    </div>
  );
}
