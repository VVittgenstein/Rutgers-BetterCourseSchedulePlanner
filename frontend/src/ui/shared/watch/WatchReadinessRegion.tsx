import { useEffect, useRef } from 'react';


import { useBcspI18n, type BcspI18nRuntime } from '../i18n/runtime';
import { useLiveWatch, type LiveWatchValue } from './LiveWatchProvider';
import type { WatchNotificationRequest } from './notification';
import type { WatchReadinessAction, WatchReadinessReason } from './readiness';

function reasonText(reason: WatchReadinessReason, i18n: BcspI18nRuntime): string {
  return {
    NOT_WATCHING: i18n.t('watch.readiness.reason.not_watching'),
    DISCONNECTED: i18n.t('watch.readiness.reason.disconnected'),
    RECONNECTING: i18n.t('watch.readiness.reason.reconnecting'),
    CONTACT_STALE: i18n.t('watch.readiness.reason.contact_stale'),
    INTENT_UNREADABLE: i18n.t('watch.readiness.reason.intent_unreadable'),
    PREPARING: i18n.t('watch.readiness.reason.preparing'),
    AUDIO_BLOCKED: i18n.t('watch.readiness.reason.audio_blocked'),
    AUDIO_FAILED: i18n.t('watch.readiness.reason.audio_failed'),
    AUDIO_LOCKED: i18n.t('watch.readiness.reason.audio_locked'),
    NO_FALLBACK_WHILE_HIDDEN: i18n.t('watch.readiness.reason.hidden_without_fallback'),
    MUTED: i18n.t('watch.readiness.reason.muted'),
  }[reason];
}

function actionText(action: WatchReadinessAction, i18n: BcspI18nRuntime): string | null {
  return {
    NONE: null,
    WAIT: null,
    RECONNECT: i18n.t('watch.readiness.action.reconnect'),
    ENABLE_SOUND: i18n.t('watch.readiness.action.enable_sound'),
    ALLOW_NOTIFICATIONS: i18n.t('watch.readiness.action.allow_notifications'),
    UNMUTE: i18n.t('watch.readiness.action.unmute'),
  }[action];
}

function runAction(action: WatchReadinessAction, watch: LiveWatchValue): void {
  if (action === 'RECONNECT') {
    watch.reconnect();
    return;
  }
  if (action === 'ENABLE_SOUND') {
    void watch.enableSound();
    return;
  }
  if (action === 'ALLOW_NOTIFICATIONS') {
    // Pressing this IS the user activation the browser demands, so the ask
    // happens on this line and nowhere further down a promise chain.
    watch.setNotificationsEnabled(true);
    watch.requestNotificationPermission();
    return;
  }
  if (action === 'UNMUTE') watch.setMuted(false);
}

/**
 * The readiness line, resident on every route.
 *
 * It states one of three things and never a fourth: this page will ring, this
 * page will not ring and here is the first reason why, or nothing is being
 * watched. It is deliberately NOT a toast -- a status that disappears after
 * five seconds is exactly how a page ends up looking fine while it is not,
 * and this is the one surface that must outlive the user's attention.
 */
export function WatchReadinessRegion() {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  const { readiness } = watch;
  const bannerRef = useRef<HTMLElement | null>(null);
  const stopped = readiness.level === 'STOPPED';

  // The banner is sticky under the app bar on every route, so anything else that
  // sticks to the top of the viewport (the search rail, the results header) has to
  // clear it. Publishing the measured height keeps that offset honest while the
  // banner stacks on phones or grows a second line of reason text.
  useEffect(() => {
    const root = globalThis.document?.documentElement;
    if (root === undefined) return undefined;
    const banner = bannerRef.current;
    if (stopped || banner === null) {
      root.style.removeProperty('--bcsp-readiness-height');
      return undefined;
    }
    const measure = () => {
      const height = Math.max(0, banner.getBoundingClientRect().height);
      root.style.setProperty('--bcsp-readiness-height', `${height}px`);
    };
    measure();
    globalThis.addEventListener('resize', measure);
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    observer?.observe(banner);
    return () => {
      observer?.disconnect();
      globalThis.removeEventListener('resize', measure);
      root.style.removeProperty('--bcsp-readiness-height');
    };
  }, [stopped]);

  if (stopped) return null;
  const ready = readiness.level === 'READY';
  const reason = readiness.reason === null ? null : reasonText(readiness.reason, i18n);
  const action = actionText(readiness.action, i18n);
  const summary = ready
    ? (readiness.fallbackInUse
      ? i18n.t('watch.readiness.ready_via_notification')
      : i18n.t('watch.readiness.ready'))
    : i18n.t('watch.readiness.degraded');
  return (
    <section
      aria-label={i18n.t('watch.readiness.region')}
      className="watch-readiness"
      data-bcsp-watch-readiness={readiness.level}
      data-broken-ring={readiness.brokenRing ?? undefined}
      ref={bannerRef}
      role="status"
    >
      <p className="watch-readiness__summary">
        <span className="watch-readiness__badge" data-level={readiness.level}>{summary}</span>
        {reason === null ? null : <span className="watch-readiness__reason">{reason}</span>}
      </p>
      {action === null ? null : (
        <button
          className="watch-readiness__action"
          onClick={() => runAction(readiness.action, watch)}
          type="button"
        >
          {action}
        </button>
      )}
    </section>
  );
}

function notificationText(
  request: WatchNotificationRequest,
  i18n: BcspI18nRuntime,
): { readonly title: string; readonly body: string } {
  const section = request.sectionKey === undefined
    ? null
    : `${request.sectionKey.index} / ${request.sectionKey.term} / ${request.sectionKey.campus}`;
  if (request.kind === 'MONITORING_DEGRADED') {
    return {
      title: i18n.t('watch.notification.degraded_title'),
      body: i18n.t('watch.notification.degraded_body'),
    };
  }
  const title = request.kind === 'SECTION_OPEN'
    ? i18n.t('watch.notification.open_title')
    : i18n.t('watch.notification.cue_failed_title');
  const body = request.kind === 'SECTION_OPEN'
    ? i18n.t('watch.notification.open_body', { section: section ?? '' })
    : i18n.t('watch.notification.cue_failed_body', { section: section ?? '' });
  return { title, body };
}

/**
 * Posts the page-level notifications the provider decided are warranted.
 *
 * Rendering nothing is the point: the decision, the deduplication and the
 * permission are all settled before a request reaches here, so this is only
 * the step that puts the words in the user's language on it.
 */
export function WatchNotificationRegion() {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  const { consumeNotification, notificationPort, notifications } = watch;
  // The i18n runtime changes identity on a locale switch, and the queue is
  // rewritten on every post. Keeping the latest of each in a ref lets the
  // effect depend on the QUEUE alone, so a language change cannot re-post
  // something the user has already been told.
  const localize = useRef(i18n);
  localize.current = i18n;
  useEffect(() => {
    for (const request of notifications) {
      const taken = consumeNotification(request.id);
      if (taken === null) continue;
      const message = notificationText(taken, localize.current);
      notificationPort.show(message.title, message.body, `bcsp:${taken.subject ?? taken.kind}`);
    }
  }, [consumeNotification, notificationPort, notifications]);
  return null;
}
