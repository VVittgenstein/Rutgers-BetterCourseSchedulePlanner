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
  const suffix = [section, notice.detail].filter((value) => value !== null && value !== undefined).join(' · ');
  const title = {
    SELECTION_LIMIT: i18n.t('watch.toast.selection_limit'),
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

export function WatchToastRegion() {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  if (watch.notices.length === 0) return null;
  return (
    <section aria-label={i18n.t('watch.toast.region')} className="watch-toast-region">
      {watch.notices.map((notice) => {
        const message = noticeText(notice, i18n);
        return (
          <article
            className="watch-toast"
            data-tone={notice.tone}
            key={notice.id}
            role={notice.tone === 'ALERT' ? 'alert' : 'status'}
          >
            <div>
              <h2 className="watch-toast__title">{message.title}</h2>
              {message.detail.length === 0 ? null : <p className="watch-toast__detail">{message.detail}</p>}
            </div>
            <button
              aria-label={i18n.t('watch.toast.dismiss', { title: message.title })}
              className="watch-toast__dismiss"
              onClick={() => watch.dismissNotice(notice.id)}
              type="button"
            >
              ×
            </button>
          </article>
        );
      })}
    </section>
  );
}
