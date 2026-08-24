import { useEffect, useRef, useState } from 'react';

import type { SectionKey } from '../product';
import { useBcspI18n, type BcspI18nRuntime } from '../i18n/runtime';
import { useLiveWatch, type WatchNotice } from './LiveWatchProvider';

function sectionLabel(sectionKey: SectionKey | undefined): string | null {
  return sectionKey === undefined
    ? null
    : `${sectionKey.index} / ${sectionKey.term} / ${sectionKey.campus}`;
}

function noticeText(
  notice: WatchNotice,
  i18n: BcspI18nRuntime,
): { readonly title: string; readonly detail: string } {
  const section = sectionLabel(notice.sectionKey);
  const detail = notice.detail === 'TERM_OUT_OF_RANGE'
    ? i18n.t('watch.term_out_of_range_detail')
    : notice.detail === 'UNSUPPORTED_TARGET'
      ? i18n.t('watch.unsupported_target_detail')
      : notice.code === 'SELECTION_BLOCKED'
        ? i18n.t('watch.intent.remove_blocked')
        : notice.detail;
  const suffix = [section, detail].filter((value) => value !== null && value !== undefined).join(' · ');
  const title = {
    SELECTION_LIMIT: i18n.t('watch.toast.selection_limit'),
    SELECTION_BLOCKED: i18n.t('watch.toast.selection_blocked'),
    TERM_OUT_OF_RANGE: i18n.t('watch.toast.term_out_of_range'),
    START_REJECTED: i18n.t('watch.toast.start_rejected'),
    COMMAND_FAILED: i18n.t('watch.toast.command_failed'),
    CONNECTION_LOST: i18n.t('watch.toast.connection_lost'),
    WATCH_STOPPED: i18n.t('watch.toast.stopped'),
    AUDIO_BLOCKED: i18n.t('watch.toast.audio_blocked'),
    AUDIO_FAILED: i18n.t('watch.toast.audio_failed'),
    AUDIO_CAP_REACHED: i18n.t('watch.toast.audio_cap'),
    AUDIO_CUE_QUEUED: i18n.t('watch.toast.audio_queued'),
    WATCH_ALERT_OPEN: i18n.t('watch.toast.open'),
  }[notice.code];
  return { title, detail: suffix };
}

const STATUS_VISIBLE_MILLISECONDS = 5_000;
const EXIT_SETTLE_MILLISECONDS = 180;

interface WatchToastProps {
  readonly dismiss: (id: number) => void;
  readonly i18n: BcspI18nRuntime;
  readonly notice: WatchNotice;
}

function WatchToast({ dismiss, i18n, notice }: WatchToastProps) {
  const message = noticeText(notice, i18n);
  const [phase, setPhase] = useState<'VISIBLE' | 'EXITING'>('VISIBLE');
  const [hovered, setHovered] = useState(false);
  const [focusWithin, setFocusWithin] = useState(false);
  const [documentHidden, setDocumentHidden] = useState(() => document.hidden);
  const remaining = useRef(STATUS_VISIBLE_MILLISECONDS);
  const paused = hovered || focusWithin || documentHidden;

  useEffect(() => {
    const onVisibilityChange = () => setDocumentHidden(document.hidden);
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => document.removeEventListener('visibilitychange', onVisibilityChange);
  }, []);

  useEffect(() => {
    if (notice.tone !== 'STATUS' || phase !== 'VISIBLE' || paused) return undefined;
    const startedAt = Date.now();
    const timeout = globalThis.setTimeout(() => setPhase('EXITING'), remaining.current);
    return () => {
      globalThis.clearTimeout(timeout);
      remaining.current = Math.max(0, remaining.current - (Date.now() - startedAt));
    };
  }, [notice.tone, paused, phase]);

  useEffect(() => {
    if (phase !== 'EXITING') return undefined;
    const timeout = globalThis.setTimeout(() => dismiss(notice.id), EXIT_SETTLE_MILLISECONDS);
    return () => globalThis.clearTimeout(timeout);
  }, [dismiss, notice.id, phase]);

  return (
    <article
      className="watch-toast"
      data-paused={paused || undefined}
      data-state={phase}
      data-tone={notice.tone}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocusWithin(false);
      }}
      onFocusCapture={() => setFocusWithin(true)}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      role={notice.tone === 'ALERT' ? 'alert' : 'status'}
    >
      <div>
        <h2 className="watch-toast__title">{message.title}</h2>
        {message.detail.length === 0 ? null : <p className="watch-toast__detail">{message.detail}</p>}
      </div>
      <button
        aria-label={i18n.t('watch.toast.dismiss', { title: message.title })}
        className="watch-toast__dismiss"
        onClick={() => setPhase('EXITING')}
        type="button"
      >
        ×
      </button>
    </article>
  );
}

export function WatchToastRegion() {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  if (watch.notices.length === 0) return null;
  return (
    <section aria-label={i18n.t('watch.toast.region')} className="watch-toast-region">
      {watch.notices.map((notice) => {
        return (
          <WatchToast
            dismiss={watch.dismissNotice}
            i18n={i18n}
            key={notice.id}
            notice={notice}
          />
        );
      })}
    </section>
  );
}
