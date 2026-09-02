import { useEffect } from 'react';

import { useBcspI18n } from '../../shared/i18n/runtime';
import {
  episodeStateMessageKeys,
  watchNotificationMessageKeys,
} from '../../shared/i18n/presenter';
import { useLocalI18n } from '../i18n/runtime';
import type {
  EpisodeDisposition,
  EpisodeHistorySummary,
  HistoryPage as HistoryPageData,
  WatchStopReason,
} from '../personal/contracts';
import { LocalPageFrame, type LocalPageAsyncState } from './PageFrame';

const HISTORY_LIMIT = 50;

export interface HistoryPageProps extends LocalPageAsyncState {
  readonly history: HistoryPageData<EpisodeHistorySummary>;
}

function safeDate(
  value: number,
  formatDate: (value: Date | number, options?: Intl.DateTimeFormatOptions) => string,
): string {
  try {
    return formatDate(value, { dateStyle: 'medium', timeStyle: 'medium' });
  } catch {
    return String(value);
  }
}

function watchStopLabel(reason: WatchStopReason, chinese: boolean): string {
  const labels: Readonly<Record<WatchStopReason, readonly [string, string]>> = {
    USER_REQUESTED: ['Stopped by user', '由用户停止'],
    CONNECTION_CLOSED: ['Connection closed', '连接已关闭'],
    HEARTBEAT_TIMEOUT: ['Connection heartbeat timed out', '连接心跳超时'],
    SERVICE_STOPPING: ['Local service stopped', '本地服务已停止'],
    UNSUPPORTED_TARGET: ['Campus is no longer supported', 'Campus 已不受支持'],
    TERM_OUT_OF_RANGE: ['Term moved outside the watch window', '学期已移出可监看范围'],
  };
  return labels[reason][chinese ? 1 : 0];
}

function dispositionLabel(
  disposition: EpisodeDisposition | null,
  chinese: boolean,
  shared: ReturnType<typeof useBcspI18n>,
  activeLabel: string,
): string {
  if (disposition === null) return activeLabel;
  switch (disposition.kind) {
    case 'SECTION_CLOSED':
      return shared.t(episodeStateMessageKeys.CLOSED);
    case 'ACKNOWLEDGED':
      return shared.t(episodeStateMessageKeys.ACKNOWLEDGED);
    case 'TIMED_OUT':
      return shared.t(episodeStateMessageKeys.TIMED_OUT);
    case 'WATCH_STOPPED':
      return watchStopLabel(disposition.reason, chinese);
  }
}

function stateTone(state: EpisodeHistorySummary['state']): string {
  switch (state) {
    case 'UNACKNOWLEDGED':
    case 'TIMED_OUT':
      return 'DANGER';
    case 'ACKNOWLEDGED':
      return 'READY';
    case 'CLOSED':
      return 'CLEAN';
  }
}

export function HistoryPage({
  error,
  history,
  notice,
  onClearError,
  onReload,
  pending = false,
}: HistoryPageProps) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  const chinese = local.locale === 'zh-CN';
  const recent = [...history.items]
    .sort((left, right) => right.lastObservedAt - left.lastObservedAt)
    .slice(0, HISTORY_LIMIT);

  useEffect(() => {
    if (onReload === undefined) return;
    void Promise.resolve().then(() => onReload()).catch(() => undefined);
  }, [onReload]);

  return (
    <LocalPageFrame
      {...(error === undefined ? {} : { error })}
      intro={local.t('local.history.intro')}
      {...(notice === undefined ? {} : { notice })}
      {...(onClearError === undefined ? {} : { onClearError })}
      {...(onReload === undefined ? {} : { onReload })}
      pending={pending}
      title={local.t('local.history.title')}
    >
      <section className="local-personal__section">
        <header className="local-personal__section-head">
          <span className="local-personal__meta">
            {local.t('local.history.recent', {
              count: local.formatNumber(recent.length),
              total: local.formatNumber(history.total),
            })}
          </span>
        </header>
        {recent.length === 0 ? (
          <p className="local-personal__notice">{local.t('local.history.empty')}</p>
        ) : (
          <ol className="local-personal__history local-personal__list">
            {recent.map((episode) => {
              const { sectionKey } = episode.identity;
              return (
                <li className="local-personal__card" key={episode.identity.episodeId}>
                  <div className="local-personal__identity">
                    <div>
                      <p className="local-personal__meta">
                        {chinese ? '课节' : 'Section'} <samp>{sectionKey.index}</samp>
                      </p>
                      <h4>{sectionKey.term} · {sectionKey.campus}</h4>
                    </div>
                    <span className="local-personal__badge" data-state={stateTone(episode.state)}>
                      {shared.t(episodeStateMessageKeys[episode.state])}
                    </span>
                  </div>
                  <div className="local-page__history-state">
                    <span className="local-personal__badge" data-state="MODE">
                      {shared.t(watchNotificationMessageKeys[episode.mode])}
                    </span>
                    <span className="local-personal__meta">
                      {local.t('local.history.observations', {
                        count: local.formatNumber(episode.observationCount),
                      })}
                    </span>
                  </div>
                  <dl className="local-personal__facts">
                    <div>
                      <dt>{local.t('local.history.first_seen')}</dt>
                      <dd>{safeDate(episode.firstObservedAt, local.formatDate)}</dd>
                    </div>
                    <div>
                      <dt>{local.t('local.history.last_seen')}</dt>
                      <dd>{safeDate(episode.lastObservedAt, local.formatDate)}</dd>
                    </div>
                    <div>
                      <dt>{local.t('local.history.disposition')}</dt>
                      <dd>{dispositionLabel(
                        episode.disposition,
                        chinese,
                        shared,
                        local.t('local.history.active'),
                      )}</dd>
                    </div>
                    <div>
                      <dt>{chinese ? '活动计数' : 'Activity counts'}</dt>
                      <dd>
                        {local.t('local.history.actions', {
                          count: local.formatNumber(episode.actionCount),
                        })}
                        {' / '}
                        {local.t('local.history.audible', {
                          count: local.formatNumber(episode.audibleCount),
                        })}
                      </dd>
                    </div>
                  </dl>
                </li>
              );
            })}
          </ol>
        )}
      </section>
    </LocalPageFrame>
  );
}
