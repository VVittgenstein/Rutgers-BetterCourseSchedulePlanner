import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { subscribe } from '../api/subscriptions';
import type { ApiError } from '../api/client';
import type { SubscriptionContactType } from '../api/types';
import { useLocalSoundNotifications } from '../hooks/useLocalSoundNotifications';
import { classNames } from '../utils/classNames';
import { LocalSoundToggle } from './LocalSoundToggle';
import './SubscriptionCenter.css';

const CONTACT_STORAGE_KEY = 'bcsp:subscriptionContact';
const CONTACT_TYPE_STORAGE_KEY = 'bcsp:subscriptionContactType';

type FeedbackTone = 'success' | 'info' | 'error';

interface Feedback {
  tone: FeedbackTone;
  message: string;
}

interface SubscriptionCenterProps {
  term?: string;
  campus?: string;
}

const loadContact = (): { contactValue: string; contactType: SubscriptionContactType } => {
  if (typeof window === 'undefined') return { contactValue: '', contactType: 'email' };
  try {
    const raw = window.localStorage.getItem(CONTACT_STORAGE_KEY);
    const storedType = window.localStorage.getItem(CONTACT_TYPE_STORAGE_KEY);
    const contactType = storedType === 'local_sound' || storedType === 'email' ? storedType : 'email';
    if (!raw) return { contactValue: '', contactType };
    const parsed = JSON.parse(raw) as { contactValue?: string };
    return {
      contactValue: parsed.contactValue ?? '',
      contactType,
    };
  } catch {
    return { contactValue: '', contactType: 'email' };
  }
};

export function SubscriptionCenter({ term, campus }: SubscriptionCenterProps) {
  const { t, i18n } = useTranslation();
  const [sectionIndex, setSectionIndex] = useState('');
  const [contactValue, setContactValue] = useState<string>(() => loadContact().contactValue);
  const [contactType, setContactType] = useState<SubscriptionContactType>(() => loadContact().contactType);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busy, setBusy] = useState(false);
  const localSound = useLocalSoundNotifications();

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      const raw = window.localStorage.getItem(CONTACT_STORAGE_KEY);
      const parsed = raw ? JSON.parse(raw) : {};
      window.localStorage.setItem(CONTACT_STORAGE_KEY, JSON.stringify({ ...parsed, contactValue }));
    } catch {
      // Best effort only.
    }
  }, [contactValue]);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(CONTACT_TYPE_STORAGE_KEY, contactType);
    } catch {
      // Best effort only.
    }
  }, [contactType]);

  const missingContext = !term || !campus;
  const contactPlaceholder = t('courseCard.subscribe.contactPlaceholder.email');

  const handleSubscribe = async () => {
    setFeedback(null);
    if (missingContext) {
      setFeedback({ tone: 'error', message: t('subscriptionCenter.errors.missingContext') });
      return;
    }

    const trimmedIndex = sectionIndex.trim();
    const trimmedContact = contactValue.trim();
    const isLocalSound = contactType === 'local_sound';
    if (!trimmedIndex) {
      setFeedback({ tone: 'error', message: t('subscriptionCenter.errors.missingSection') });
      return;
    }
    if (!isLocalSound && !trimmedContact) {
      setFeedback({ tone: 'error', message: t('subscriptionCenter.errors.missingContact') });
      return;
    }
    if (isLocalSound && !localSound.deviceId) {
      setFeedback({ tone: 'error', message: t('subscriptionCenter.errors.missingDevice') });
      return;
    }

    const contactForSubmit = isLocalSound ? localSound.deviceId : trimmedContact;

    setBusy(true);
    try {
      const response = await subscribe(
        {
          term: term!,
          campus: campus!,
          sectionIndex: trimmedIndex,
          contactType,
          contactValue: contactForSubmit,
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
      if (isLocalSound) {
        void localSound.enable();
      }
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
        <div className="subscription-center__contact-types">
          <button
            type="button"
            className={classNames(
              'subscription-center__pill',
              contactType === 'email' && 'subscription-center__pill--active',
            )}
            onClick={() => setContactType('email')}
          >
            {t('subscriptionCenter.contactTypes.email')}
          </button>
          <button
            type="button"
            className={classNames(
              'subscription-center__pill',
              contactType === 'local_sound' && 'subscription-center__pill--active',
            )}
            onClick={() => setContactType('local_sound')}
          >
            {t('subscriptionCenter.contactTypes.sound')}
          </button>
        </div>
        <div className="subscription-center__contact">
          {contactType === 'email' ? (
            <>
              <input
                type="email"
                value={contactValue}
                onChange={(event) => setContactValue(event.target.value)}
                placeholder={contactPlaceholder}
              />
              <p className="subscription-center__hint">{t('courseCard.subscribe.contactHint')}</p>
            </>
          ) : (
            <div className="subscription-center__device">
              <div>
                <p className="subscription-center__device-label">
                  {t('subscriptionCenter.localSound.deviceLabel')}
                </p>
                <code className="subscription-center__device-value">{localSound.deviceId}</code>
              </div>
              <button
                type="button"
                className="subscription-center__pill"
                onClick={localSound.regenerateDeviceId}
              >
                {t('subscriptionCenter.localSound.actions.resetDevice')}
              </button>
            </div>
          )}
        </div>
        {contactType === 'local_sound' && (
          <p className="subscription-center__hint subscription-center__hint--muted">
            {t('subscriptionCenter.localSound.deviceHint')}
          </p>
        )}
      </div>

      <LocalSoundToggle controls={localSound} />

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
