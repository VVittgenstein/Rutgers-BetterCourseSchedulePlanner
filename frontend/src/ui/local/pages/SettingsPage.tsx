import { useEffect, useId, useRef, useState, type FormEvent } from 'react';

import { ActionButton } from '../../shared/design-system';
import { useBcspI18n } from '../../shared/i18n/runtime';
import { useLocalI18n } from '../i18n/runtime';
import type {
  LocalLocaleOverride,
  LocalSettings,
  PreparedUserDataReset,
  StoredSettings,
} from '../personal/contracts';
import {
  LocalPageFrame,
  completeAction,
  type LocalPageAsyncState,
  type MaybePromise,
} from './PageFrame';

type ConfirmScope = 'FILTERS' | 'VIEWS' | null;
type WorkingScope = 'SETTINGS' | 'FILTERS' | 'VIEWS' | 'PREPARE_ALL' | 'CONFIRM_ALL' | null;
type SettingsSaveState = 'UNCHANGED' | 'DIRTY' | 'INVALID' | 'SAVING' | 'SAVED' | 'FAILED';

export interface SettingsPageProps extends LocalPageAsyncState {
  readonly onConfirmUserDataReset: (prepared: PreparedUserDataReset) => MaybePromise<void>;
  readonly onDeleteAllSavedViews: () => MaybePromise<void>;
  readonly onPrepareUserDataReset: () => PreparedUserDataReset | Promise<PreparedUserDataReset>;
  readonly onResetCurrentFilters: () => MaybePromise<void>;
  readonly onUpdateSettings: (value: LocalSettings) => MaybePromise<void>;
  readonly savedViewCount: number;
  readonly settings: StoredSettings;
}

function validSettings(value: LocalSettings): boolean {
  return Number.isInteger(value.catalogRefreshMinutes)
    && value.catalogRefreshMinutes >= 1
    && value.catalogRefreshMinutes <= 1440
    && Number.isInteger(value.openRefreshSeconds)
    && value.openRefreshSeconds >= 3
    && value.openRefreshSeconds <= 3600
    && Number.isInteger(value.volumePercent)
    && value.volumePercent >= 0
    && value.volumePercent <= 100
    && Number.isInteger(value.soundPolicy.maxAudible)
    && value.soundPolicy.maxAudible >= 1
    && value.soundPolicy.maxAudible <= 255;
}

