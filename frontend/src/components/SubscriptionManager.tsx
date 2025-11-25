import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { listActiveSubscriptions, unsubscribe } from '../api/subscriptions';
import type { ApiError } from '../api/client';
import type { ActiveSubscription } from '../api/types';
import { classNames } from '../utils/classNames';
import './SubscriptionManager.css';

type PanelStatus = 'idle' | 'loading' | 'error';

export function SubscriptionManager() {
  const { t } = useTranslation();
  const [subscriptions, setSubscriptions] = useState<ActiveSubscription[]>([]);
  const [status, setStatus] = useState<PanelStatus>('idle');
  const [error, setError] = useState<string | null>(null);
  const [removingId, setRemovingId] = useState<number | null>(null);

  const loadSubscriptions = async () => {
    setStatus('loading');
    setError(null);
    try {
      const response = await listActiveSubscriptions();
      setSubscriptions(response.subscriptions);
      setStatus('idle');
    } catch (err) {
      const apiError = err as ApiError;
      setError(apiError.message);
      setStatus('error');
    }
  };

  useEffect(() => {
    void loadSubscriptions();
  }, []);

  const handleRemove = async (entry: ActiveSubscription) => {
    setRemovingId(entry.subscriptionId);
    setError(null);
    try {
      await unsubscribe({
        subscriptionId: entry.subscriptionId,
        contactValue: entry.contactValue,
        reason: 'user_request',
      });
      setSubscriptions((prev) => prev.filter((item) => item.subscriptionId !== entry.subscriptionId));
    } catch (err) {
      const apiError = err as ApiError;
      setError(apiError.message);
    } finally {
      setRemovingId(null);
    }
  };

  const isLoading = status === 'loading';
  const hasItems = subscriptions.length > 0;

  return (
    <section className="subscription-manager">
      <header className="subscription-manager__header">
        <div>
          <p className="subscription-manager__eyebrow">{t('subscriptionManager.title')}</p>
          <p className="subscription-manager__subtitle">{t('subscriptionManager.subtitle')}</p>
        </div>
        <button
          type="button"
          className="subscription-manager__refresh"
          onClick={loadSubscriptions}
          disabled={isLoading}
        >
          {isLoading ? t('common.status.loading') : t('subscriptionManager.refresh')}
        </button>
      </header>

      {error && (
        <div className="subscription-manager__feedback subscription-manager__feedback--error">
          <p>{t('subscriptionManager.error', { message: error })}</p>
        </div>
      )}

      {!hasItems && !isLoading && !error && (
        <div className="subscription-manager__empty">
          <p>{t('subscriptionManager.empty')}</p>
        </div>
      )}

      {isLoading && (
        <div className="subscription-manager__empty">
          <p>{t('common.status.loading')}</p>
        </div>
      )}

      {hasItems && (
        <ul className="subscription-manager__list">
          {subscriptions.map((entry) => {
            const contactType = entry.contactType === 'local_sound' ? 'local_sound' : 'email';
            const channelLabel =
              contactType === 'local_sound'
                ? t('subscriptionManager.channel.sound')
                : t('subscriptionManager.channel.email');

            return (
              <li key={entry.subscriptionId} className="subscription-manager__item">
                <button
                  type="button"
                  className="subscription-manager__remove"
                  aria-label={t('subscriptionManager.remove')}
                  onClick={() => handleRemove(entry)}
                  disabled={removingId === entry.subscriptionId}
                >
                  <span aria-hidden="true">-</span>
                </button>
                <div className="subscription-manager__info">
                  <div className="subscription-manager__row">
                    <span className="subscription-manager__code">{entry.sectionIndex}</span>
                    <span className="subscription-manager__meta">
                      {t('subscriptionManager.meta', { campus: entry.campus, term: entry.term })}
                    </span>
                    <span
                      className={classNames(
                        'subscription-manager__channel-pill',
                        contactType === 'local_sound'
                          ? 'subscription-manager__channel-pill--sound'
                          : 'subscription-manager__channel-pill--email',
                      )}
                    >
                      {channelLabel}
                    </span>
                  </div>
                  <p className="subscription-manager__title">
                    {entry.courseTitle ?? t('subscriptionManager.fallbackTitle')}
                  </p>
                  {entry.sectionNumber && (
                    <p className="subscription-manager__hint">
                      {entry.sectionNumber}
                      {entry.subjectCode ? ` · ${entry.subjectCode}` : ''}
                    </p>
                  )}
                </div>
                <button
                  type="button"
                  className={classNames(
                    'subscription-manager__action',
                    removingId === entry.subscriptionId && 'subscription-manager__action--disabled',
                  )}
                  onClick={() => handleRemove(entry)}
                  disabled={removingId === entry.subscriptionId}
                >
                  {removingId === entry.subscriptionId
                    ? t('subscriptionManager.removing')
                    : t('subscriptionManager.remove')}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
