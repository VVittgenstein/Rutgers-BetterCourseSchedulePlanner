import { useMemo, useState } from 'react';

import { ActionButton, Metric, StatusSignal } from '../design-system';
import { useBcspI18n } from '../i18n/runtime';
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
} from './LiveWatchProvider';

function sectionLabel(sectionKey: SectionKey): string {
  return `${sectionKey.index} / ${sectionKey.term} / ${sectionKey.campus}`;
}

function formatTime(value: string | null, format: (value: number) => string): string {
  if (value === null) return 'Not observed';
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? format(timestamp) : 'Invalid timestamp';
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="watch-telemetry__fact">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function countsLabel(snapshot: OpenCounterSnapshotV1, scope: 'RUN' | 'TODAY'): string {
  const counts = scope === 'RUN' ? snapshot.runCounts : snapshot.todayCounts;
  if (counts === undefined) return 'Not exposed for this target';
  return `${counts.attempted} attempted / ${counts.succeeded} succeeded / ${counts.failed} failed / ${counts.empty} empty`;
}

function FreshnessBadge({ state }: { readonly state: string }) {
  return <span className="watch-workspace__badge" data-state={state}>{state}</span>;
}

function BatchTelemetry({ status }: { readonly status: OpenRefreshStatusV1 }) {
  const i18n = useBcspI18n();
  const format = (value: number) => i18n.formatDate(value, { dateStyle: 'medium', timeStyle: 'medium' });
  const actual = status.scheduler.actualStartToStartIntervalMilliseconds;
  return (
    <article className="watch-telemetry__batch" data-freshness={status.freshness.state}>
      <header className="watch-telemetry__batch-head">
        <div>
          <p className="watch-workspace__kicker">Open batch</p>
          <h4>{status.batch.term} / {status.batch.campus}</h4>
        </div>
        <FreshnessBadge state={status.freshness.state} />
      </header>
      <dl className="watch-telemetry__facts">
        <Fact label="Last attempt" value={status.latestAttempt === null
          ? 'Never attempted'
          : `${status.latestAttempt.classification} · ${formatTime(status.latestAttempt.completedAt, format)}`} />
        <Fact label="Last valid" value={formatTime(status.lastValidObservation?.observedAt ?? null, format)} />
        <Fact label="Requested general" value={`${status.scheduler.requestedGeneralIntervalSeconds}s`} />
        <Fact label="Requested effective" value={`${status.scheduler.requestedEffectiveIntervalSeconds}s · ${status.scheduler.lane}`} />
        <Fact label="Actual interval" value={actual === null ? 'Not measured' : `${(actual / 1000).toFixed(2)}s`} />
        <Fact label="Scheduler lag" value={`${status.scheduler.schedulerLagMilliseconds}ms`} />
        <Fact label="Circuit" value={`${status.circuit.state}${status.circuit.reason === null ? '' : ` · ${status.circuit.reason}`}`} />
        <Fact label="Circuit retry" value={formatTime(status.circuit.retryAt, format)} />
        <Fact label="Run counters" value={countsLabel(status.counterSnapshot, 'RUN')} />
        <Fact label={`Rutgers day · ${status.counterSnapshot.rutgersDay}`} value={countsLabel(status.counterSnapshot, 'TODAY')} />
        <Fact label="LKG age" value={status.freshness.lastKnownGoodAgeSeconds === null
          ? 'Unknown'
          : `${status.freshness.lastKnownGoodAgeSeconds}s`} />
        <Fact label="Uncertainty" value={status.freshness.uncertainty ?? 'None'} />
      </dl>
    </article>
  );
}

function SectionTelemetry({ status }: { readonly status: OpenSectionStatusV1 }) {
  const i18n = useBcspI18n();
  const observedAt = formatTime(status.freshness.observedAt, (value) =>
    i18n.formatDate(value, { dateStyle: 'medium', timeStyle: 'short' }));
  return (
    <article className="watch-telemetry__section" data-freshness={status.freshness.state}>
      <div className="watch-workspace__identity">
        <strong className="watch-workspace__index">{status.sectionKey.index}</strong>
        <FreshnessBadge state={status.freshness.state} />
        <span className="watch-workspace__badge" data-state={status.state}>{status.state}</span>
      </div>
      <p className="watch-workspace__meta">{status.sectionKey.term} / {status.sectionKey.campus}</p>
      <p className="watch-workspace__meta">Observed {observedAt}</p>
      <p className="watch-workspace__meta">Lag {status.schedulerLagMilliseconds}ms · {status.freshness.uncertainty ?? 'certain'}</p>
    </article>
  );
}

function OpenTelemetryPanel() {
  const watch = useLiveWatch();
  return (
    <section className="watch-telemetry" aria-labelledby="watch-telemetry-title">
      <header className="watch-workspace__section-head">
        <div>
          <p className="watch-workspace__kicker">Truthful service evidence</p>
          <h3 className="watch-workspace__section-title" id="watch-telemetry-title">Freshness / lag / circuit / counters</h3>
        </div>
        <ActionButton busy={watch.telemetryLoading} onClick={() => void watch.refreshTelemetry()} tone="quiet">
          Read status
        </ActionButton>
      </header>
      {watch.telemetryError ? (
        <p className="watch-workspace__confirm" role="alert">Some status reads failed. Last successful values remain visible where available.</p>
      ) : null}
      {watch.batchStatuses.length === 0 ? (
        <p className="watch-workspace__empty">Select a Section to read its current BCSP Open status. This does not request a manual Rutgers refresh.</p>
      ) : (
        <div className="watch-telemetry__grid">
          {watch.batchStatuses.map((status) => (
            <BatchTelemetry key={`${status.batch.term}-${status.batch.campus}`} status={status} />
          ))}
        </div>
      )}
      {watch.sectionStatuses.length === 0 ? null : (
        <div className="watch-telemetry__sections" aria-label="Selected Section Open status">
          {watch.sectionStatuses.map((status) => (
            <SectionTelemetry key={sectionLabel(status.sectionKey)} status={status} />
          ))}
        </div>
      )}
    </section>
  );
}

function ActiveActions({ watchItem }: { readonly watchItem: ActiveWatchView }) {
  const watch = useLiveWatch();
  return (
    <div className="watch-workspace__actions">
      <ActionButton onClick={() => watch.resetAudibleCount(watchItem)} tone="quiet">Reset count</ActionButton>
      <ActionButton onClick={() => watch.stop(watchItem)} tone="accent">Stop</ActionButton>
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
  const watch = useLiveWatch();
  const inactiveCount = watch.selected.filter((sectionKey) => !watch.isActive(sectionKey)).length;
  return (
    <section aria-labelledby="watch-selected-title">
      <header className="watch-workspace__section-head">
        <div>
          <p className="watch-workspace__kicker">Explicit browser-session selection</p>
          <h3 className="watch-workspace__section-title" id="watch-selected-title">Selected Sections</h3>
        </div>
        <span className="watch-workspace__count" aria-label={`${watch.selected.length} of ${MAX_SELECTED_SECTIONS} selected`}>
          {watch.selected.length}/{MAX_SELECTED_SECTIONS}
        </span>
      </header>
      {watch.selected.length === 0 ? (
        <p className="watch-workspace__empty">Choose “+ Watch” on a Course or Section result. Selection alone does not start a subscription.</p>
      ) : (
        <ul className="watch-workspace__list">
          {watch.selected.map((sectionKey) => {
            const active = watch.active.find((value) => sectionLabel(value.sectionKey) === sectionLabel(sectionKey));
            const pending = watch.pending.some((value) => sectionLabel(value) === sectionLabel(sectionKey));
            const observation = watch.observations.find((value) => sectionLabel(value.sectionKey) === sectionLabel(sectionKey));
            return (
              <li className="watch-workspace__item" key={sectionLabel(sectionKey)}>
                <div>
                  <div className="watch-workspace__identity">
                    <strong className="watch-workspace__index">{sectionKey.index}</strong>
                    <span className="watch-workspace__badge" data-state={active === undefined ? 'SELECTED' : 'READY'}>
                      {pending ? 'STARTING' : active === undefined ? 'SELECTED' : 'WATCHING'}
                    </span>
                    {observation === undefined ? null : (
                      <span className="watch-workspace__badge" data-state={observation.state}>{observation.state}</span>
                    )}
                  </div>
                  <p className="watch-workspace__meta">{sectionKey.term} / {sectionKey.campus}</p>
                  {active === undefined ? null : (
                    <p className="watch-workspace__meta">
                      {active.policy.notificationMode} · max {active.policy.maxAudible} · {active.policy.continuousDuration.kind === 'UNLIMITED'
                        ? 'unlimited'
                        : `${active.policy.continuousDuration.seconds}s`}
                    </p>
                  )}
                </div>
                {active === undefined ? (
                  <ActionButton disabled={pending} onClick={() => watch.remove(sectionKey)} tone="quiet">Remove</ActionButton>
                ) : <ActiveActions watchItem={active} />}
              </li>
            );
          })}
        </ul>
      )}
      <div className="watch-workspace__actions">
        <ActionButton
          disabled={!policyReady || inactiveCount === 0 || watch.pending.length > 0}
          onClick={() => watch.startSelected(policy)}
          tone="accent"
        >
          Start selected · {inactiveCount}
        </ActionButton>
        <ActionButton
          disabled={!policyReady || watch.active.length === 0}
          onClick={() => watch.active.forEach((item) => watch.updatePolicy(item, policy))}
          tone="quiet"
        >
          Apply policy to active
        </ActionButton>
      </div>
    </section>
  );
}

function AlertCenter() {
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
          <p className="watch-workspace__kicker">Server-authoritative episodes</p>
          <h3 className="watch-workspace__section-title" id="watch-alerts-title">Alert center</h3>
        </div>
        <ActionButton disabled={unacknowledged.length === 0} onClick={watch.acknowledgeAll} tone="accent">
          Acknowledge all
        </ActionButton>
      </header>
      {watch.alerts.length === 0 && hiddenActionableEpisodes.length === 0 ? (
        <p className="watch-workspace__empty">No visible Open-seat alerts.</p>
      ) : (
        <div className="watch-workspace__alerts">
          {watch.alerts.map((alert) => {
            const episode = watch.episodes.find((candidate) =>
              candidate.episodeId === alert.episode.episodeId) ?? alert.episode;
            return (
            <article className="watch-workspace__alert" key={alert.alertId}>
              <div>
                <h4>Section {episode.sectionKey.index} · {episode.state}</h4>
                <p>{episode.observationCount} observations · audible {episode.audibleCount}/{episode.maxAudible}</p>
                <p className="watch-workspace__meta">Dismiss hides this alert card; episode controls remain available until resolved.</p>
              </div>
              <div className="watch-workspace__actions">
                {episode.notificationMode === 'CONTINUOUS' && episode.state === 'UNACKNOWLEDGED' ? (
                  <ActionButton onClick={() => watch.acknowledge(episode)} tone="accent">Acknowledge</ActionButton>
                ) : null}
                {episode.notificationMode === 'CONTINUOUS' && episode.state === 'TIMED_OUT' ? (
                  <ActionButton onClick={() => watch.resume(episode)} tone="accent">Resume alarm</ActionButton>
                ) : null}
                <ActionButton onClick={() => watch.dismissAlert(alert)} tone="quiet">Dismiss alert</ActionButton>
              </div>
            </article>
            );
          })}
          {hiddenActionableEpisodes.map((episode) => (
            <article className="watch-workspace__alert" data-alert-visibility="DISMISSED" key={episode.episodeId}>
              <div>
                <h4>Section {episode.sectionKey.index} · {episode.state}</h4>
                <p>{episode.observationCount} observations · alert card dismissed</p>
                <p className="watch-workspace__meta">The episode remains server-active; its control stays available here.</p>
              </div>
              <div className="watch-workspace__actions">
                {episode.state === 'UNACKNOWLEDGED' ? (
                  <ActionButton onClick={() => watch.acknowledge(episode)} tone="accent">Acknowledge</ActionButton>
                ) : (
                  <ActionButton onClick={() => watch.resume(episode)} tone="accent">Resume alarm</ActionButton>
                )}
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

export function WatchWorkspace() {
  const watch = useLiveWatch();
  const [notificationMode, setNotificationMode] = useState<WatchPolicyV1['notificationMode']>(
    DEFAULT_WATCH_POLICY.notificationMode,
  );
  const [maxAudible, setMaxAudible] = useState(DEFAULT_WATCH_POLICY.maxAudible);
  const [durationKind, setDurationKind] = useState<'FINITE' | 'UNLIMITED'>('FINITE');
  const [continuousConfirmed, setContinuousConfirmed] = useState(false);
  const validMaximum = Number.isInteger(maxAudible) && maxAudible > 0;
  const policy = useMemo<WatchPolicyV1>(() => ({
    notificationMode,
    maxAudible: validMaximum ? maxAudible : DEFAULT_WATCH_POLICY.maxAudible,
    continuousDuration: durationKind === 'UNLIMITED'
      ? { kind: 'UNLIMITED' }
      : { kind: 'FINITE', seconds: 600 },
  }), [durationKind, maxAudible, notificationMode, validMaximum]);
  const policyReady = validMaximum && (notificationMode === 'ONE_SHOT' || continuousConfirmed);
  const connectionSignal = watch.connection === 'OPEN'
    ? 'ready'
    : watch.connection === 'CONNECTING'
      ? 'refreshing'
      : watch.connection === 'ERROR'
        ? 'offline'
        : 'unknown';

  return (
    <div className="watch-workspace" data-watch-connection={watch.connection}>
      <section className="watch-workspace__command-grid">
        <div className="watch-workspace__panel">
          <p className="watch-workspace__kicker">03 / live subscription console</p>
          <h3 className="watch-workspace__title">Watch desk</h3>
          <p className="watch-workspace__lede">Select up to nine Sections, choose the alert policy, then explicitly start a connection-bound watch.</p>
          <div className="watch-workspace__status-strip" aria-label="Watch session status">
            <StatusSignal detail="WebSocket" label={watch.connection} state={connectionSignal} />
            <Metric label="Selected" value={watch.selected.length} />
            <Metric label="Active" value={watch.active.length} />
            <Metric label="Open episodes" value={watch.episodes.filter((episode) => episode.state === 'UNACKNOWLEDGED').length} />
          </div>
        </div>
        <div className="watch-workspace__panel">
          <div className="watch-workspace__form">
            <fieldset className="watch-workspace__mode">
              <legend>Notification mode</legend>
              <label>
                <input checked={notificationMode === 'ONE_SHOT'} name="watch-mode" onChange={() => {
                  setNotificationMode('ONE_SHOT');
                  setContinuousConfirmed(false);
                }} type="radio" />
                One-shot cue per valid Open observation
              </label>
              <label>
                <input checked={notificationMode === 'CONTINUOUS'} name="watch-mode" onChange={() => setNotificationMode('CONTINUOUS')} type="radio" />
                Continuous episode alarm
              </label>
            </fieldset>
            <div className="watch-workspace__field">
              <label htmlFor="watch-max-audible">Maximum audible cues per Section</label>
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
              {validMaximum ? null : <span className="watch-workspace__inline-status" role="alert">Enter a positive whole number.</span>}
            </div>
            {notificationMode === 'CONTINUOUS' ? (
              <>
                <div className="watch-workspace__field">
                  <label htmlFor="watch-duration">Continuous duration</label>
                  <select className="watch-workspace__select" id="watch-duration" onChange={(event) => setDurationKind(event.currentTarget.value as 'FINITE' | 'UNLIMITED')} value={durationKind}>
                    <option value="FINITE">10 minutes</option>
                    <option value="UNLIMITED">Unlimited — until acknowledged</option>
                  </select>
                </div>
                <label className="watch-workspace__confirm">
                  <input checked={continuousConfirmed} onChange={(event) => setContinuousConfirmed(event.currentTarget.checked)} type="checkbox" />
                  I understand that CONTINUOUS sound persists until acknowledgement, timeout, or a reliable state transition.
                </label>
              </>
            ) : null}
            <div className="watch-workspace__field">
              <label htmlFor="watch-volume">Sound volume · {watch.volume}%</label>
              <input id="watch-volume" max="100" min="0" onChange={(event) => watch.setVolume(Number(event.currentTarget.value))} type="range" value={watch.volume} />
            </div>
            <div className="watch-workspace__actions">
              <ActionButton onClick={() => void watch.testSound()} tone="accent">Enable / test sound</ActionButton>
              <ActionButton disabled={watch.audioState !== 'READY'} onClick={() => watch.setMuted(!watch.muted)} tone="quiet">
                {watch.muted ? 'Unmute' : 'Mute'}
              </ActionButton>
              <ActionButton disabled={watch.connection === 'IDLE' || watch.connection === 'CLOSED'} onClick={watch.disconnect} tone="quiet">Disconnect</ActionButton>
            </div>
            <p className="watch-workspace__inline-status" role="status">Audio {watch.audioState} · {watch.muted ? 'muted' : 'audible'} · mixer {watch.continuousEpisodeIds.length === 0 ? 'idle' : `${watch.continuousEpisodeIds.length} episodes`}</p>
            {!policyReady ? <p className="watch-workspace__inline-status" role="alert">Confirm the CONTINUOUS alarm before starting or applying this policy.</p> : null}
          </div>
        </div>
      </section>
      <SelectedSectionManager policy={policy} policyReady={policyReady} />
      <AlertCenter />
      <OpenTelemetryPanel />
    </div>
  );
}
