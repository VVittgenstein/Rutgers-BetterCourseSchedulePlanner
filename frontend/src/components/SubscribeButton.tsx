import { useEffect, useMemo, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import type { ApiError } from '../api/client';
import { subscribe, unsubscribe } from '../api/subscriptions';
import type {
  SubscribeResponsePayload,
  UnsubscribeResponsePayload,
} from '../api/types';
import type { CourseSectionPreview } from '../hooks/useCourseQuery';
import { classNames } from '../utils/classNames';
import './SubscribeButton.css';

const CONTACT_STORAGE_KEY = 'bcsp:subscriptionContact';
const CLICK_COOLDOWN_MS = 600;

type FeedbackTone = 'success' | 'warning' | 'error' | 'info';

interface Feedback {
  tone: FeedbackTone;
  message: string;
  traceId?: string;
}

interface SubscribeButtonProps {
  term: string;
  campus: string;
  sections: CourseSectionPreview[];
  courseTitle: string;
  courseCode: string;
}

const defaultContact = (): { contactValue: string; token?: string } => {
  if (typeof window === 'undefined') return { contactValue: '' };
  try {
    const raw = window.localStorage.getItem(CONTACT_STORAGE_KEY);
    if (!raw) return { contactValue: '' };
    const parsed = JSON.parse(raw) as { contactValue?: string; token?: string };
    return {
      contactValue: parsed.contactValue ?? '',
      token: parsed.token,
    };
  } catch {
    return { contactValue: '' };
  }
};

export function SubscribeButton({ term, campus, sections, courseTitle, courseCode }: SubscribeButtonProps) {
  const { t, i18n } = useTranslation();
  const [sectionIndex, setSectionIndex] = useState<string>(() => sections[0]?.index ?? '');
  const [contactValue, setContactValue] = useState<string>(() => defaultContact().contactValue ?? '');
  const [unsubscribeToken, setUnsubscribeToken] = useState<string>(() => defaultContact().token ?? '');
  const [recentSubscriptionId, setRecentSubscriptionId] = useState<number | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busyAction, setBusyAction] = useState<'subscribe' | 'unsubscribe' | null>(null);
  const lastClickRef = useRef(0);

  useEffect(() => {
    if (typeof window === 'undefined') return;
    try {
      window.localStorage.setItem(
        CONTACT_STORAGE_KEY,
        JSON.stringify({ contactValue, token: unsubscribeToken || undefined }),
      );
    } catch {
      // Best-effort persistence; ignore storage errors.
    }
  }, [contactValue, unsubscribeToken]);

  useEffect(() => {
    const defaultSection = sections[0]?.index;
    if (!sectionIndex && defaultSection) {
      setSectionIndex(defaultSection);
    }
  }, [sections, sectionIndex]);

  const sectionOptions = useMemo(
    () =>
      sections.map((section) => ({
        index: section.index,
        label: `${section.sectionNumber ?? section.index} · ${section.index}`,
        status: formatStatus(section.isOpen ? 'OPEN' : section.openStatus, t),
        instructor: section.instructors[0] ?? t('courseCard.meta.instructorFallback'),
        meeting: formatMeeting(section.meetings[0]),
        isOpen: section.isOpen,
      })),
    [sections, t],
  );

  const selectedLabel = useMemo(() => {
    const current = sectionOptions.find((entry) => entry.index === sectionIndex);
    return current ? `${current.label} · ${current.instructor}` : '';
  }, [sectionIndex, sectionOptions]);

  const guardDoubleClick = () => {
    const now = Date.now();
    if (now - lastClickRef.current < CLICK_COOLDOWN_MS) {
      return false;
    }
    lastClickRef.current = now;
    return true;
  };

  const handleSubscribe = async () => {
    const trimmedIndex = sectionIndex.trim();
    const trimmedContact = contactValue.trim();
    if (!trimmedIndex) {
      setFeedback({ tone: 'error', message: t('courseCard.subscribe.errors.missingSection') });
      return;
    }
    if (!trimmedContact) {
      setFeedback({ tone: 'error', message: t('courseCard.subscribe.errors.missingContact') });
      return;
    }

    if (busyAction || !guardDoubleClick()) return;
    setBusyAction('subscribe');
    setFeedback(null);

    try {
      const response = await subscribe(
        {
          term,
          campus,
          sectionIndex: trimmedIndex,
          contactType: 'email',
          contactValue: trimmedContact,
          locale: i18n.language,
        },
        undefined,
      );
      handleSubscribeSuccess(response);
    } catch (error) {
      handleError(error as ApiError);
    } finally {
      setBusyAction(null);
    }
  };

  const handleSubscribeSuccess = (payload: SubscribeResponsePayload) => {
    setRecentSubscriptionId(payload.subscriptionId);
    if (payload.unsubscribeToken) {
      setUnsubscribeToken(payload.unsubscribeToken);
    }

    const messages: string[] = [];
    if (payload.existing) {
      messages.push(t('courseCard.subscribe.status.existing'));
    } else {
      messages.push(t('courseCard.subscribe.status.created'));
    }
    if (payload.requiresVerification) {
      messages.push(t('courseCard.subscribe.status.requiresVerification'));
    }
    if (!payload.sectionResolved) {
      messages.push(t('courseCard.subscribe.status.unresolved'));
    }

    setFeedback({
      tone: payload.existing ? 'info' : 'success',
      message: messages.join(' '),
      traceId: payload.traceId,
    });
  };

  const handleUnsubscribe = async () => {
    const trimmedToken = unsubscribeToken.trim();
    const trimmedContact = contactValue.trim();
    const useToken = Boolean(trimmedToken);
    const useId = recentSubscriptionId !== null;

    if (!useToken && !useId) {
      setFeedback({ tone: 'error', message: t('courseCard.subscribe.errors.missingToken') });
      return;
    }

    if (busyAction || !guardDoubleClick()) return;
    setBusyAction('unsubscribe');
    setFeedback(null);

    try {
      const response = await unsubscribe(
        {
          subscriptionId: useId ? recentSubscriptionId ?? undefined : undefined,
          unsubscribeToken: useToken ? trimmedToken : undefined,
          contactValue: trimmedContact || undefined,
          reason: 'user_request',
        },
        undefined,
      );
      handleUnsubscribeSuccess(response);
    } catch (error) {
      handleError(error as ApiError);
    } finally {
      setBusyAction(null);
    }
  };

  const handleUnsubscribeSuccess = (payload: UnsubscribeResponsePayload) => {
    setFeedback({
      tone: 'success',
      message: t('courseCard.subscribe.status.unsubscribed', { previous: payload.previousStatus }),
      traceId: payload.traceId,
    });
  };

  const handleError = (error: ApiError) => {
    const details = Array.isArray(error.details) ? error.details[0] : undefined;
    setFeedback({
      tone: 'error',
      message: details ?? error.message,
      traceId: error.traceId,
    });
  };

  const placeholder = t('courseCard.subscribe.contactPlaceholder.email');

  const panelLabel = `${t('courseCard.subscribe.title')} · ${courseCode} ${courseTitle}`;
  const useScrollableList = sectionOptions.length > 12;

  return (
    <div className="subscribe-panel" aria-label={panelLabel}>
      <div className="subscribe-panel__header">
        <div>
          <p className="subscribe-panel__eyebrow">{t('courseCard.subscribe.title')}</p>
          <p className="subscribe-panel__hint">{t('courseCard.subscribe.helper')}</p>
        </div>
        <span className="subscribe-panel__badge">
          {campus} · {term}
        </span>
      </div>

      {sectionOptions.length > 0 && (
        <div
          className={classNames(
            'subscribe-panel__sections',
            useScrollableList && 'subscribe-panel__sections--list',
          )}
        >
          {sectionOptions.map((entry) => (
            <button
              key={entry.index}
              type="button"
              className={classNames(
                'subscribe-panel__section',
                entry.isOpen && 'subscribe-panel__section--open',
                entry.index === sectionIndex && 'subscribe-panel__section--active',
              )}
              onClick={() => setSectionIndex(entry.index)}
              aria-pressed={entry.index === sectionIndex}
            >
              <span className="subscribe-panel__section-code">{entry.label}</span>
              <span className="subscribe-panel__section-status">{entry.status}</span>
              {entry.meeting && <span className="subscribe-panel__section-meta">{entry.meeting}</span>}
              <span className="subscribe-panel__section-meta">{entry.instructor}</span>
            </button>
          ))}
        </div>
      )}

      <div className="subscribe-panel__field">
        <label className="subscribe-panel__label" htmlFor={`subscribe-section-${courseCode}`}>
          {t('courseCard.subscribe.sectionLabel')}
        </label>
        <input
          id={`subscribe-section-${courseCode}`}
          className="subscribe-panel__input"
          list={`subscription-sections-${courseCode}`}
          value={sectionIndex}
          onChange={(event) => setSectionIndex(event.target.value)}
          placeholder={t('courseCard.subscribe.sectionPlaceholder')}
        />
        <datalist id={`subscription-sections-${courseCode}`}>
          {sectionOptions.map((option) => (
            <option key={option.index} value={option.index} label={option.label} />
          ))}
        </datalist>
        {selectedLabel && <p className="subscribe-panel__hint">{selectedLabel}</p>}
      </div>

      <div className="subscribe-panel__field">
        <span className="subscribe-panel__label">{t('courseCard.subscribe.contactLabel')}</span>
        <div className="subscribe-panel__contact">
          <input
            className="subscribe-panel__input"
            value={contactValue}
            onChange={(event) => setContactValue(event.target.value)}
            placeholder={placeholder}
            autoComplete="email"
          />
          <p className="subscribe-panel__hint">{t('courseCard.subscribe.contactHint')}</p>
        </div>
      </div>

      <div className="subscribe-panel__field">
        <label className="subscribe-panel__label" htmlFor={`unsubscribe-token-${courseCode}`}>
          {t('courseCard.subscribe.tokenLabel')}
        </label>
        <input
          id={`unsubscribe-token-${courseCode}`}
          className="subscribe-panel__input"
          value={unsubscribeToken}
          onChange={(event) => setUnsubscribeToken(event.target.value)}
          placeholder={t('courseCard.subscribe.tokenPlaceholder')}
        />
      </div>

      {feedback && (
        <div className={classNames('subscribe-panel__feedback', `subscribe-panel__feedback--${feedback.tone}`)}>
          <p className="subscribe-panel__feedback-message">{feedback.message}</p>
          {feedback.traceId && (
            <p className="subscribe-panel__feedback-trace">
              {t('courseCard.subscribe.traceId', { id: feedback.traceId })}
            </p>
          )}
        </div>
      )}

      <div className="subscribe-panel__actions">
        <button
          type="button"
          className="subscribe-panel__action-btn"
          onClick={handleSubscribe}
          disabled={busyAction === 'subscribe'}
        >
          {busyAction === 'subscribe' ? t('common.status.loading') : t('courseCard.subscribe.actions.subscribe')}
        </button>
        <button
          type="button"
          className="subscribe-panel__action-btn subscribe-panel__action-btn--ghost"
          onClick={handleUnsubscribe}
          disabled={busyAction === 'unsubscribe'}
        >
          {busyAction === 'unsubscribe' ? t('common.status.refreshing') : t('courseCard.subscribe.actions.unsubscribe')}
        </button>
      </div>
    </div>
  );
}

function formatStatus(openStatus: string | null, translate: TFunction<'translation'>) {
  const normalized = openStatus?.toUpperCase() ?? '';
  if (normalized.includes('WAIT')) return translate('courseCard.subscribe.sectionStatus.waitlist');
  if (normalized.includes('OPEN')) return translate('courseCard.subscribe.sectionStatus.open');
  return translate('courseCard.subscribe.sectionStatus.closed');
}

function formatMeeting(meeting: CourseSectionPreview['meetings'][number] | undefined) {
  if (!meeting) return null;
  const time = [meeting.startMinutes, meeting.endMinutes].every((value) => typeof value === 'number')
    ? `${formatMinutes(meeting.startMinutes)}–${formatMinutes(meeting.endMinutes)}`
    : null;
  if (meeting.day && time) return `${meeting.day} ${time}`;
  if (meeting.day) return meeting.day;
  if (time) return time;
  return null;
}

function formatMinutes(value: number) {
  const hours = Math.floor(value / 60)
    .toString()
    .padStart(2, '0');
  const minutes = (value % 60).toString().padStart(2, '0');
  return `${hours}:${minutes}`;
}
