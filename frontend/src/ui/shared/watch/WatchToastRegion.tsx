import type { SectionKey } from '../product';
import { useLiveWatch, type WatchNotice } from './LiveWatchProvider';

function sectionLabel(sectionKey: SectionKey | undefined): string | null {
  return sectionKey === undefined
    ? null
    : `${sectionKey.index} / ${sectionKey.term} / ${sectionKey.campus}`;
}

function noticeText(notice: WatchNotice): { readonly title: string; readonly detail: string } {
  const section = sectionLabel(notice.sectionKey);
  const suffix = [section, notice.detail].filter((value) => value !== null && value !== undefined).join(' · ');
  const title = {
    SELECTION_LIMIT: 'Selection limit reached',
    START_REJECTED: 'Watch could not start',
    COMMAND_FAILED: 'Watch command failed',
    CONNECTION_LOST: 'Watch connection ended',
    WATCH_STOPPED: 'Watch stopped',
    AUDIO_BLOCKED: 'Browser blocked sound',
    AUDIO_FAILED: 'Sound playback failed',
    AUDIO_CAP_REACHED: 'Audible limit reached — watch remains active',
    AUDIO_CUE_QUEUED: 'Sound cue queued',
    WATCH_ALERT_OPEN: 'A watched Section is open',
  }[notice.code];
  return { title, detail: suffix };
}

export function WatchToastRegion() {
  const watch = useLiveWatch();
  if (watch.notices.length === 0) return null;
  return (
    <section aria-label="Watch notifications" className="watch-toast-region">
      {watch.notices.map((notice) => {
        const message = noticeText(notice);
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
              aria-label={`Dismiss ${message.title}`}
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
