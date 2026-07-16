import { useEffect, useMemo, useRef, useState } from 'react';

import { ActionButton, Metric, StatusSignal } from '../design-system';
import {
  circuitStateMessageKeys,
  episodeStateMessageKeys,
  freshnessMessageKeys,
  openStateMessageKeys,
  openUncertaintyMessageKeys,
  refreshClassificationMessageKeys,
  schedulerLaneMessageKeys,
  watchNotificationMessageKeys,
} from '../i18n/presenter';
import { useBcspI18n, type BcspI18nRuntime } from '../i18n/runtime';
import type {
  OpenCounterSnapshotV1,
  OpenRefreshStatusV1,
  OpenSectionStatusV1,
  SectionKey,
  WatchPolicyV1,
} from '../product';
import {
  DEFAULT_WATCH_POLICY,
  MAX_SELECTED_SECTIONS,
  useLiveWatch,
  type ActiveWatchView,
  type WatchTelemetryResourceState,
} from './LiveWatchProvider';

function sectionLabel(sectionKey: SectionKey): string {
  return `${sectionKey.index} / ${sectionKey.term} / ${sectionKey.campus}`;
}

function connectionLabel(
  state: string,
  activeCount: number,
  muted: boolean,
  i18n: BcspI18nRuntime,
): string {
  switch (state) {
    case 'CONNECTING': return i18n.t('watch.connection.connecting');
    case 'OPEN': {
      if (activeCount === 0) return i18n.t('watch.connection.open');
      const key = activeCount === 1
        ? muted ? 'watch.connection.watching_one_muted' : 'watch.connection.watching_one'
        : muted ? 'watch.connection.watching_muted' : 'watch.connection.watching';
      return i18n.t(key, { count: i18n.formatNumber(activeCount) });
    }
    case 'CLOSED': return i18n.t('watch.connection.closed');
    case 'ERROR': return i18n.t('watch.connection.error');
    default: return i18n.t('watch.connection.idle');
  }
}

function audioLabel(state: string, i18n: BcspI18nRuntime): string {
  switch (state) {
    case 'UNLOCKING': return i18n.t('audio.state.unlocking');
    case 'READY': return i18n.t('audio.state.ready');
    case 'BLOCKED': return i18n.t('audio.state.blocked');
    case 'FAILED': return i18n.t('audio.state.failed');
    default: return i18n.t('audio.state.muted');
  }
}