function numberValue(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function sameSettings(left: LocalSettings, right: LocalSettings): boolean {
  return left.localeOverride === right.localeOverride
    && left.catalogRefreshMinutes === right.catalogRefreshMinutes
    && left.openRefreshSeconds === right.openRefreshSeconds
    && left.volumePercent === right.volumePercent
    && left.soundPolicy.notificationMode === right.soundPolicy.notificationMode
    && left.soundPolicy.maxAudible === right.soundPolicy.maxAudible
    && left.soundPolicy.continuousDuration.kind === right.soundPolicy.continuousDuration.kind
    && (left.soundPolicy.continuousDuration.kind !== 'FINITE'
      || (right.soundPolicy.continuousDuration.kind === 'FINITE'
        && left.soundPolicy.continuousDuration.seconds
          === right.soundPolicy.continuousDuration.seconds));
}

export function SettingsPage({
  error,
  onClearError,
  onConfirmUserDataReset,
  onDeleteAllSavedViews,
  onPrepareUserDataReset,
  onReload,
  onResetCurrentFilters,
  onUpdateSettings,
  pending = false,
  savedViewCount,
  settings,
}: SettingsPageProps) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  const chinese = local.locale === 'zh-CN';
  const localeId = useId();
  const catalogId = useId();
  const openId = useId();
  const volumeId = useId();
  const modeId = useId();
  const maximumId = useId();
  const durationId = useId();
  const filtersResetTriggerId = useId();
  const filtersResetConfirmId = useId();
  const viewsResetTriggerId = useId();
  const viewsResetConfirmId = useId();
  const fullResetTriggerId = useId();
  const fullResetConfirmId = useId();
  const [draft, setDraft] = useState<LocalSettings>(settings.value);
  const [baseline, setBaseline] = useState<LocalSettings>(settings.value);
  const [working, setWorking] = useState<WorkingScope>(null);
  const [confirmScope, setConfirmScope] = useState<ConfirmScope>(null);
  const [preparedReset, setPreparedReset] = useState<PreparedUserDataReset | null>(null);
  const [saveOutcome, setSaveOutcome] = useState<'IDLE' | 'SAVED' | 'FAILED'>('IDLE');
  const lastSaved = useRef<LocalSettings | null>(null);
  const previousConfirmScope = useRef<ConfirmScope>(null);
  const previousPreparedReset = useRef(false);
  const valid = validSettings(draft);
  const dirty = !sameSettings(draft, baseline);
  const locked = pending || working !== null;
  const saveState: SettingsSaveState = working === 'SETTINGS'
    ? 'SAVING'
    : !valid
      ? 'INVALID'
      : saveOutcome === 'FAILED'
        ? 'FAILED'
        : saveOutcome === 'SAVED' && !dirty
          ? 'SAVED'
          : dirty
            ? 'DIRTY'
            : 'UNCHANGED';

  useEffect(() => {
    setDraft(settings.value);
    setBaseline(settings.value);
    setSaveOutcome(lastSaved.current !== null && sameSettings(lastSaved.current, settings.value)
      ? 'SAVED'
      : 'IDLE');
  }, [settings.revision, settings.value]);

  useEffect(() => {
    if (confirmScope !== null) {
      const confirmId = confirmScope === 'FILTERS' ? filtersResetConfirmId : viewsResetConfirmId;
      document.getElementById(confirmId)?.focus();
    } else if (previousConfirmScope.current !== null) {
      const previousScope = previousConfirmScope.current;
      const triggerId = previousScope === 'FILTERS'
        ? filtersResetTriggerId
        : viewsResetTriggerId;
      const trigger = document.getElementById(triggerId);
      if (trigger instanceof HTMLButtonElement && !trigger.disabled) {
        trigger.focus();
      } else {
        document.getElementById(`local-settings-${previousScope.toLowerCase()}-reset-title`)?.focus();
      }
    }
    previousConfirmScope.current = confirmScope;
  }, [
    confirmScope,
    filtersResetConfirmId,
    filtersResetTriggerId,
    viewsResetConfirmId,
    viewsResetTriggerId,
  ]);

  useEffect(() => {
    if (preparedReset !== null) {
      document.getElementById(fullResetConfirmId)?.focus();
    } else if (previousPreparedReset.current) {
      document.getElementById(fullResetTriggerId)?.focus();
    }
    previousPreparedReset.current = preparedReset !== null;
  }, [fullResetConfirmId, fullResetTriggerId, preparedReset]);

  function updateDraft(update: (current: LocalSettings) => LocalSettings) {
    setSaveOutcome('IDLE');
    setDraft(update);
  }

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!valid || !dirty) return;
    const submitted = draft;
    setWorking('SETTINGS');
    setSaveOutcome('IDLE');
    const completed = await completeAction(() => onUpdateSettings(submitted));
    setWorking(null);
    if (completed) {
      lastSaved.current = submitted;
      setBaseline(submitted);
      setSaveOutcome('SAVED');
    } else {
      setSaveOutcome('FAILED');
    }
  }

  async function confirmSmallReset(scope: Exclude<ConfirmScope, null>) {
    setWorking(scope);
    const completed = await completeAction(
      scope === 'FILTERS' ? onResetCurrentFilters : onDeleteAllSavedViews,
    );
    setWorking(null);
    if (completed) setConfirmScope(null);
  }

  async function prepareFullReset() {
    setWorking('PREPARE_ALL');
    try {
      setPreparedReset(await onPrepareUserDataReset());
    } catch {
      setPreparedReset(null);
    } finally {
      setWorking(null);
    }
  }

  async function confirmFullReset() {
    if (preparedReset === null) return;
    setWorking('CONFIRM_ALL');
    const completed = await completeAction(() => onConfirmUserDataReset(preparedReset));
    setWorking(null);
    if (completed) setPreparedReset(null);
  }

  function updateLocale(localeOverride: LocalLocaleOverride) {
    updateDraft((current) => ({ ...current, localeOverride }));
  }

  const saveMessage = {
    UNCHANGED: chinese ? '没有尚未保存的更改。' : 'NO UNSAVED CHANGES',
    DIRTY: chinese ? '更改尚未保存，可以保存。' : 'UNSAVED CHANGES / READY TO SAVE',
    INVALID: chinese ? '请修正超出范围的设置值。' : 'CORRECT OUT-OF-RANGE VALUES',
    SAVING: local.t('local.status.busy'),
    SAVED: local.t('local.settings.saved'),
    FAILED: local.t('local.status.error'),
  }[saveState];

  return (
    <LocalPageFrame
      {...(error === undefined ? {} : { error })}
      intro={local.t('local.settings.intro')}
      kicker={local.t('local.settings.kicker')}
      {...(onClearError === undefined ? {} : { onClearError })}
      {...(onReload === undefined ? {} : { onReload })}
      pending={pending}
      title={local.t('local.settings.title')}
    >
      <section aria-labelledby="local-settings-preferences-title" className="local-personal__section">
        <header className="local-personal__section-head">
          <div>
            <p className="local-personal__kicker">[ 01 / CONTROL ]</p>
            <h3 className="local-personal__section-title" id="local-settings-preferences-title">
              {local.t('local.settings.preferences')}
            </h3>
          </div>
          <span className="local-personal__badge" data-state="READY">
            REV / {local.formatNumber(settings.revision)}
          </span>
        </header>
        <form onSubmit={save}>
          <div className="local-personal__settings">
            <div className="local-personal__field">
              <label htmlFor={localeId}>{local.t('local.settings.locale')}</label>
              <select
                disabled={locked}
                id={localeId}
                onChange={(event) => updateLocale(event.currentTarget.value as LocalLocaleOverride)}
                value={draft.localeOverride}
              >
                <option value="system">{local.t('local.settings.locale_system')}</option>
                <option value="en-US">{local.t('local.settings.locale_en')}</option>
                <option value="zh-CN">{local.t('local.settings.locale_zh')}</option>
              </select>
            </div>
            <div className="local-personal__field">
              <label htmlFor={catalogId}>{local.t('local.settings.catalog_refresh')}</label>
              <input
                disabled={locked}
                id={catalogId}
                inputMode="numeric"
                max={1440}
                min={1}
                onChange={(event) => {
                  const catalogRefreshMinutes = numberValue(event.currentTarget.value);
                  updateDraft((current) => ({
                    ...current,
                    catalogRefreshMinutes,
                  }));
                }}
                required
                step={1}
                type="number"
                value={draft.catalogRefreshMinutes}
              />
            </div>
            <div className="local-personal__field">
              <label htmlFor={openId}>{local.t('local.settings.open_refresh')}</label>
              <input
                disabled={locked}
                id={openId}
                inputMode="numeric"
                max={3600}
                min={3}
                onChange={(event) => {
                  const openRefreshSeconds = numberValue(event.currentTarget.value);
                  updateDraft((current) => ({
                    ...current,
                    openRefreshSeconds,
                  }));
                }}
                required
                step={1}
                type="number"
                value={draft.openRefreshSeconds}
              />
            </div>
            <div className="local-personal__field">
              <label htmlFor={volumeId}>{local.t('local.settings.volume')}</label>
              <div className="local-page__range-row">
                <input
                  disabled={locked}
                  id={volumeId}
                  max={100}
                  min={0}
                  onChange={(event) => {
                    const volumePercent = numberValue(event.currentTarget.value);
                    updateDraft((current) => ({
                      ...current,
                      volumePercent,
                    }));
                  }}
                  step={1}
                  type="range"
                  value={draft.volumePercent}
                />
                <output htmlFor={volumeId}>{local.formatNumber(draft.volumePercent)}%</output>
              </div>
            </div>
            <div className="local-personal__field">
              <label htmlFor={modeId}>{local.t('local.settings.mode')}</label>
              <select
                disabled={locked}
                id={modeId}
                onChange={(event) => {
                  const notificationMode = event.currentTarget.value as LocalSettings['soundPolicy']['notificationMode'];
                  updateDraft((current) => ({
                    ...current,
                    soundPolicy: { ...current.soundPolicy, notificationMode },
                  }));
                }}
                value={draft.soundPolicy.notificationMode}
              >
                <option value="ONE_SHOT">{local.t('local.settings.mode_one_shot')}</option>
                <option value="CONTINUOUS">{local.t('local.settings.mode_continuous')}</option>
              </select>
            </div>
            <div className="local-personal__field">
              <label htmlFor={maximumId}>{local.t('local.settings.max_audible')}</label>
              <input
                disabled={locked}
                id={maximumId}
                inputMode="numeric"
                max={255}
                min={1}
                onChange={(event) => {
                  const maxAudible = numberValue(event.currentTarget.value);
                  updateDraft((current) => ({
                    ...current,
                    soundPolicy: { ...current.soundPolicy, maxAudible },
                  }));
                }}
                required
                step={1}
                type="number"
                value={draft.soundPolicy.maxAudible}
              />
            </div>
            <div className="local-personal__field">
              <label htmlFor={durationId}>{local.t('local.settings.duration')}</label>
              <select
                disabled={locked || draft.soundPolicy.notificationMode !== 'CONTINUOUS'}
                id={durationId}
                onChange={(event) => {
                  const continuousDuration: LocalSettings['soundPolicy']['continuousDuration'] =
                    event.currentTarget.value === 'UNLIMITED'
                      ? { kind: 'UNLIMITED' }
                      : { kind: 'FINITE', seconds: 600 };
                  updateDraft((current) => ({
                    ...current,
                    soundPolicy: { ...current.soundPolicy, continuousDuration },
                  }));
                }}
                value={draft.soundPolicy.continuousDuration.kind}
              >
                <option value="FINITE">{local.t('local.settings.duration_finite')}</option>
                <option value="UNLIMITED">{local.t('local.settings.duration_unlimited')}</option>
              </select>
            </div>
          </div>
          <div className="local-page__settings-actions">
            <p
              aria-live={saveState === 'FAILED' ? 'assertive' : 'polite'}
              className="local-personal__meta"
              data-state={saveState}
              role="status"
            >
              {saveMessage}
            </p>
            <ActionButton
              busy={working === 'SETTINGS'}
              busyLabel={local.t('local.status.busy')}
              disabled={locked || !valid || !dirty}
              tone="accent"
              type="submit"
            >
              {local.t('local.settings.save')}
            </ActionButton>
          </div>
        </form>
      </section>

      <section aria-labelledby="local-settings-reset-title" className="local-personal__section">
        <header className="local-personal__section-head">
          <div>
            <p className="local-personal__kicker">[ 02 / RESET ]</p>
            <h3 className="local-personal__section-title" id="local-settings-reset-title">
              {local.t('local.reset.title')}
            </h3>
          </div>
          <p>{local.t('local.reset.intro')}</p>
        </header>
        <div className="local-personal__reset-grid">
          <article className="local-personal__card local-page__reset-scope" data-scope="FILTERS">
            <p className="local-personal__meta">01 / FILTERS ONLY</p>
            <h4 id="local-settings-filters-reset-title" tabIndex={-1}>
              {local.t('local.reset.filters_title')}
            </h4>
            <p>{local.t('local.reset.filters_body')}</p>
            {confirmScope === 'FILTERS' ? (
              <div className="local-page__inline-panel" role="group" aria-label={local.t('local.reset.filters_title')}>
                <p><strong>{chinese ? '确认：' : 'CONFIRM:'}</strong> {local.t('local.reset.filters_body')}</p>
                <div className="local-page__inline-actions">
                  <ActionButton
                    busy={working === 'FILTERS'}
                    busyLabel={local.t('local.status.busy')}
                    id={filtersResetConfirmId}
                    onClick={() => void confirmSmallReset('FILTERS')}
                    tone="accent"
                  >
                    {local.t('local.reset.filters_action')}
                  </ActionButton>
                  <ActionButton disabled={locked} onClick={() => setConfirmScope(null)} tone="quiet">
                    {shared.t('action.close')}
                  </ActionButton>
                </div>
              </div>
            ) : (
              <ActionButton
                disabled={locked}
                id={filtersResetTriggerId}
                onClick={() => setConfirmScope('FILTERS')}
                tone="quiet"
              >
                {local.t('local.reset.filters_action')}
              </ActionButton>
            )}
          </article>

          <article className="local-personal__card local-page__reset-scope" data-scope="VIEWS">
            <p className="local-personal__meta">
              02 / {local.t('local.saved.count', { count: local.formatNumber(savedViewCount) })}
            </p>
            <h4 id="local-settings-views-reset-title" tabIndex={-1}>
              {local.t('local.reset.views_title')}
            </h4>
            <p>{local.t('local.reset.views_body')}</p>
            {confirmScope === 'VIEWS' ? (
              <div className="local-page__inline-panel" role="group" aria-label={local.t('local.reset.views_title')}>
                <p><strong>{chinese ? '确认：' : 'CONFIRM:'}</strong> {local.t('local.reset.views_body')}</p>
                <div className="local-page__inline-actions">
                  <ActionButton
                    busy={working === 'VIEWS'}
                    busyLabel={local.t('local.status.busy')}
                    id={viewsResetConfirmId}
                    onClick={() => void confirmSmallReset('VIEWS')}
                    tone="accent"
                  >
                    {local.t('local.reset.views_action')}
                  </ActionButton>
                  <ActionButton disabled={locked} onClick={() => setConfirmScope(null)} tone="quiet">
                    {shared.t('action.close')}
                  </ActionButton>
                </div>
              </div>
            ) : (
              <ActionButton
                disabled={locked || savedViewCount === 0}
                id={viewsResetTriggerId}
                onClick={() => setConfirmScope('VIEWS')}
                tone="quiet"
              >
                {local.t('local.reset.views_action')}
              </ActionButton>
            )}
          </article>

          <article className="local-personal__card local-page__reset-scope" data-scope="ALL">
            <p className="local-personal__meta">03 / ALL LOCAL USER DATA</p>
            <h4>{local.t('local.reset.user_title')}</h4>
            <p>{local.t('local.reset.user_body')}</p>
            {preparedReset === null ? (
              <ActionButton
                busy={working === 'PREPARE_ALL'}
                busyLabel={local.t('local.status.busy')}
                disabled={locked}
                id={fullResetTriggerId}
                onClick={() => void prepareFullReset()}
                tone="accent"
              >
                {local.t('local.reset.prepare')}
              </ActionButton>
            ) : (
              <div className="local-page__inline-panel" role="group" aria-label={local.t('local.reset.user_title')}>
                <p>{local.t('local.reset.prepared', {
                  seconds: local.formatNumber(preparedReset.expiresInSeconds),
                })}</p>
                <div className="local-page__inline-actions">
                  <ActionButton
                    busy={working === 'CONFIRM_ALL'}
                    busyLabel={local.t('local.status.busy')}
                    id={fullResetConfirmId}
                    onClick={() => void confirmFullReset()}
                    tone="accent"
                  >
                    {local.t('local.reset.confirm')}
                  </ActionButton>
                  <ActionButton disabled={locked} onClick={() => setPreparedReset(null)} tone="quiet">
                    {shared.t('action.close')}
                  </ActionButton>
                </div>
              </div>
            )}
          </article>
        </div>
      </section>
    </LocalPageFrame>
  );
}
