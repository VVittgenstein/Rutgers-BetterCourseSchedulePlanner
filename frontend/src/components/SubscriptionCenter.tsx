import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { subscribe } from '../api/subscriptions';
import type { ApiError } from '../api/client';
import type { SubscriptionContactType } from '../api/types';
import { classNames } from '../utils/classNames';
import './SubscriptionCenter.css';

const CONTACT_STORAGE_KEY = 'bcsp:subscriptionContact';

type FeedbackTone = 'success' | 'info' | 'error';

interface Feedback {
  tone: FeedbackTone;
  message: string;
}

interface SubscriptionCenterProps {
  term?: string;
  campus?: string;
}

const loadContact = (): { contactType: SubscriptionContactType; contactValue: string } => {
  if (typeof window === 'undefined') return { contactType: 'email', contactValue: '' };
  try {
    const raw = window.localStorage.getItem(CONTACT_STORAGE_KEY);
    if (!raw) return { contactType: 'email', contactValue: '' };
    const parsed = JSON.parse(raw) as { contactType?: SubscriptionContactType; contactValue?: string };
    return {
      contactType: parsed.contactType ?? 'email',
      contactValue: parsed.contactValue ?? '',
    };
  } catch {
    return { contactType: 'email', contactValue: '' };
  }
};

export function SubscriptionCenter({ term, campus }: SubscriptionCenterProps) {
  const { t, i18n } = useTranslation();
  const [sectionIndex, setSectionIndex] = useState('');
  const [contactType, setContactType] = useState<SubscriptionContactType>(() => loadContact().contactType);
  const [contactValue, setContactValue] = useState<string>(() => loadContact().contactValue);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(CONTACT_STORAGE_KEY, JSON.stringify({ contactType, contactValue }));
    } catch {
      // Best effort only.
    }
  }, [contactType, contactValue]);

  const missingContext = !term || !campus;
  const contactPlaceholder =
    contactType === 'email'
      ? t('courseCard.subscribe.contactPlaceholder.email')
      : t('courseCard.subscribe.contactPlaceholder.discord');

  const handleSubscribe = async () => {
    setFeedback(null);
    if (missingContext) {
      setFeedback({ tone: 'error', message: t('subscriptionCenter.errors.missingContext') });
      return;
    }

    const trimmedIndex = sectionIndex.trim();
    const trimmedContact = contactValue.trim();
    if (!trimmedIndex) {
      setFeedback({ tone: 'error', message: t('subscriptionCenter.errors.missingSection') });
      return;
    }
    if (!trimmedContact) {
      setFeedback({ tone: 'error', message: t('subscriptionCenter.errors.missingContact') });
      return;
    }

    setBusy(true);
    try {
      const response = await subscribe(
        {
          term: term!,
          campus: campus!,
          sectionIndex: trimmedIndex,
          contactType,
          contactValue: trimmedContact,
          locale: i18n.language,
        },
        undefined,
      );
      setFeedback({
        tone: response.existing ? 'info' : 'success',
        message: response.existing
          ? t('subscriptionCenter.status.existing')
          : t('subscriptionCenter.status.created'),
      });
      setSectionIndex('');
    } catch (error) {
      const apiError = error as ApiError;
      const details = Array.isArray(apiError.details) ? apiError.details[0] : null;
      setFeedback({ tone: 'error', message: details ?? apiError.message });
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="subscription-center">
      <header className="subscription-center__header">
        <div>
          <p className="subscription-center__eyebrow">{t('subscriptionCenter.eyebrow')}</p>
          <h3 className="subscription-center__title">{t('subscriptionCenter.title')}</h3>
        </div>
        <span className="subscription-center__badge">
          {campus ?? t('subscriptionCenter.missingCampus')} · {term ?? t('subscriptionCenter.missingTerm')}
        </span>
      </header>
      <p className="subscription-center__hint">{t('subscriptionCenter.subtitle')}</p>

      <label className="subscription-center__field">
        <span>{t('subscriptionCenter.sectionLabel')}</span>
        <input
          type="text"
          value={sectionIndex}
          onChange={(event) => setSectionIndex(event.target.value)}
          placeholder={t('subscriptionCenter.sectionPlaceholder')}
          inputMode="numeric"
        />
      </label>

      <div className="subscription-center__field">
        <span>{t('subscriptionCenter.contactLabel')}</span>
        <div className="subscription-center__contact">
          <div className="subscription-center__contact-types">
            <button
              type="button"
              className={classNames(
                'subscription-center__pill',
                contactType === 'email' && 'subscription-center__pill--active',
              )}
              onClick={() => setContactType('email')}
            >
              {t('courseCard.subscribe.contactTypes.email')}
            </button>
            <button
              type="button"
              className={classNames(
                'subscription-center__pill',
                contactType === 'discord_user' && 'subscription-center__pill--active',
              )}
              onClick={() => setContactType('discord_user')}
            >
              {t('courseCard.subscribe.contactTypes.discord')}
            </button>
          </div>
          <input
            type={contactType === 'email' ? 'email' : 'text'}
            value={contactValue}
            onChange={(event) => setContactValue(event.target.value)}
            placeholder={contactPlaceholder}
          />
        </div>
      </div>

      {feedback && (
        <div className={classNames('subscription-center__feedback', `subscription-center__feedback--${feedback.tone}`)}>
          <p>{feedback.message}</p>
        </div>
      )}

      <button
        type="button"
        className="subscription-center__submit"
        onClick={handleSubscribe}
        disabled={busy || missingContext}
      >
        {busy ? t('common.status.loading') : t('subscriptionCenter.submit')}
      </button>
    </section>
  );
}
