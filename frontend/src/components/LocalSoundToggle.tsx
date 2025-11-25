import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import type { LocalSoundControls } from '../hooks/useLocalSoundNotifications';
import { classNames } from '../utils/classNames';
import './LocalSoundToggle.css';

interface LocalSoundToggleProps {
  controls: LocalSoundControls;
}

const formatClock = (timestamp: number | null) => {
  if (!timestamp) return null;
  return new Date(timestamp).toLocaleTimeString();
};

export function LocalSoundToggle({ controls }: LocalSoundToggleProps) {
  const { t } = useTranslation();
  const { enabled, status, lastError, audioBlocked, lastPolledAt, toasts, deviceId } = controls;

  const statusLabel = useMemo(() => {
    if (!enabled) return t('subscriptionCenter.localSound.status.disabled');
    if (status === 'polling') return t('subscriptionCenter.localSound.status.polling');
    if (status === 'error') return t('subscriptionCenter.localSound.status.error');
    if (lastPolledAt) {
      return t('subscriptionCenter.localSound.status.idle', { time: formatClock(lastPolledAt) });
    }
    return t('subscriptionCenter.localSound.status.ready');
  }, [enabled, lastPolledAt, status, t]);

  return (
    <div className="local-sound">
      <header className="local-sound__header">
        <div>
          <p className="local-sound__eyebrow">{t('subscriptionCenter.localSound.eyebrow')}</p>
          <h4 className="local-sound__title">{t('subscriptionCenter.localSound.title')}</h4>
          <p className="local-sound__hint">{t('subscriptionCenter.localSound.subtitle')}</p>
        </div>
        <button
          type="button"
          className={classNames('local-sound__toggle', enabled && 'local-sound__toggle--on')}
          onClick={() => {
            void controls.toggle();
          }}
        >
          {enabled
            ? t('subscriptionCenter.localSound.actions.disable')
            : t('subscriptionCenter.localSound.actions.enable')}
        </button>
      </header>

      <div className="local-sound__status">
        <div>
          <p className="local-sound__status-label">{statusLabel}</p>
          <p className="local-sound__status-meta">
            {t('subscriptionCenter.localSound.deviceLabel')}: <code>{deviceId}</code>
          </p>
        </div>
        <div className="local-sound__status-chip">
          {enabled ? t('subscriptionCenter.localSound.state.on') : t('subscriptionCenter.localSound.state.off')}
        </div>
      </div>

      {audioBlocked && (
        <div className="local-sound__feedback local-sound__feedback--warning">
          <p>{t('subscriptionCenter.localSound.soundBlocked')}</p>
          <button
            type="button"
            className="local-sound__pill"
            onClick={() => {
              void controls.resumeAudio();
            }}
          >
            {t('subscriptionCenter.localSound.actions.resumeAudio')}
          </button>
        </div>
      )}

      {lastError && (
        <div className="local-sound__feedback local-sound__feedback--error">
          <p>{t('subscriptionCenter.localSound.errors.polling', { message: lastError })}</p>
        </div>
      )}

      {toasts.length > 0 && (
        <div className="local-sound__toasts">
          {toasts.map((toast) => (
            <div key={toast.id} className="local-sound__toast">
              <div>
                <p className="local-sound__toast-title">
                  {toast.notification.courseTitle ?? t('subscriptionCenter.localSound.toast.fallbackTitle')}
                </p>
                <p className="local-sound__toast-body">
                  {t('subscriptionCenter.localSound.toast.body', {
                    index: toast.notification.sectionIndex,
                    campus: toast.notification.campus,
                    term: toast.notification.term,
                  })}
                </p>
                <p className="local-sound__toast-meta">
                  {formatClock(toast.receivedAt) ?? ''}{' '}
                  {toast.notification.traceId
                    ? t('subscriptionCenter.localSound.toast.traceId', { id: toast.notification.traceId })
                    : ''}
                </p>
              </div>
              <button
                type="button"
                className="local-sound__pill"
                onClick={() => controls.dismissToast(toast.id)}
              >
                {t('subscriptionCenter.localSound.actions.dismiss')}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