function formatTime(
  value: string | null,
  format: (value: number) => string,
  i18n: BcspI18nRuntime,
): string {
  if (value === null) return i18n.t('common.not_observed');
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? format(timestamp) : i18n.t('common.invalid_timestamp');
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="watch-telemetry__fact">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function countsLabel(
  snapshot: OpenCounterSnapshotV1,
  scope: 'RUN' | 'TODAY',
  i18n: BcspI18nRuntime,
): string {
  const counts = scope === 'RUN' ? snapshot.runCounts : snapshot.todayCounts;
  if (counts === undefined) return i18n.t('common.not_exposed');
  return i18n.t('watch.counter_summary', {
    attempted: i18n.formatNumber(counts.attempted),
    empty: i18n.formatNumber(counts.empty),
    failed: i18n.formatNumber(counts.failed),
    succeeded: i18n.formatNumber(counts.succeeded),
  });
}

function FreshnessBadge({ state }: { readonly state: string }) {
  const { t } = useBcspI18n();
  const key = state === 'FRESH' || state === 'STALE' || state === 'UNKNOWN'
    ? freshnessMessageKeys[state]
    : undefined;
  return <span className="watch-workspace__badge" data-state={state}>{key === undefined ? state : t(key)}</span>;
}

function BatchTelemetry({ status }: { readonly status: OpenRefreshStatusV1 }) {
  const i18n = useBcspI18n();
  const format = (value: number) => i18n.formatDate(value, { dateStyle: 'medium', timeStyle: 'medium' });
  const actual = status.scheduler.actualStartToStartIntervalMilliseconds;
  return (
    <article className="watch-telemetry__batch" data-freshness={status.freshness.state}>
      <header className="watch-telemetry__batch-head">
        <div>
          <p className="watch-workspace__kicker">{i18n.t('watch.telemetry.open_batch')}</p>
          <h4>{status.batch.term} / {status.batch.campus}</h4>
        </div>
        <FreshnessBadge state={status.freshness.state} />
      </header>
      <dl className="watch-telemetry__facts">
        <Fact label={i18n.t('watch.telemetry.last_attempt')} value={status.latestAttempt === null
          ? i18n.t('common.never_attempted')
          : `${i18n.t(refreshClassificationMessageKeys[status.latestAttempt.classification])} · ${formatTime(status.latestAttempt.completedAt, format, i18n)}`} />
        <Fact label={i18n.t('watch.telemetry.last_valid')} value={formatTime(status.lastValidObservation?.observedAt ?? null, format, i18n)} />
        <Fact label={i18n.t('watch.telemetry.requested_general')} value={`${status.scheduler.requestedGeneralIntervalSeconds}s`} />
        <Fact label={i18n.t('watch.telemetry.requested_effective')} value={`${status.scheduler.requestedEffectiveIntervalSeconds}s · ${i18n.t(schedulerLaneMessageKeys[status.scheduler.lane])}`} />
        <Fact label={i18n.t('watch.telemetry.actual_interval')} value={actual === null ? i18n.t('common.not_measured') : `${(actual / 1000).toFixed(2)}s`} />
        <Fact label={i18n.t('watch.telemetry.scheduler_lag')} value={`${status.scheduler.schedulerLagMilliseconds}ms`} />
        <Fact label={i18n.t('watch.telemetry.circuit')} value={`${i18n.t(circuitStateMessageKeys[status.circuit.state])}${status.circuit.reason === null ? '' : ` · ${status.circuit.reason}`}`} />
        <Fact label={i18n.t('watch.telemetry.circuit_retry')} value={formatTime(status.circuit.retryAt, format, i18n)} />
        <Fact label={i18n.t('watch.telemetry.run_counters')} value={countsLabel(status.counterSnapshot, 'RUN', i18n)} />
        <Fact label={i18n.t('watch.telemetry.rutgers_day', { date: status.counterSnapshot.rutgersDay })} value={countsLabel(status.counterSnapshot, 'TODAY', i18n)} />
        <Fact label={i18n.t('watch.telemetry.lkg_age')} value={status.freshness.lastKnownGoodAgeSeconds === null
          ? i18n.t('common.unknown')
          : `${status.freshness.lastKnownGoodAgeSeconds}s`} />
        <Fact label={i18n.t('watch.telemetry.uncertainty')} value={status.freshness.uncertainty === null
          ? i18n.t('common.none')
          : i18n.t(openUncertaintyMessageKeys[status.freshness.uncertainty])} />
      </dl>
    </article>
  );
}

function SectionTelemetry({ status }: { readonly status: OpenSectionStatusV1 }) {
  const i18n = useBcspI18n();
  const observedAt = formatTime(status.freshness.observedAt, (value) =>
    i18n.formatDate(value, { dateStyle: 'medium', timeStyle: 'short' }), i18n);
  return (
    <article className="watch-telemetry__section" data-freshness={status.freshness.state}>
      <div className="watch-workspace__identity">
        <strong className="watch-workspace__index">{status.sectionKey.index}</strong>
        <FreshnessBadge state={status.freshness.state} />
        <span className="watch-workspace__badge" data-state={status.state}>
          {i18n.t(openStateMessageKeys[status.state])}
        </span>
      </div>
      <p className="watch-workspace__meta">{status.sectionKey.term} / {status.sectionKey.campus}</p>
      <p className="watch-workspace__meta">{i18n.t('watch.telemetry.observed', { time: observedAt })}</p>
      <p className="watch-workspace__meta">{i18n.t('watch.telemetry.lag', {
        certainty: status.freshness.uncertainty === null
          ? i18n.t('common.certain')
          : i18n.t(openUncertaintyMessageKeys[status.freshness.uncertainty]),
        lag: i18n.formatNumber(status.schedulerLagMilliseconds),
      })}</p>
    </article>
  );
}

function resourceLabel(resource: WatchTelemetryResourceState, i18n: BcspI18nRuntime): string {
  if (resource.kind === 'SECTION' && resource.sectionKey !== null) {
    return i18n.t('watch.telemetry.resource_section', { index: resource.sectionKey.index });
  }
  const batch = resource.batch;
  return i18n.t('watch.telemetry.resource_batch', {
    campus: batch?.campus ?? i18n.t('common.unknown'),
    term: batch?.term ?? i18n.t('common.unknown'),
  });
}

function resourceTime(resource: WatchTelemetryResourceState, i18n: BcspI18nRuntime): string {
  if (resource.lastSuccessAt === null) return i18n.t('common.not_observed');
  const timestamp = Date.parse(resource.lastSuccessAt);
  return Number.isFinite(timestamp)
    ? i18n.formatDate(timestamp, { hour: '2-digit', minute: '2-digit' })
    : i18n.t('common.invalid_timestamp');
}

function TelemetryResourceState({ resource }: { readonly resource: WatchTelemetryResourceState }) {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  const lastSuccess = resourceTime(resource, i18n);
  const message = resource.loading
    ? i18n.t('watch.telemetry.resource_loading')
    : resource.error !== null && resource.availability === 'LKG'
      ? i18n.t('watch.telemetry.resource_lkg_error', { time: lastSuccess })
      : resource.error !== null
        ? i18n.t('watch.telemetry.resource_error_no_data')
        : resource.availability === 'LKG'
          ? i18n.t('watch.telemetry.resource_lkg', { time: lastSuccess })
          : resource.availability === 'ERROR_NO_DATA'
            ? i18n.t('watch.telemetry.resource_no_data')
            : i18n.t('watch.telemetry.resource_current', { time: lastSuccess });
  return (
    <article
      className="watch-telemetry__resource"
      data-availability={resource.availability}
      data-loading={resource.loading || undefined}
    >
      <div>
        <h4>{resourceLabel(resource, i18n)}</h4>
        <p aria-live="polite" className="watch-workspace__meta" role={resource.error === null ? 'status' : 'alert'}>
          {message}
        </p>
      </div>
      {resource.error === null ? null : (
        <div className="watch-telemetry__resource-actions">
          <ActionButton
            busy={resource.loading}
            busyLabel={i18n.t('watch.telemetry.resource_loading')}
            onClick={() => void watch.retryTelemetryResource(resource.key)}
            tone="quiet"
          >
            {i18n.t('watch.telemetry.retry')}
          </ActionButton>
          <details className="watch-workspace__diagnostics">
            <summary>{i18n.t('watch.diagnostics.summary')}</summary>
            <dl className="watch-workspace__diagnostic-facts">
              <Fact label={i18n.t('watch.telemetry.http_status')} value={resource.error.httpStatus === null
                ? i18n.t('common.not_exposed')
                : i18n.formatNumber(resource.error.httpStatus)} />
              <Fact label={i18n.t('watch.telemetry.api_code')} value={resource.error.apiCode ?? i18n.t('common.not_exposed')} />
              <Fact label={i18n.t('watch.telemetry.trace_id')} value={resource.error.traceId ?? i18n.t('common.not_exposed')} />
            </dl>
          </details>
        </div>
      )}
    </article>
  );
}

function OpenTelemetryPanel() {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  const hasTelemetryTargets = watch.selected.length > 0 || watch.active.length > 0;
  return (
    <section className="watch-telemetry" aria-labelledby="watch-telemetry-title">
      <header className="watch-workspace__section-head">
        <div>
          <p className="watch-workspace__kicker">{i18n.t('watch.telemetry.kicker')}</p>
          <h3 className="watch-workspace__section-title" id="watch-telemetry-title">{i18n.t('watch.telemetry.title')}</h3>
        </div>
        <ActionButton busy={watch.telemetryLoading} onClick={() => void watch.refreshTelemetry()} tone="quiet">
          {i18n.t('watch.telemetry.read')}
        </ActionButton>
      </header>
      {watch.telemetryResources.length === 0 ? null : (
        <div className="watch-telemetry__resources">
          {watch.telemetryResources.map((resource) => (
            <TelemetryResourceState key={resource.key} resource={resource} />
          ))}
        </div>
      )}
      {watch.batchStatuses.length === 0 && !hasTelemetryTargets ? (
        <p className="watch-workspace__empty">{i18n.t('watch.telemetry.empty')}</p>
      ) : watch.batchStatuses.length > 0 ? (
        <div className="watch-telemetry__grid">
          {watch.batchStatuses.map((status) => (
            <BatchTelemetry key={`${status.batch.term}-${status.batch.campus}`} status={status} />
          ))}
        </div>
      ) : null}
      {watch.sectionStatuses.length === 0 ? null : (
        <div className="watch-telemetry__sections" aria-label={i18n.t('watch.telemetry.sections_label')}>
          {watch.sectionStatuses.map((status) => (
            <SectionTelemetry key={sectionLabel(status.sectionKey)} status={status} />
          ))}
        </div>
      )}
    </section>
  );
}

function ActiveActions({ watchItem }: { readonly watchItem: ActiveWatchView }) {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  return (
    <div className="watch-workspace__actions">
      <ActionButton onClick={() => watch.resetAudibleCount(watchItem)} tone="quiet">{i18n.t('watch.clear_audible_count')}</ActionButton>
      <ActionButton onClick={() => watch.stop(watchItem)} tone="accent">{i18n.t('watch.stop_short')}</ActionButton>
    </div>
  );
}

function SelectedSectionManager({
  policy,
  policyReady,
}: {
  readonly policy: WatchPolicyV1;
  readonly policyReady: boolean;
}) {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  const inactiveCount = watch.selected.filter((sectionKey) =>
    !watch.isActive(sectionKey) && watch.isWatchable(sectionKey)).length;
  return (
    <section aria-labelledby="watch-selected-title">
      <header className="watch-workspace__section-head">
        <div>
          <p className="watch-workspace__kicker">{i18n.t('watch.selection_kicker')}</p>
          <h3 className="watch-workspace__section-title" id="watch-selected-title">{i18n.t('watch.selected_title')}</h3>
        </div>
        <span className="watch-workspace__count" aria-label={i18n.t('watch.selected_count', {
          count: i18n.formatNumber(watch.selected.length),
          max: i18n.formatNumber(MAX_SELECTED_SECTIONS),
        })}>
          {i18n.formatNumber(watch.selected.length)}/{i18n.formatNumber(MAX_SELECTED_SECTIONS)}
        </span>
      </header>
      {watch.selected.length === 0 ? (
        <p className="watch-workspace__empty">{i18n.t('watch.selected_empty')}</p>
      ) : (
        <ul className="watch-workspace__list">
          {watch.selected.map((sectionKey) => {
            const active = watch.active.find((value) => sectionLabel(value.sectionKey) === sectionLabel(sectionKey));
            const pending = watch.pending.some((value) => sectionLabel(value) === sectionLabel(sectionKey));
            const observation = watch.observations.find((value) => sectionLabel(value.sectionKey) === sectionLabel(sectionKey));
            const watchable = watch.isWatchable(sectionKey);
            return (
              <li className="watch-workspace__item" key={sectionLabel(sectionKey)}>
                <div>
                  <div className="watch-workspace__identity">
                    <strong className="watch-workspace__index">{sectionKey.index}</strong>
                    <span className="watch-workspace__badge" data-state={active === undefined
                      ? watchable ? 'SELECTED' : 'OUT_OF_RANGE'
                      : 'READY'}>
                      {pending
                        ? i18n.t('watch.state.starting')
                        : active === undefined
                          ? i18n.t(watchable ? 'watch.state.selected' : 'watch.term_out_of_range')
                          : i18n.t('watch.state.watching')}
                    </span>
                    {observation === undefined ? null : (
                      <span className="watch-workspace__badge" data-state={observation.state}>
                        {i18n.t(openStateMessageKeys[observation.state])}
                      </span>
                    )}
                  </div>
                  <p className="watch-workspace__meta">{sectionKey.term} / {sectionKey.campus}</p>
                  {active === undefined && !watchable ? (
                    <p className="watch-workspace__meta">{i18n.t('watch.term_out_of_range_detail')}</p>
                  ) : null}
                  {active === undefined ? null : (
                    <p className="watch-workspace__meta">
                      {i18n.t('watch.policy_summary', {
                        duration: active.policy.continuousDuration.kind === 'UNLIMITED'
                          ? i18n.t('watch.unlimited')
                          : `${active.policy.continuousDuration.seconds}s`,
                        max: i18n.formatNumber(active.policy.maxAudible),
                        mode: i18n.t(watchNotificationMessageKeys[active.policy.notificationMode]),
                      })}
                    </p>
                  )}
                </div>
                {active === undefined ? (
                  <ActionButton disabled={pending} onClick={() => watch.remove(sectionKey)} tone="quiet">{i18n.t('watch.remove')}</ActionButton>
                ) : <ActiveActions watchItem={active} />}
              </li>
            );
          })}
        </ul>
      )}
      {watch.selected.length === 0 ? null : (
        <div className="watch-workspace__actions">
          <ActionButton
            busy={watch.starting}
            busyLabel={i18n.t('watch.starting_selected')}
            disabled={!policyReady || inactiveCount === 0 || watch.pending.length > 0 || watch.starting}
            onClick={() => void watch.startSelected(policy)}
            tone="accent"
          >
            {i18n.t('watch.start_selected', { count: i18n.formatNumber(inactiveCount) })}
          </ActionButton>
          <ActionButton
            disabled={!policyReady || watch.active.length === 0}
            onClick={() => watch.active.forEach((item) => watch.updatePolicy(item, policy))}
            tone="quiet"
          >
            {i18n.t('watch.apply_policy')}
          </ActionButton>
        </div>
      )}
    </section>
  );
}

function AlertCenter() {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  const unacknowledged = watch.episodes.filter((episode) =>
    episode.notificationMode === 'CONTINUOUS' && episode.state === 'UNACKNOWLEDGED');
  const visibleEpisodeIds = new Set(watch.alerts.map((alert) => alert.episode.episodeId));
  const hiddenActionableEpisodes = watch.episodes.filter((episode) =>
    episode.notificationMode === 'CONTINUOUS'
    && (episode.state === 'UNACKNOWLEDGED' || episode.state === 'TIMED_OUT')
    && !visibleEpisodeIds.has(episode.episodeId));
  return (
    <section aria-labelledby="watch-alerts-title">
      <header className="watch-workspace__section-head">
        <div>
          <p className="watch-workspace__kicker">{i18n.t('watch.episodes_kicker')}</p>
          <h3 className="watch-workspace__section-title" id="watch-alerts-title">{i18n.t('watch.alert_center')}</h3>
        </div>
        {unacknowledged.length === 0 ? null : (
          <ActionButton onClick={watch.acknowledgeAll} tone="accent">
            {i18n.t('action.acknowledge_all')}
          </ActionButton>
        )}
      </header>
      {watch.alerts.length === 0 && hiddenActionableEpisodes.length === 0 ? (
        <p className="watch-workspace__empty">{i18n.t('watch.no_alerts')}</p>
      ) : (
        <div className="watch-workspace__alerts">
          {watch.alerts.map((alert) => {
            const episode = watch.episodes.find((candidate) =>
              candidate.episodeId === alert.episode.episodeId) ?? alert.episode;
            return (
            <article className="watch-workspace__alert" key={alert.alertId}>
              <div>
                <h4>{i18n.t('watch.alert_heading', {
                  index: episode.sectionKey.index,
                  state: i18n.t(episodeStateMessageKeys[episode.state]),
                })}</h4>
                <p>{i18n.t('watch.alert_counts', {
                  audible: i18n.formatNumber(episode.audibleCount),
                  max: i18n.formatNumber(episode.maxAudible),
                  observations: i18n.formatNumber(episode.observationCount),
                })}</p>
                <p className="watch-workspace__meta">{i18n.t('watch.alert_dismiss_note')}</p>
              </div>
              <div className="watch-workspace__actions">
                {episode.notificationMode === 'CONTINUOUS' && episode.state === 'UNACKNOWLEDGED' ? (
                  <ActionButton onClick={() => watch.acknowledge(episode)} tone="accent">{i18n.t('action.acknowledge')}</ActionButton>
                ) : null}
                {episode.notificationMode === 'CONTINUOUS' && episode.state === 'TIMED_OUT' ? (
                  <ActionButton onClick={() => watch.resume(episode)} tone="accent">{i18n.t('watch.resume_alarm')}</ActionButton>
                ) : null}
                <ActionButton onClick={() => watch.dismissAlert(alert)} tone="quiet">{i18n.t('watch.dismiss_alert')}</ActionButton>
              </div>
            </article>
            );
          })}
          {hiddenActionableEpisodes.map((episode) => (
            <article className="watch-workspace__alert" data-alert-visibility="DISMISSED" key={episode.episodeId}>
              <div>
                <h4>{i18n.t('watch.alert_heading', {
                  index: episode.sectionKey.index,
                  state: i18n.t(episodeStateMessageKeys[episode.state]),
                })}</h4>
                <p>{i18n.t('watch.alert_dismissed', {
                  observations: i18n.formatNumber(episode.observationCount),
                })}</p>
                <p className="watch-workspace__meta">{i18n.t('watch.alert_active_note')}</p>
              </div>
              <div className="watch-workspace__actions">
                {episode.state === 'UNACKNOWLEDGED' ? (
                  <ActionButton onClick={() => watch.acknowledge(episode)} tone="accent">{i18n.t('action.acknowledge')}</ActionButton>
                ) : (
                  <ActionButton onClick={() => watch.resume(episode)} tone="accent">{i18n.t('watch.resume_alarm')}</ActionButton>
                )}
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

export interface WatchWorkspaceProps {
  readonly initialPolicy?: WatchPolicyV1 | undefined;
  readonly onPolicyChange?: ((policy: WatchPolicyV1) => void) | undefined;
}

export function WatchWorkspace({
  initialPolicy = DEFAULT_WATCH_POLICY,
  onPolicyChange,
}: WatchWorkspaceProps = {}) {
  const i18n = useBcspI18n();
  const watch = useLiveWatch();
  const [notificationMode, setNotificationMode] = useState<WatchPolicyV1['notificationMode']>(
    initialPolicy.notificationMode,
  );
  const [maxAudible, setMaxAudible] = useState(initialPolicy.maxAudible);
  const [durationKind, setDurationKind] = useState<'FINITE' | 'UNLIMITED'>(
    initialPolicy.continuousDuration.kind,
  );
  const [continuousConfirmed, setContinuousConfirmed] = useState(false);
  const initialPolicyRender = useRef(true);
  const onPolicyChangeRef = useRef(onPolicyChange);
  onPolicyChangeRef.current = onPolicyChange;
  const validMaximum = Number.isInteger(maxAudible) && maxAudible > 0;
  const policy = useMemo<WatchPolicyV1>(() => ({
    notificationMode,
    maxAudible: validMaximum ? maxAudible : DEFAULT_WATCH_POLICY.maxAudible,
    continuousDuration: durationKind === 'UNLIMITED'
      ? { kind: 'UNLIMITED' }
      : { kind: 'FINITE', seconds: 600 },
  }), [durationKind, maxAudible, notificationMode, validMaximum]);
  const policyReady = validMaximum && (notificationMode === 'ONE_SHOT' || continuousConfirmed);
  useEffect(() => {
    if (initialPolicyRender.current) {
      initialPolicyRender.current = false;
      return;
    }
    if (policyReady) onPolicyChangeRef.current?.(policy);
  }, [policy, policyReady]);
  const connectionSignal = watch.connection === 'OPEN'
    ? 'ready'
    : watch.connection === 'CONNECTING'
      ? 'refreshing'
      : watch.connection === 'ERROR'
        ? 'offline'
      : 'unknown';
  const watchAudioWarning = watch.active.length > 0
    && (watch.audioState === 'BLOCKED' || watch.audioState === 'FAILED');

  return (
    <div className="watch-workspace" data-watch-connection={watch.connection}>
      <section className="watch-workspace__command-grid">
        <div className="watch-workspace__panel">
          <p className="watch-workspace__kicker">{i18n.t('watch.console_kicker')}</p>
          <h3 className="watch-workspace__title">{i18n.t('watch.console_title')}</h3>
          <p className="watch-workspace__lede">{i18n.t('watch.desk_lede')}</p>
          <div className="watch-workspace__status-strip" aria-label={i18n.t('watch.session_status')}>
            <StatusSignal
              detail={i18n.t('watch.connection.user_label')}
              label={connectionLabel(watch.connection, watch.active.length, watch.muted, i18n)}
              state={connectionSignal}
            />
            <Metric label={i18n.t('watch.metric.selected')} value={i18n.formatNumber(watch.selected.length)} />
            <Metric label={i18n.t('watch.metric.active')} value={i18n.formatNumber(watch.active.length)} />
            <Metric label={i18n.t('watch.metric.open_episodes')} value={i18n.formatNumber(
              watch.episodes.filter((episode) => episode.state === 'UNACKNOWLEDGED').length,
            )} />
          </div>
          <details className="watch-workspace__diagnostics">
            <summary>{i18n.t('watch.diagnostics.summary')}</summary>
            <dl className="watch-workspace__diagnostic-facts">
              <Fact label={i18n.t('watch.diagnostics.transport')} value={`WebSocket / ${watch.connection}`} />
            </dl>
          </details>
        </div>
        <div className="watch-workspace__panel">
          <div className="watch-workspace__form">
            <fieldset className="watch-workspace__mode">
              <legend>{i18n.t('watch.notification_mode')}</legend>
              <label>
                <input checked={notificationMode === 'ONE_SHOT'} name="watch-mode" onChange={() => {
                  setNotificationMode('ONE_SHOT');
                  setContinuousConfirmed(false);
                }} type="radio" />
                {i18n.t('watch.one_shot_detail')}
              </label>
              <label>
                <input checked={notificationMode === 'CONTINUOUS'} name="watch-mode" onChange={() => setNotificationMode('CONTINUOUS')} type="radio" />
                {i18n.t('watch.continuous_detail')}
              </label>
            </fieldset>
            <div className="watch-workspace__field">
              <label htmlFor="watch-max-audible">{i18n.t('watch.max_audible')}</label>
              <input
                aria-invalid={!validMaximum || undefined}
                className="watch-workspace__input"
                id="watch-max-audible"
                min="1"
                onChange={(event) => setMaxAudible(Number(event.currentTarget.value))}
                step="1"
                type="number"
                value={maxAudible}
              />
              {validMaximum ? null : <span className="watch-workspace__inline-status" role="alert">{i18n.t('watch.invalid_max')}</span>}
            </div>
            {notificationMode === 'CONTINUOUS' ? (
              <>
                <div className="watch-workspace__field">
                  <label htmlFor="watch-duration">{i18n.t('watch.continuous_duration')}</label>
                  <select className="watch-workspace__select" id="watch-duration" onChange={(event) => setDurationKind(event.currentTarget.value as 'FINITE' | 'UNLIMITED')} value={durationKind}>
                    <option value="FINITE">{i18n.t('watch.ten_minutes')}</option>
                    <option value="UNLIMITED">{i18n.t('watch.unlimited_ack')}</option>
                  </select>
                </div>
                <label className="watch-workspace__confirm">
                  <input checked={continuousConfirmed} onChange={(event) => setContinuousConfirmed(event.currentTarget.checked)} type="checkbox" />
                  {i18n.t('watch.continuous_confirm')}
                </label>
              </>
            ) : null}
            <div className="watch-workspace__field">
              <label htmlFor="watch-volume">{i18n.t('watch.sound_volume', {
                volume: i18n.formatNumber(watch.volume),
              })}</label>
              <input id="watch-volume" max="100" min="0" onChange={(event) => watch.setVolume(Number(event.currentTarget.value))} type="range" value={watch.volume} />
            </div>
            <div className="watch-workspace__actions">
              <ActionButton
                busy={watch.audioState === 'UNLOCKING'}
                busyLabel={i18n.t('audio.state.unlocking')}
                onClick={() => void watch.testSound()}
                tone="accent"
              >
                {i18n.t('watch.enable_test_sound')}
              </ActionButton>
              <ActionButton disabled={watch.audioState !== 'READY'} onClick={() => watch.setMuted(!watch.muted)} tone="quiet">
                {i18n.t(watch.muted ? 'watch.unmute' : 'watch.mute')}
              </ActionButton>
              <ActionButton disabled={watch.connection === 'IDLE' || watch.connection === 'CLOSED'} onClick={watch.disconnect} tone="quiet">{i18n.t('watch.disconnect')}</ActionButton>
            </div>
            <p className="watch-workspace__inline-status" role="status">{i18n.t('watch.audio_summary', {
              audio: audioLabel(watch.audioState, i18n),
              audibility: i18n.t(watch.muted ? 'watch.audibility.muted' : 'watch.audibility.audible'),
              mixer: watch.continuousEpisodeIds.length === 0
                ? i18n.t('watch.mixer_idle')
                : i18n.t('watch.mixer_episodes', {
                  count: i18n.formatNumber(watch.continuousEpisodeIds.length),
                }),
            })}</p>
            {watchAudioWarning ? (
              <p className="watch-workspace__confirm" role="alert">
                {i18n.t(watch.audioState === 'BLOCKED'
                  ? 'watch.audio_watch_warning.blocked'
                  : 'watch.audio_watch_warning.failed')}
              </p>
            ) : null}
            {!policyReady ? <p className="watch-workspace__inline-status" role="alert">{i18n.t('watch.continuous_required')}</p> : null}
          </div>
        </div>
      </section>
      <SelectedSectionManager policy={policy} policyReady={policyReady} />
      <AlertCenter />
      <OpenTelemetryPanel />
    </div>
  );
}
